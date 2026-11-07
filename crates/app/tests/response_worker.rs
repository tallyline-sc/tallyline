// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Real response-processing admission, priority and joined lifetime contracts.

use tallyline_app::api::ResponseWorker;

#[test]
#[expect(
    non_snake_case,
    reason = "Requirement tags use a double underscore separator."
)]
fn r_nf_3_r_sub_5__response_processing_is_bounded_below_normal_and_off_the_io_runtime() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            use std::future::Future;
            use std::task::{Context, Poll, Waker};
            let (worker, processor) = ResponseWorker::start().unwrap();
            let (entered, entering) = std::sync::mpsc::channel();
            let (release, released) = std::sync::mpsc::channel();
            let mut first = std::pin::pin!(processor.process(move || {
                assert!(
                    tallyline_adapter_platform::api::worker_priority()
                        .unwrap()
                        .is_below_normal()
                );
                assert!(tokio::runtime::Handle::try_current().is_err());
                entered.send(()).unwrap();
                released.recv().unwrap();
                1
            }));
            let mut context = Context::from_waker(Waker::noop());
            assert!(matches!(first.as_mut().poll(&mut context), Poll::Pending));
            entering
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            let mut second = std::pin::pin!(processor.process(|| 2));
            let mut third = std::pin::pin!(processor.process(|| 3));
            assert!(matches!(second.as_mut().poll(&mut context), Poll::Pending));
            assert!(matches!(third.as_mut().poll(&mut context), Poll::Pending));
            assert!(
                processor.process(|| 4).await.is_err(),
                "The fourth response must not create another thread or unbounded queue entry"
            );
            release.send(()).unwrap();
            assert_eq!(first.await.unwrap(), 1);
            assert_eq!(second.await.unwrap(), 2);
            assert_eq!(third.await.unwrap(), 3);
            worker.finish().unwrap();
            assert!(processor.process(|| 5).await.is_err());
        });
}
