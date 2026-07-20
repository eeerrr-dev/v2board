//! `sd_notify(3)`-style state reporting for the production systemd units.
//!
//! Both native services run as `Type=notify`: the worker gates its watchdog on
//! dependency health, while the API pings unconditionally as an event-loop
//! liveness signal. Outside systemd (`NOTIFY_SOCKET` unset) every call is a
//! no-op, so local Docker and test runs need no special casing.

/// Send one datagram to the systemd notify socket, if present.
#[cfg(target_os = "linux")]
pub fn systemd_notify(message: &str) -> std::io::Result<()> {
    use std::{
        os::{linux::net::SocketAddrExt, unix::ffi::OsStrExt},
        path::Path,
    };

    let Some(path) = std::env::var_os("NOTIFY_SOCKET") else {
        return Ok(());
    };
    let socket = std::os::unix::net::UnixDatagram::unbound()?;
    let bytes = path.as_os_str().as_bytes();
    if let Some(abstract_name) = bytes.strip_prefix(b"@") {
        let address = std::os::unix::net::SocketAddr::from_abstract_name(abstract_name)?;
        socket.send_to_addr(message.as_bytes(), &address)?;
    } else {
        socket.send_to(message.as_bytes(), Path::new(&path))?;
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn systemd_notify(_message: &str) -> std::io::Result<()> {
    Ok(())
}
