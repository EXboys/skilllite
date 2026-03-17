//! HTTP Proxy server for filtering HTTP/HTTPS traffic.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

use skilllite_core::observability;

use super::config::ProxyConfig;
use super::tunnel;

/// HTTP Proxy server for filtering HTTP/HTTPS traffic
pub struct HttpProxy {
    config: Arc<RwLock<ProxyConfig>>,
    listener: Option<TcpListener>,
    running: Arc<AtomicBool>,
    port: u16,
}

impl HttpProxy {
    /// Create a new HTTP proxy with the given configuration
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
        let listener = self.listener.take().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "Proxy already started")
        })?;

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
                            tracing::warn!("[HTTP Proxy] Error handling client {}: {}", addr, e);
                        }
                    });
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(e) => {
                    tracing::error!("[HTTP Proxy] Accept error: {}", e);
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

        let mut reader = BufReader::new(client.try_clone()?);
        let mut request_line = String::new();
        reader.read_line(&mut request_line)?;

        let parts: Vec<&str> = request_line.trim().split_whitespace().collect();
        if parts.len() < 3 {
            return Self::send_error(&mut client, 400, "Bad Request");
        }

        let method = parts[0];
        let target = parts[1];

        if method == "CONNECT" {
            return Self::handle_connect(&mut client, &mut reader, target, &config);
        }

        Self::handle_http_request(
            &mut client,
            &mut reader,
            method,
            target,
            &request_line,
            &config,
        )
    }

    fn handle_connect(
        client: &mut TcpStream,
        reader: &mut BufReader<TcpStream>,
        target: &str,
        config: &Arc<RwLock<ProxyConfig>>,
    ) -> std::io::Result<()> {
        let (host, port) = Self::parse_host_port(target, 443)?;

        {
            let cfg = config.read().map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("proxy config lock: {}", e),
                )
            })?;
            if !cfg.is_domain_allowed(&host) {
                let blocked_target = format!("{}:{}", host, port);
                observability::security_blocked_network(
                    "unknown",
                    &blocked_target,
                    "domain_not_in_allowlist",
                );
                return Self::send_error(client, 403, "Forbidden - Domain not in allowlist");
            }
        }

        loop {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            if line.trim().is_empty() {
                break;
            }
        }

        let target_addr = format!("{}:{}", host, port);
        let mut target_stream = match TcpStream::connect_timeout(
            &target_addr.to_socket_addrs()?.next().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "Could not resolve host")
            })?,
            Duration::from_secs(30),
        ) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("[HTTP Proxy] Failed to connect to {}: {}", target_addr, e);
                return Self::send_error(client, 502, &format!("Bad Gateway - {}", e));
            }
        };

        client.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")?;
        client.flush()?;

        client.set_read_timeout(Some(Duration::from_secs(120)))?;
        client.set_write_timeout(Some(Duration::from_secs(120)))?;
        target_stream.set_read_timeout(Some(Duration::from_secs(120)))?;
        target_stream.set_write_timeout(Some(Duration::from_secs(120)))?;

        tunnel::tunnel_data(
            client,
            &mut target_stream,
            32768,
            Duration::from_secs(120),
            true,
        )
    }

    fn handle_http_request(
        client: &mut TcpStream,
        reader: &mut BufReader<TcpStream>,
        method: &str,
        target: &str,
        _request_line: &str,
        config: &Arc<RwLock<ProxyConfig>>,
    ) -> std::io::Result<()> {
        let host = if target.starts_with("http://") {
            let url = &target[7..];
            url.split('/')
                .next()
                .unwrap_or("")
                .split(':')
                .next()
                .unwrap_or("")
                .to_string()
        } else {
            return Self::send_error(client, 400, "Bad Request - Invalid URL");
        };

        {
            let cfg = config.read().map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("proxy config lock: {}", e),
                )
            })?;
            if !cfg.is_domain_allowed(&host) {
                observability::security_blocked_network(
                    "unknown",
                    &host,
                    "domain_not_in_allowlist",
                );
                return Self::send_error(client, 403, "Forbidden - Domain not in allowlist");
            }
        }

        let mut headers = Vec::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            if line.trim().is_empty() {
                break;
            }
            if !line.to_lowercase().starts_with("proxy-") {
                headers.push(line);
            }
        }

        let (target_host, target_port) = Self::parse_url_host_port(target)?;
        let target_addr = format!("{}:{}", target_host, target_port);

        let mut target_stream = match TcpStream::connect_timeout(
            &target_addr.to_socket_addrs()?.next().ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "Could not resolve host")
            })?,
            Duration::from_secs(30),
        ) {
            Ok(s) => s,
            Err(e) => {
                return Self::send_error(client, 502, &format!("Bad Gateway - {}", e));
            }
        };

        let path = if target.starts_with("http://") {
            let url = &target[7..];
            if let Some(pos) = url.find('/') {
                &url[pos..]
            } else {
                "/"
            }
        } else {
            target
        };

        let request = format!("{} {} HTTP/1.1\r\n", method, path);
        target_stream.write_all(request.as_bytes())?;
        for header in &headers {
            target_stream.write_all(header.as_bytes())?;
        }
        target_stream.write_all(b"\r\n")?;
        target_stream.flush()?;

        tunnel::tunnel_data(
            &mut target_stream,
            client,
            32768,
            Duration::from_secs(120),
            true,
        )
    }

    fn send_error(client: &mut TcpStream, code: u16, message: &str) -> std::io::Result<()> {
        let response = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{}\r\n",
            code, message, message
        );
        client.write_all(response.as_bytes())?;
        client.flush()?;
        Ok(())
    }

    fn parse_host_port(s: &str, default_port: u16) -> std::io::Result<(String, u16)> {
        if let Some(pos) = s.rfind(':') {
            let host = s[..pos].to_string();
            let port = s[pos + 1..].parse::<u16>().map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid port")
            })?;
            Ok((host, port))
        } else {
            Ok((s.to_string(), default_port))
        }
    }

    fn parse_url_host_port(url: &str) -> std::io::Result<(String, u16)> {
        let url = if url.starts_with("http://") {
            &url[7..]
        } else if url.starts_with("https://") {
            &url[8..]
        } else {
            url
        };
        let host_port = url.split('/').next().unwrap_or(url);
        Self::parse_host_port(host_port, 80)
    }
}
