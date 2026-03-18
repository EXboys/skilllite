//! SOCKS5 Proxy server for filtering other TCP traffic.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

use skilllite_core::observability;

use super::config::ProxyConfig;
use super::tunnel;

/// SOCKS5 Proxy server for filtering other TCP traffic
pub struct Socks5Proxy {
    config: Arc<RwLock<ProxyConfig>>,
    listener: Option<TcpListener>,
    running: Arc<AtomicBool>,
    port: u16,
}

impl Socks5Proxy {
    /// Create a new SOCKS5 proxy with the given configuration
    pub fn new(config: ProxyConfig) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        listener.set_nonblocking(true)?;

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            listener: Some(listener),
            running: Arc::new(AtomicBool::new(false)),
            port,
        })
    }

    /// Get the port the proxy is listening on
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Start the proxy server in a background thread
    pub fn start(&mut self) -> std::io::Result<thread::JoinHandle<()>> {
        let listener = self
            .listener
            .take()
            .ok_or_else(|| std::io::Error::other("Proxy already started"))?;

        self.running.store(true, Ordering::SeqCst);
        let running = Arc::clone(&self.running);
        let config = Arc::clone(&self.config);

        let handle = thread::spawn(move || {
            Self::run_server(listener, config, running);
        });

        Ok(handle)
    }

    /// Stop the proxy server
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    fn run_server(
        listener: TcpListener,
        config: Arc<RwLock<ProxyConfig>>,
        running: Arc<AtomicBool>,
    ) {
        while running.load(Ordering::SeqCst) {
            match listener.accept() {
                Ok((stream, addr)) => {
                    let config = Arc::clone(&config);
                    thread::spawn(move || {
                        if let Err(e) = Self::handle_client(stream, addr, config) {
                            tracing::warn!("[SOCKS5 Proxy] Error handling client {}: {}", addr, e);
                        }
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(e) => {
                    tracing::error!("[SOCKS5 Proxy] Accept error: {}", e);
                }
            }
        }
    }

    fn handle_client(
        mut client: TcpStream,
        _addr: SocketAddr,
        config: Arc<RwLock<ProxyConfig>>,
    ) -> std::io::Result<()> {
        client.set_read_timeout(Some(Duration::from_secs(30)))?;
        client.set_write_timeout(Some(Duration::from_secs(30)))?;

        let mut buf = [0u8; 256];

        client.read_exact(&mut buf[..2])?;
        if buf[0] != 0x05 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid SOCKS version",
            ));
        }

        let nmethods = buf[1] as usize;
        client.read_exact(&mut buf[..nmethods])?;

        let has_no_auth = buf[..nmethods].contains(&0x00);
        if !has_no_auth {
            client.write_all(&[0x05, 0xFF])?;
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "No acceptable auth method",
            ));
        }

        client.write_all(&[0x05, 0x00])?;

        client.read_exact(&mut buf[..4])?;
        if buf[0] != 0x05 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid SOCKS version",
            ));
        }

        let cmd = buf[1];
        if cmd != 0x01 {
            Self::send_reply(&mut client, 0x07)?;
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Only CONNECT is supported",
            ));
        }

        let atyp = buf[3];
        let (host, port) = match atyp {
            0x01 => {
                client.read_exact(&mut buf[..4])?;
                let ip = format!("{}.{}.{}.{}", buf[0], buf[1], buf[2], buf[3]);
                client.read_exact(&mut buf[..2])?;
                let port = u16::from_be_bytes([buf[0], buf[1]]);
                (ip, port)
            }
            0x03 => {
                client.read_exact(&mut buf[..1])?;
                let len = buf[0] as usize;
                client.read_exact(&mut buf[..len])?;
                let domain = String::from_utf8_lossy(&buf[..len]).to_string();
                client.read_exact(&mut buf[..2])?;
                let port = u16::from_be_bytes([buf[0], buf[1]]);
                (domain, port)
            }
            0x04 => {
                client.read_exact(&mut buf[..16])?;
                let ip = format!(
                    "{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}:{:x}",
                    u16::from_be_bytes([buf[0], buf[1]]),
                    u16::from_be_bytes([buf[2], buf[3]]),
                    u16::from_be_bytes([buf[4], buf[5]]),
                    u16::from_be_bytes([buf[6], buf[7]]),
                    u16::from_be_bytes([buf[8], buf[9]]),
                    u16::from_be_bytes([buf[10], buf[11]]),
                    u16::from_be_bytes([buf[12], buf[13]]),
                    u16::from_be_bytes([buf[14], buf[15]]),
                );
                client.read_exact(&mut buf[..2])?;
                let port = u16::from_be_bytes([buf[0], buf[1]]);
                (ip, port)
            }
            _ => {
                Self::send_reply(&mut client, 0x08)?;
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Invalid address type",
                ));
            }
        };

        {
            let cfg = config
                .read()
                .map_err(|e| std::io::Error::other(format!("proxy config lock: {}", e)))?;
            let allowed = if atyp == 0x01 || atyp == 0x04 {
                cfg.is_ip_connection_allowed(&host)
            } else {
                cfg.is_domain_allowed(&host)
            };

            if !allowed {
                let blocked_target = format!("{}:{}", host, port);
                let reason = if atyp == 0x01 || atyp == 0x04 {
                    "ip_direct_connection_blocked"
                } else {
                    "domain_not_in_allowlist"
                };
                observability::security_blocked_network("unknown", &blocked_target, reason);
                Self::send_reply(&mut client, 0x02)?;
                return Ok(());
            }
        }

        let target_addr = format!("{}:{}", host, port);
        let target_stream = match target_addr.as_str().to_socket_addrs() {
            Ok(mut addrs) => {
                if let Some(addr) = addrs.next() {
                    match TcpStream::connect_timeout(&addr, Duration::from_secs(30)) {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::warn!(
                                "[SOCKS5 Proxy] Failed to connect to {}: {}",
                                target_addr,
                                e
                            );
                            Self::send_reply(&mut client, 0x05)?;
                            return Ok(());
                        }
                    }
                } else {
                    Self::send_reply(&mut client, 0x04)?;
                    return Ok(());
                }
            }
            Err(e) => {
                tracing::warn!("[SOCKS5 Proxy] Failed to resolve {}: {}", host, e);
                Self::send_reply(&mut client, 0x04)?;
                return Ok(());
            }
        };

        Self::send_reply(&mut client, 0x00)?;

        let mut target = target_stream;
        tunnel::tunnel_data(
            &mut client,
            &mut target,
            8192,
            Duration::from_secs(60),
            false,
        )
    }

    fn send_reply(client: &mut TcpStream, rep: u8) -> std::io::Result<()> {
        let reply = [0x05, rep, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        client.write_all(&reply)?;
        client.flush()
    }
}
