// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fixed activation framing with one overall deadline per exchange.

use crate::socket::LocalSocket;
use std::io;
use std::time::{Duration, Instant};

pub(crate) const TOKEN: &[u8] = b"activate";
const ACKNOWLEDGEMENT: &[u8] = b"1";
pub(crate) struct ProtocolLimits {
    pub(crate) startup_timeout: Duration,
    request_timeout: Duration,
    poll_interval: Duration,
}

pub(crate) const LIMITS: ProtocolLimits = ProtocolLimits {
    startup_timeout: Duration::from_secs(2),
    request_timeout: Duration::from_millis(250),
    poll_interval: Duration::from_millis(5),
};

pub(crate) fn wait(deadline: Instant) -> io::Result<()> {
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or_else(|| io::Error::from(io::ErrorKind::TimedOut))?;
    std::thread::sleep(remaining.min(LIMITS.poll_interval));
    Ok(())
}

fn read_frame<const N: usize>(
    socket: &LocalSocket,
    deadline: Instant,
) -> io::Result<([u8; N], usize)> {
    let mut buffer = [0; N];
    let mut used = 0;
    loop {
        if used == N {
            return Err(io::ErrorKind::InvalidData.into());
        }
        match socket.read(&mut buffer[used..]) {
            Ok(0) => return Ok((buffer, used)),
            Ok(count) => used += count,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => wait(deadline)?,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => wait(deadline)?,
            Err(error) => return Err(error),
        }
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
    }
}

fn write_frame(socket: &LocalSocket, mut bytes: &[u8], deadline: Instant) -> io::Result<()> {
    while !bytes.is_empty() {
        if Instant::now() >= deadline {
            return Err(io::ErrorKind::TimedOut.into());
        }
        match socket.write(bytes) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(count) => bytes = bytes.get(count..).ok_or(io::ErrorKind::InvalidData)?,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => wait(deadline)?,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => wait(deadline)?,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

pub(crate) fn request(socket: &LocalSocket, deadline: Instant) -> io::Result<()> {
    write_frame(socket, TOKEN, deadline)?;
    socket.finish_request()?;
    let (bytes, used) = read_frame::<{ ACKNOWLEDGEMENT.len() + 1 }>(socket, deadline)?;
    if bytes.get(..used) != Some(ACKNOWLEDGEMENT) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(())
}

pub(crate) fn accept(socket: &LocalSocket) -> io::Result<bool> {
    let deadline = Instant::now() + LIMITS.request_timeout;
    let (bytes, used) = read_frame::<{ TOKEN.len() + 1 }>(socket, deadline)?;
    if bytes.get(..used) != Some(TOKEN) {
        return Ok(false);
    }
    write_frame(socket, ACKNOWLEDGEMENT, deadline)?;
    socket.finish_request()?;
    Ok(true)
}
