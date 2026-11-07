// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Guarded platform socket operations without raw handle escape.

use socket2::SockAddr;
#[cfg(unix)]
use socket2::Socket;
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::time::Duration;

#[cfg(windows)]
pub(crate) use crate::windows_socket::WindowsSocket as LocalSocket;

#[cfg(unix)]
pub(crate) struct LocalSocket {
    socket: Socket,
}

impl LocalSocket {
    #[cfg(unix)]
    pub(crate) fn new() -> io::Result<Self> {
        Ok(Self {
            socket: Socket::new(socket2::Domain::UNIX, socket2::Type::STREAM, None)?,
        })
    }

    pub(crate) fn nonblocking(&self) -> io::Result<()> {
        self.socket.set_nonblocking(true)
    }
    pub(crate) fn bind(&self, address: &SockAddr) -> io::Result<()> {
        self.socket.bind(address)
    }
    pub(crate) fn listen(&self) -> io::Result<()> {
        self.socket.listen(8)
    }
    pub(crate) fn connect(&self, address: &SockAddr, timeout: Duration) -> io::Result<()> {
        self.socket.connect_timeout(address, timeout)?;
        self.nonblocking()
    }
    pub(crate) fn accept(&self) -> io::Result<Self> {
        let (socket, _) = self.socket.accept()?;
        let accepted = Self {
            socket,
            #[cfg(windows)]
            runtime: std::sync::Arc::clone(&self.runtime),
        };
        accepted.nonblocking()?;
        Ok(accepted)
    }
    pub(crate) fn read(&self, buffer: &mut [u8]) -> io::Result<usize> {
        (&self.socket).read(buffer)
    }
    pub(crate) fn write(&self, buffer: &[u8]) -> io::Result<usize> {
        (&self.socket).write(buffer)
    }
    pub(crate) fn finish_request(&self) -> io::Result<()> {
        self.socket.shutdown(Shutdown::Write)
    }
}
