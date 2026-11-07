// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Serial bounded image work on an owned below-normal CPU thread.

use std::io;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError},
};
use std::thread::JoinHandle;
use std::time::Duration;
use tallyline_adapter_files::api::model::{
    AttachmentEncodingSettings, AttachmentImageError, AttachmentPreview, EncodedAttachment,
    ImageIntakeSettings,
};
use tallyline_adapter_files::api::{encode_manual_attachment, prepare_manual_attachment_file};
use tallyline_reporting::api::model::{AttachmentRegion, Report};

type Reply<T> = mpsc::Sender<Result<T, AttachmentImageError>>;

/// A full image queue or a worker that is no longer running.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachmentWorkerError {
    /// One image is running and another is already queued.
    Busy,
    /// The owned worker has stopped or failed priority setup.
    Stopped,
}

enum Task {
    Prepare(
        PathBuf,
        AttachmentRegion,
        ImageIntakeSettings,
        Reply<AttachmentPreview>,
    ),
    Encode(
        Box<Report>,
        AttachmentPreview,
        AttachmentEncodingSettings,
        Reply<EncodedAttachment>,
    ),
}

/// Owns one image task at a time with one queued task and joined shutdown.
pub struct AttachmentWorker {
    incoming: SyncSender<Task>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<io::Result<()>>>,
}
impl AttachmentWorker {
    /// Starts the serial image worker and verifies below-normal priority before handling work.
    /// Calls the nonblocking repaint hook after each result, including an abandoned reply.
    ///
    /// # Errors
    /// Returns thread creation failure; priority failure stops the worker before any image work.
    pub fn start(wake: impl Fn() + Send + 'static) -> io::Result<Self> {
        let (incoming, tasks) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let worker = crate::executor::spawn("tallyline-attachment", move || {
            tallyline_adapter_platform::api::lower_worker_priority()?;
            while !stopping.load(Ordering::Acquire) {
                let task = match tasks.recv_timeout(Duration::from_millis(100)) {
                    Ok(task) => task,
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => break,
                };
                match task {
                    Task::Prepare(path, region, settings, reply) => deliver(
                        reply,
                        prepare_manual_attachment_file(&path, region, &settings),
                    ),
                    Task::Encode(report, preview, settings, reply) => deliver(
                        reply,
                        encode_manual_attachment(&report, &preview, &settings),
                    ),
                }
                wake();
            }
            Ok(())
        })?;
        Ok(Self {
            incoming,
            stop,
            worker: Some(worker),
        })
    }
    /// Queues bounded source decoding, hashing and source-scale region preparation.
    ///
    /// # Errors
    /// Returns a full queue or a stopped worker without accepting the task.
    pub fn prepare(
        &self,
        path: PathBuf,
        region: AttachmentRegion,
        settings: ImageIntakeSettings,
    ) -> Result<Receiver<Result<AttachmentPreview, AttachmentImageError>>, AttachmentWorkerError>
    {
        let (reply, receive) = mpsc::channel();
        self.submit(Task::Prepare(path, region, settings, reply))?;
        Ok(receive)
    }
    /// Queues encoding of only the exact Report-confirmed preview.
    ///
    /// # Errors
    /// Returns a full queue or a stopped worker without accepting the task.
    pub fn encode(
        &self,
        report: Report,
        preview: AttachmentPreview,
        settings: AttachmentEncodingSettings,
    ) -> Result<Receiver<Result<EncodedAttachment, AttachmentImageError>>, AttachmentWorkerError>
    {
        let (reply, receive) = mpsc::channel();
        self.submit(Task::Encode(Box::new(report), preview, settings, reply))?;
        Ok(receive)
    }
    /// Stops admission and joins the current image task before returning.
    /// Queued tasks are abandoned and their reply channels disconnect.
    ///
    /// # Errors
    /// Returns failed priority setup or a worker panic.
    pub fn finish(mut self) -> io::Result<()> {
        self.join()
    }
    fn submit(&self, task: Task) -> Result<(), AttachmentWorkerError> {
        if self.stop.load(Ordering::Acquire) {
            return Err(AttachmentWorkerError::Stopped);
        }
        self.incoming.try_send(task).map_err(|error| match error {
            TrySendError::Full(_) => AttachmentWorkerError::Busy,
            TrySendError::Disconnected(_) => AttachmentWorkerError::Stopped,
        })
    }
    fn join(&mut self) -> io::Result<()> {
        self.stop.store(true, Ordering::Release);
        match self.worker.take() {
            Some(worker) => worker
                .join()
                .map_err(|_| io::Error::other("attachment worker terminated unexpectedly"))?,
            None => Ok(()),
        }
    }
}
impl Drop for AttachmentWorker {
    fn drop(&mut self) {
        if let Err(error) = self.join() {
            tracing::error!(%error,"Attachment worker shutdown failed");
        }
    }
}
fn deliver<T>(reply: Reply<T>, result: Result<T, AttachmentImageError>) {
    if reply.send(result).is_err() {
        tracing::debug!("Attachment result was abandoned");
    }
}
