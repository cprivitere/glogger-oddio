//! Dependency-free single-instance guard: holding a bound loopback socket for
//! the process lifetime. The OS releases it on exit, so there is no stale lock
//! file to clean up. Deliberately not `tauri-plugin-single-instance`: the build
//! must not need network access to fetch a new crate.
//!
//! The port is derived from the install's identity (bundle identifier + app
//! data directory), so two instances of the *same* install collide and the
//! second refuses to start, while a release and an experimental build — which
//! use distinct data directories and databases — can run side by side.

use std::hash::{Hash, Hasher};
use std::net::TcpListener;

use tauri::Manager;

/// IANA dynamic/private range the guard draws its port from.
const PORT_BASE: u16 = 49152;
const PORT_SPAN: u32 = 16384; // 49152..=65535

/// Holds the bound listener for the process lifetime; dropping it (or process
/// exit) releases the port.
pub struct SingleInstanceGuard {
    _listener: TcpListener,
}

/// Stable identity of this install: the bundle identifier plus the resolved
/// app data directory. Release and experimental builds differ in both, so they
/// map to different ports; two launches of the same build match.
fn guard_key(app: &tauri::AppHandle) -> String {
    let identifier = app.config().identifier.clone();
    let dir = app
        .path()
        .app_data_dir()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    format!("{identifier}|{dir}")
}

/// Map an install key to a loopback port in `PORT_BASE..PORT_BASE+PORT_SPAN`.
/// `DefaultHasher::new()` is deterministic (fixed seeds), so every process of
/// the same build derives the same port.
fn guard_port(key: &str) -> u16 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    PORT_BASE + (hasher.finish() % PORT_SPAN as u64) as u16
}

/// Try to become the sole instance of this install. Returns `true` if this
/// process owns the guard (or the port could not be bound for a non-conflict
/// reason, in which case the app must still run). Returns `false` only when the
/// address is already in use — i.e. another instance of the same install is
/// running.
pub fn acquire(app: &tauri::AppHandle) -> bool {
    let addr = format!("127.0.0.1:{}", guard_port(&guard_key(app)));
    match TcpListener::bind(addr) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn port_is_deterministic_and_in_range() {
        let a = guard_port("com.example.glogger|/home/u/.local/share/glogger");
        let b = guard_port("com.example.glogger|/home/u/.local/share/glogger");
        assert_eq!(a, b, "same install key must map to the same port");
        let max = (PORT_BASE as u32 + PORT_SPAN - 1) as u16;
        assert!((PORT_BASE..=max).contains(&a));
    }

    #[test]
    fn release_and_experimental_installs_get_distinct_ports() {
        // Different bundle identifiers and data dirs => the two builds coexist.
        let release = guard_port("com.glogger|/d/glogger.Release");
        let experimental = guard_port("com.glogger.Experimental|/d/glogger.Experimental");
        assert_ne!(release, experimental);
    }
}
