// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Windows AF_UNIX ownership without socket-constructor bootstrap traffic.

use socket2::Socket;
use std::io;
use std::os::windows::io::{FromRawSocket, OwnedSocket};
use std::sync::Arc;
use windows_sys::Win32::Networking::WinSock::{
    AF_UNIX, INVALID_SOCKET, SOCK_STREAM, SOCKET_ERROR, WSA_FLAG_NO_HANDLE_INHERIT,
    WSA_FLAG_OVERLAPPED, WSACleanup, WSADATA, WSAGetLastError, WSASocketW, WSAStartup,
};

const WINSOCK_2_2: u16 = 0x0202;

pub(super) struct Winsock;

impl Winsock {
    /// Acquires one Winsock reference and checks the negotiated version.
    ///
    /// # Safety
    /// The local initialised structure remains writable throughout WSAStartup;
    /// every successful startup immediately gains a guard balanced by cleanup.
    #[expect(
        unsafe_code,
        reason = "Winsock startup has no safe API without socket creation."
    )]
    fn start() -> io::Result<Self> {
        let mut data = WSADATA::default();
        let code = unsafe { WSAStartup(WINSOCK_2_2, &mut data) };
        if code != 0 {
            return Err(io::Error::from_raw_os_error(code));
        }
        let runtime = Self;
        if data.wVersion != WINSOCK_2_2 {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Winsock 2.2 is unavailable",
            ));
        }
        Ok(runtime)
    }

    /// Releases this guard's one successful Winsock startup reference.
    ///
    /// # Safety
    /// Only Drop calls this once after the final guarded socket has closed;
    /// the matching successful startup reference belongs to this guard.
    #[expect(
        unsafe_code,
        reason = "Winsock reference cleanup requires the operating-system API."
    )]
    fn cleanup(&self) -> io::Result<()> {
        let code = unsafe {
            if WSACleanup() == SOCKET_ERROR {
                WSAGetLastError()
            } else {
                0
            }
        };
        if code == 0 {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(code))
        }
    }
}

impl Drop for Winsock {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            tracing::error!(%error, "Winsock cleanup failed");
        }
    }
}

pub(super) struct WindowsSocket {
    pub(super) socket: Socket,
    pub(super) runtime: Arc<Winsock>,
}

impl WindowsSocket {
    /// Creates one non-inheritable AF_UNIX stream with its Winsock lifetime.
    ///
    /// # Safety
    /// Null protocol information is valid with the explicit family/type/protocol;
    /// a successful uniquely owned handle transfers exactly once to OwnedSocket.
    /// The runtime guard outlives the socket, including all error paths.
    #[expect(
        unsafe_code,
        reason = "Direct AF_UNIX construction avoids the library UDP bootstrap."
    )]
    pub(super) fn new() -> io::Result<Self> {
        let runtime = Arc::new(Winsock::start()?);
        let owned = unsafe {
            let raw = WSASocketW(
                i32::from(AF_UNIX),
                SOCK_STREAM,
                0,
                std::ptr::null(),
                0,
                WSA_FLAG_NO_HANDLE_INHERIT | WSA_FLAG_OVERLAPPED,
            );
            if raw == INVALID_SOCKET {
                return Err(io::Error::from_raw_os_error(WSAGetLastError()));
            }
            OwnedSocket::from_raw_socket(raw as std::os::windows::io::RawSocket)
        };
        Ok(Self {
            socket: Socket::from(owned),
            runtime,
        })
    }
}
