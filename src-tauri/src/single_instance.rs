//! Dependency-free single-instance guard: holding a bound loopback socket for
//! the process lifetime. The OS releases it on exit, so there is no stale lock
//! file to clean up. Deliberately not `tauri-plugin-single-instance`: the build
//! must not need network access to fetch a new crate.

use std::net::TcpListener;

use tauri::Manager;

/// Loopback port reserved for the guard. Binds only on localhost, so it never
/// collides with another machine's process and is what second launches probe.
pub const SINGLE_INSTANCE_ADDR: &str = "127.0.0.1:49731";

/// Holds the bound listener for the process lifetime; dropping it (or process
/// exit) releases the port.
pub struct SingleInstanceGuard {
    _listener: TcpListener,
}

/// Try to become the sole instance. Returns `true` if this process owns the
/// guard (or the port could not be bound for a non-conflict reason, in which
/// case the app must still run). Returns `false` only when the address is
/// already in use — i.e. another instance is running.
pub fn acquire(app: &tauri::AppHandle) -> bool {
    match TcpListener::bind(SINGLE_INSTANCE_ADDR) {
        Ok(listener) => {
            app.manage(SingleInstanceGuard {
                _listener: listener,
            });
            true
        }
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => false,
        Err(e) => {
            // A firewall or disabled loopback must not brick the app — carry on.
            eprintln!("[single-instance] bind failed, continuing: {e}");
            true
        }
    }
}
