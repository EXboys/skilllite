//! Shared TCP tunneling between two streams.

use std::io::{Read, Write};
use std::net::{Shutdown as NetShutdown, TcpStream};
use std::thread;
use std::time::Duration;

/// Bidirectionally tunnel data between two TCP streams.
///
/// Spawns two threads — one for each direction — and waits for both to finish.
/// Parameters allow callers to control buffer size, read timeout, and whether
/// Nagle's algorithm is disabled (nodelay).
pub(super) fn tunnel_data(
    stream1: &mut TcpStream,
    stream2: &mut TcpStream,
    buf_size: usize,
    read_timeout: Duration,
    nodelay: bool,
) -> std::io::Result<()> {
    let mut s1_read = stream1.try_clone()?;
    let mut s1_write = stream1.try_clone()?;
    let mut s2_read = stream2.try_clone()?;
    let mut s2_write = stream2.try_clone()?;

    s1_read.set_read_timeout(Some(read_timeout))?;
    s2_read.set_read_timeout(Some(read_timeout))?;

    if nodelay {
        let _ = s1_read.set_nodelay(true);
        let _ = s2_read.set_nodelay(true);
    }

    // stream1 → stream2
    let handle1 = thread::spawn(move || {
        let mut buf = vec![0u8; buf_size];
        loop {
            match s1_read.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if s2_write.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    if s2_write.flush().is_err() {
                        break;
                    }
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    continue;
                }
                Err(_) => break,
            }
        }
        let _ = s2_write.shutdown(NetShutdown::Write);
    });

    // stream2 → stream1
    let handle2 = thread::spawn(move || {
        let mut buf = vec![0u8; buf_size];
        loop {
            match s2_read.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if s1_write.write_all(&buf[..n]).is_err() {
                        break;
                    }
                    if s1_write.flush().is_err() {
                        break;
                    }
                }
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    continue;
                }
                Err(_) => break,
            }
        }
        let _ = s1_write.shutdown(NetShutdown::Write);
    });

    let _ = handle1.join();
    let _ = handle2.join();

    Ok(())
}
