// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Withholding of actual HTTP connections until their ownership is observed.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr};
use std::num::NonZeroU16;
use std::pin::Pin;
use std::task::{Context, Poll};

use hyper_util::client::legacy::connect::{Connection, HttpInfo};
use tallyline_recognition::api::MetadataOwnershipSession;
use tallyline_recognition::api::model::{
    LoopbackConnection, MetadataAttempt, MetadataGeneration, MetadataOwnership, MetadataRefusal,
    MetadataSessionState,
};
use tokio::time::{self, Instant};
use tower::{Layer, Service};

type BoxError = Box<dyn Error + Send + Sync>;

#[derive(Clone)]
pub(crate) struct MetadataConnector<T> {
    session: T,
    generation: MetadataGeneration,
    port: NonZeroU16,
    deadline: Instant,
}

impl<T> MetadataConnector<T> {
    pub(crate) fn new(
        session: T,
        generation: MetadataGeneration,
        port: NonZeroU16,
        deadline: Instant,
    ) -> Self {
        Self {
            session,
            generation,
            port,
            deadline,
        }
    }
}

impl<S, T: Clone> Layer<S> for MetadataConnector<T> {
    type Service = OwnershipGate<S, T>;

    fn layer(&self, inner: S) -> Self::Service {
        OwnershipGate {
            inner,
            context: self.clone(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct OwnershipGate<S, T> {
    inner: S,
    context: MetadataConnector<T>,
}

impl<S, T, Request> Service<Request> for OwnershipGate<S, T>
where
    S: Service<Request, Error = BoxError>,
    S::Future: Send + 'static,
    S::Response: Connection + Send + 'static,
    T: MetadataOwnershipSession,
{
    type Response = S::Response;
    type Error = BoxError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, BoxError>> + Send>>;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let connection = self.inner.call(request);
        let context = self.context.clone();
        Box::pin(async move {
            let gate = async {
                current_generation(&context.session, context.generation).map_err(boxed_refusal)?;
                within_deadline(context.deadline).map_err(boxed_refusal)?;
                let connection = connection.await?;
                let attempt = actual_attempt(&connection, context.generation, context.port)
                    .map_err(boxed_refusal)?;
                match context.session.observe(attempt).await {
                    MetadataOwnership::Owned(observed)
                        if observed.generation() != attempt.generation() =>
                    {
                        Err(boxed_refusal(MetadataRefusal::GenerationChanged))
                    }
                    MetadataOwnership::Owned(observed) if observed != attempt => {
                        Err(boxed_refusal(MetadataRefusal::TupleMismatch))
                    }
                    MetadataOwnership::Owned(_) => {
                        current_generation(&context.session, context.generation)
                            .map_err(boxed_refusal)?;
                        within_deadline(context.deadline).map_err(boxed_refusal)?;
                        Ok(connection)
                    }
                    MetadataOwnership::ForeignListener => {
                        Err(boxed_refusal(MetadataRefusal::ForeignListener))
                    }
                    MetadataOwnership::Unknown => {
                        Err(boxed_refusal(MetadataRefusal::OwnershipUnknown))
                    }
                }
            };
            tokio::select! {
                biased;
                reason = context.session.revoked(context.generation) => Err(boxed_refusal(reason)),
                result = time::timeout_at(context.deadline, gate) => result.map_err(|_| boxed_refusal(MetadataRefusal::Deadline))?,
            }
        })
    }
}

fn actual_attempt<C: Connection>(
    connection: &C,
    generation: MetadataGeneration,
    port: NonZeroU16,
) -> Result<MetadataAttempt, MetadataRefusal> {
    let connected = connection.connected();
    if connected.is_proxied() {
        return Err(MetadataRefusal::Configuration);
    }
    let mut extras = Default::default();
    connected.get_extras(&mut extras);
    let info = extras
        .get::<HttpInfo>()
        .ok_or(MetadataRefusal::OwnershipUnknown)?;
    let local = info.local_addr();
    let remote = info.remote_addr();
    let loopback = IpAddr::V4(Ipv4Addr::LOCALHOST);
    if local.ip() != loopback || remote.ip() != loopback || remote.port() != port.get() {
        return Err(MetadataRefusal::TupleMismatch);
    }
    let client_port = NonZeroU16::new(local.port()).ok_or(MetadataRefusal::TupleMismatch)?;
    Ok(MetadataAttempt::new(
        generation,
        LoopbackConnection::new(client_port, port),
    ))
}

#[derive(Debug)]
struct ConnectorRefusal(MetadataRefusal);

impl Display for ConnectorRefusal {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("local metadata connection refused")
    }
}

impl Error for ConnectorRefusal {}

fn boxed_refusal(reason: MetadataRefusal) -> BoxError {
    Box::new(ConnectorRefusal(reason))
}

pub(crate) fn transport_refusal(error: &reqwest::Error) -> MetadataRefusal {
    let mut source: Option<&(dyn Error + 'static)> = Some(error);
    while let Some(error) = source {
        if let Some(refusal) = error.downcast_ref::<ConnectorRefusal>() {
            return refusal.0;
        }
        source = error.source();
    }
    if error.is_timeout() {
        MetadataRefusal::Deadline
    } else {
        MetadataRefusal::Transport
    }
}

pub(crate) fn current_generation<T: MetadataOwnershipSession>(
    session: &T,
    generation: MetadataGeneration,
) -> Result<(), MetadataRefusal> {
    match session.state() {
        MetadataSessionState::Current(current) if current == generation => Ok(()),
        MetadataSessionState::Current(_) => Err(MetadataRefusal::GenerationChanged),
        MetadataSessionState::Revoked(reason) => Err(reason),
    }
}

pub(crate) fn within_deadline(deadline: Instant) -> Result<(), MetadataRefusal> {
    if Instant::now() >= deadline {
        Err(MetadataRefusal::Deadline)
    } else {
        Ok(())
    }
}
