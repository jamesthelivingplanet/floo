//! OS-level port probing.
//!
//! We never trust the registry alone. Before handing out a port, we also ask
//! the OS whether anything is bound to it. This guards against unrelated
//! processes (Docker, random scripts) that floo doesn't know about.

use std::net::TcpListener;

/// Return true if nothing is currently bound to `127.0.0.1:port`.
///
/// We test by trying to bind a socket ourselves and immediately dropping it.
/// If bind fails (typically EADDRINUSE), something else holds it.
///
/// Caveat: this is a point-in-time check. Between this call and the caller
/// actually starting its server, an unrelated process could grab the port.
/// That race is accepted, see SPEC.md.
pub fn is_port_free_on_os(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bound_port_is_not_free_then_free_after_drop() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();

        assert!(!is_port_free_on_os(port));

        drop(listener);

        assert!(is_port_free_on_os(port));
    }
}
