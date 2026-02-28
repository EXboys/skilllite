//! Network Proxy Module for Domain-Level Filtering
//!
//! This module implements HTTP and SOCKS5 proxy servers that run on the host
//! and filter network traffic based on domain allowlists/denylists.
//!
//! Architecture:
//! - Sandboxed process can only connect to localhost proxy ports
//! - HTTP Proxy: Handles HTTP/HTTPS traffic with domain filtering
//! - SOCKS5 Proxy: Handles other TCP traffic (SSH, databases, etc.)
//!
//! On macOS: Seatbelt profile allows only localhost:proxy_port
//! On Linux: Network namespace removed, traffic routed via Unix socket

use skilllite_core::observability;
use std::io::{Read, Write, BufRead, BufReader};
use std::net::{TcpListener, TcpStream, SocketAddr, ToSocketAddrs, Shutdown};
use std::sync::{Arc, RwLock, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::Duration;

// ============================================================================
// Reverse DNS Lookup (F5: IP direct-connect blocking)
// ============================================================================

/// Attempt reverse DNS lookup for an IP address using system `getnameinfo`.
/// Returns the resolved hostname, or `None` if lookup fails or returns the
/// raw IP string (no PTR record).
#[cfg(unix)]
fn reverse_dns_lookup(ip: &std::net::IpAddr) -> Option<String> {
    use std::net::IpAddr;

    unsafe {
        let mut host_buf = [0u8; 1025]; // NI_MAXHOST

        let ret = match ip {
            IpAddr::V4(ipv4) => {
                let mut sa: libc::sockaddr_in = std::mem::zeroed();
                sa.sin_family = libc::AF_INET as libc::sa_family_t;
                sa.sin_addr.s_addr = u32::from_ne_bytes(ipv4.octets());
                libc::getnameinfo(
                    &sa as *const _ as *const libc::sockaddr,
                    std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
                    host_buf.as_mut_ptr() as *mut libc::c_char,
                    host_buf.len() as libc::socklen_t,
                    std::ptr::null_mut(),
                    0,
                    libc::NI_NAMEREQD,
                )
            }
            IpAddr::V6(ipv6) => {
                let mut sa: libc::sockaddr_in6 = std::mem::zeroed();
                sa.sin6_family = libc::AF_INET6 as libc::sa_family_t;
                sa.sin6_addr.s6_addr = ipv6.octets();
                libc::getnameinfo(
                    &sa as *const _ as *const libc::sockaddr,
                    std::mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t,
                    host_buf.as_mut_ptr() as *mut libc::c_char,
                    host_buf.len() as libc::socklen_t,
                    std::ptr::null_mut(),
                    0,
                    libc::NI_NAMEREQD,
                )
            }
        };

        if ret != 0 {
            return None;
        }

        let c_str = std::ffi::CStr::from_ptr(host_buf.as_ptr() as *const libc::c_char);
        c_str.to_str().ok().map(|s| s.to_string())
    }
}

#[cfg(not(unix))]
fn reverse_dns_lookup(_ip: &std::net::IpAddr) -> Option<String> {
    None
}

// ============================================================================
// Proxy Configuration
// ============================================================================

/// Configuration for the network proxy
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    /// Allowed domains (supports wildcards like *.github.com)
    pub allowed_domains: Vec<String>,
    /// Denied domains (takes precedence over allowed)
    pub denied_domains: Vec<String>,
    /// Whether to allow all domains if allowlist is empty
    pub allow_all_if_empty: bool,
    /// Whether loopback addresses (127.0.0.0/8, ::1, "localhost") are allowed
    /// by default.  Loopback traffic stays on the machine and is not a data
    /// exfiltration vector, so it is allowed unless explicitly denied.
    pub allow_loopback: bool,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            allowed_domains: Vec::new(),
            denied_domains: Vec::new(),
            allow_all_if_empty: false,
            allow_loopback: true,
        }
    }
}

impl ProxyConfig {
    /// Create a config that blocks all network access
    pub fn block_all() -> Self {
        Self {
            allowed_domains: Vec::new(),
            denied_domains: Vec::new(),
            allow_all_if_empty: false,
            allow_loopback: false,
        }
    }

    /// Create a config with specific allowed domains
    pub fn with_allowed_domains(domains: Vec<String>) -> Self {
        Self {
            allowed_domains: domains,
            denied_domains: Vec::new(),
            allow_all_if_empty: false,
            allow_loopback: true,
        }
    }

    /// Whether `domain` is a loopback name (RFC 6761 ".localhost" TLD).
    fn is_loopback_domain(domain: &str) -> bool {
        let d = domain.to_lowercase();
        d == "localhost" || d.ends_with(".localhost")
    }

    /// Check if a domain is allowed
    pub fn is_domain_allowed(&self, domain: &str) -> bool {
        let domain_lower = domain.to_lowercase();

        // Check denied list first (takes precedence)
        for denied in &self.denied_domains {
            if Self::domain_matches(&domain_lower, denied) {
                return false;
            }
        }

        // Loopback domains (localhost, *.localhost) allowed by default —
        // traffic stays on the machine, not a data-exfiltration vector.
        if self.allow_loopback && Self::is_loopback_domain(&domain_lower) {
            return true;
        }

        // If allowlist is empty and allow_all_if_empty is true, allow
        if self.allowed_domains.is_empty() {
            return self.allow_all_if_empty;
        }

        // Check allowed list
        for allowed in &self.allowed_domains {
            if Self::domain_matches(&domain_lower, allowed) {
                return true;
            }
        }

        false
    }

    /// Check if a direct IP connection should be allowed.
    ///
    /// When domain filtering is active, raw IP addresses cannot be matched
    /// against domain patterns. This method attempts reverse DNS (PTR lookup)
    /// to resolve the IP to a hostname, then checks that hostname against the
    /// allowlist. If reverse DNS fails, the connection is blocked (fail-secure).
    pub fn is_ip_connection_allowed(&self, ip_str: &str) -> bool {
        // Check denied list with the raw IP first
        for denied in &self.denied_domains {
            if Self::domain_matches(ip_str, denied) {
                return false;
            }
        }

        // Loopback IPs (127.0.0.0/8, ::1) allowed by default — same
        // rationale as loopback domains: traffic never leaves the host.
        if self.allow_loopback {
            if let Ok(ip) = ip_str.parse::<std::net::IpAddr>() {
                if ip.is_loopback() {
                    return true;
                }
            }
        }

        // No specific domain filtering → fall back to standard logic
        if self.allowed_domains.is_empty() {
            return self.allow_all_if_empty;
        }

        // Wildcard "*" allows all — no reverse DNS needed
        if self.allowed_domains.iter().any(|d| d.trim() == "*") {
            return true;
        }

        // Domain filtering is active — attempt reverse DNS
        let ip: std::net::IpAddr = match ip_str.parse() {
            Ok(ip) => ip,
            Err(_) => return false,
        };

        match reverse_dns_lookup(&ip) {
            Some(ref domain) => self.is_domain_allowed(domain),
            None => false, // Fail-secure: no PTR record → block
        }
    }

    /// Check if a domain matches a pattern (supports wildcards)
    /// Pattern may include an optional `:port` suffix which is stripped before matching.
    /// e.g. "*:80" matches all domains, "*.github.com:443" matches sub.github.com
    fn domain_matches(domain: &str, pattern: &str) -> bool {
        let pattern_lower = pattern.to_lowercase().trim().to_string();
        
        // Strip :port suffix if present (e.g. "*:80" → "*", "*.example.com:443" → "*.example.com")
        let pattern_clean = if let Some(colon_pos) = pattern_lower.rfind(':') {
            let after_colon = &pattern_lower[colon_pos + 1..];
            if !after_colon.is_empty() && after_colon.chars().all(|c| c.is_ascii_digit()) {
                &pattern_lower[..colon_pos]
            } else {
                &pattern_lower
            }
        } else {
            &pattern_lower
        };

        // Single "*" matches all domains
        if pattern_clean == "*" {
            return true;
        }
        
        if pattern_clean.starts_with("*.") {
            // Wildcard pattern: *.example.com matches sub.example.com and example.com
            let suffix = &pattern_clean[1..]; // .example.com
            let base = &pattern_clean[2..];   // example.com
            domain.ends_with(suffix) || domain == base
        } else {
            domain == pattern_clean
        }
    }
}

// ============================================================================
// Shared Tunneling
// ============================================================================

/// Bidirectionally tunnel data between two TCP streams.
///
/// Spawns two threads — one for each direction — and waits for both to finish.
/// Parameters allow callers to control buffer size, read timeout, and whether
/// Nagle's algorithm is disabled (nodelay).
fn tunnel_data(
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
        let _ = s2_write.shutdown(Shutdown::Write);
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
        let _ = s1_write.shutdown(Shutdown::Write);
    });

    let _ = handle1.join();
    let _ = handle2.join();

    Ok(())
}

// ============================================================================
// HTTP Proxy Server
// ============================================================================

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
        // Bind to a random available port on localhost
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        
        // Set non-blocking for graceful shutdown
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
        let listener = self.listener.take()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "Proxy already started"))?;
        
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

    /// Run the HTTP proxy server
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
                    // No connection available, sleep briefly
                    thread::sleep(Duration::from_millis(10));
                }
                Err(e) => {
                    tracing::error!("[HTTP Proxy] Accept error: {}", e);
                }
            }
        }
    }

    /// Handle a single HTTP proxy client connection
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

        // Handle CONNECT method (HTTPS tunneling)
        if method == "CONNECT" {
            return Self::handle_connect(&mut client, &mut reader, target, &config);
        }

        // Handle regular HTTP requests
        Self::handle_http_request(&mut client, &mut reader, method, target, &request_line, &config)
    }

    /// Handle CONNECT method for HTTPS tunneling
    fn handle_connect(
        client: &mut TcpStream,
        reader: &mut BufReader<TcpStream>,
        target: &str,
        config: &Arc<RwLock<ProxyConfig>>,
    ) -> std::io::Result<()> {
        // Parse host:port
        let (host, port) = Self::parse_host_port(target, 443)?;

        // Check if domain is allowed
        {
            let cfg = config.read().expect("proxy config lock");
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

        // Read and discard remaining headers
        loop {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            if line.trim().is_empty() {
                break;
            }
        }

        // Connect to target
        let target_addr = format!("{}:{}", host, port);
        let mut target_stream = match TcpStream::connect_timeout(
            &target_addr.to_socket_addrs()?.next()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "Could not resolve host"))?,
            Duration::from_secs(30),
        ) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("[HTTP Proxy] Failed to connect to {}: {}", target_addr, e);
                return Self::send_error(client, 502, &format!("Bad Gateway - {}", e));
            }
        };

        // Send 200 Connection Established
        client.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")?;
        client.flush()?;

        // Set timeouts for tunneling
        client.set_read_timeout(Some(Duration::from_secs(120)))?;
        client.set_write_timeout(Some(Duration::from_secs(120)))?;
        target_stream.set_read_timeout(Some(Duration::from_secs(120)))?;
        target_stream.set_write_timeout(Some(Duration::from_secs(120)))?;

        // Tunnel data between client and target (32KB buffer, 120s, nodelay for SSL)
        tunnel_data(client, &mut target_stream, 32768, Duration::from_secs(120), true)
    }

    /// Handle regular HTTP requests
    fn handle_http_request(
        client: &mut TcpStream,
        reader: &mut BufReader<TcpStream>,
        method: &str,
        target: &str,
        _request_line: &str,
        config: &Arc<RwLock<ProxyConfig>>,
    ) -> std::io::Result<()> {
        // Parse URL to extract host
        let host = if target.starts_with("http://") {
            let url = &target[7..];
            url.split('/').next().unwrap_or("")
                .split(':').next().unwrap_or("")
                .to_string()
        } else {
            return Self::send_error(client, 400, "Bad Request - Invalid URL");
        };

        // Check if domain is allowed
        {
            let cfg = config.read().expect("proxy config lock");
            if !cfg.is_domain_allowed(&host) {
                observability::security_blocked_network(
                    "unknown",
                    &host,
                    "domain_not_in_allowlist",
                );
                return Self::send_error(client, 403, "Forbidden - Domain not in allowlist");
            }
        }

        // Read headers
        let mut headers = Vec::new();
        loop {
            let mut line = String::new();
            reader.read_line(&mut line)?;
            if line.trim().is_empty() {
                break;
            }
            // Skip Proxy-* headers
            if !line.to_lowercase().starts_with("proxy-") {
                headers.push(line);
            }
        }

        // Parse host:port from target URL
        let (target_host, target_port) = Self::parse_url_host_port(target)?;
        let target_addr = format!("{}:{}", target_host, target_port);

        // Connect to target
        let mut target_stream = match TcpStream::connect_timeout(
            &target_addr.to_socket_addrs()?.next()
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "Could not resolve host"))?,
            Duration::from_secs(30),
        ) {
            Ok(s) => s,
            Err(e) => {
                return Self::send_error(client, 502, &format!("Bad Gateway - {}", e));
            }
        };

        // Convert absolute URL to relative path
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

        // Forward request
        let request = format!("{} {} HTTP/1.1\r\n", method, path);
        target_stream.write_all(request.as_bytes())?;
        for header in &headers {
            target_stream.write_all(header.as_bytes())?;
        }
        target_stream.write_all(b"\r\n")?;
        target_stream.flush()?;

        // Forward response (32KB buffer, 120s, nodelay for SSL)
        tunnel_data(&mut target_stream, client, 32768, Duration::from_secs(120), true)
    }

    /// Send an HTTP error response
    fn send_error(client: &mut TcpStream, code: u16, message: &str) -> std::io::Result<()> {
        let response = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: text/plain\r\nConnection: close\r\n\r\n{}\r\n",
            code, message, message
        );
        client.write_all(response.as_bytes())?;
        client.flush()?;
        Ok(())
    }

    /// Parse host:port from a string
    fn parse_host_port(s: &str, default_port: u16) -> std::io::Result<(String, u16)> {
        if let Some(pos) = s.rfind(':') {
            let host = s[..pos].to_string();
            let port = s[pos + 1..].parse::<u16>()
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid port"))?;
            Ok((host, port))
        } else {
            Ok((s.to_string(), default_port))
        }
    }

    /// Parse host:port from a URL
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

// ============================================================================
// SOCKS5 Proxy Server
// ============================================================================

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
        // Bind to a random available port on localhost
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        
        // Set non-blocking for graceful shutdown
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
        let listener = self.listener.take()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "Proxy already started"))?;
        
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

    /// Run the SOCKS5 proxy server
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

    /// Handle a single SOCKS5 client connection
    fn handle_client(
        mut client: TcpStream,
        _addr: SocketAddr,
        config: Arc<RwLock<ProxyConfig>>,
    ) -> std::io::Result<()> {
        client.set_read_timeout(Some(Duration::from_secs(30)))?;
        client.set_write_timeout(Some(Duration::from_secs(30)))?;

        // SOCKS5 handshake
        let mut buf = [0u8; 256];
        
        // Read version and auth methods
        client.read_exact(&mut buf[..2])?;
        if buf[0] != 0x05 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid SOCKS version"));
        }
        
        let nmethods = buf[1] as usize;
        client.read_exact(&mut buf[..nmethods])?;
        
        // We only support no authentication (0x00)
        let has_no_auth = buf[..nmethods].contains(&0x00);
        if !has_no_auth {
            // Send "no acceptable methods"
            client.write_all(&[0x05, 0xFF])?;
            return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "No acceptable auth method"));
        }
        
        // Send "no authentication required"
        client.write_all(&[0x05, 0x00])?;
        
        // Read connection request
        client.read_exact(&mut buf[..4])?;
        if buf[0] != 0x05 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid SOCKS version"));
        }
        
        let cmd = buf[1];
        if cmd != 0x01 {
            // Only CONNECT (0x01) is supported
            Self::send_reply(&mut client, 0x07)?; // Command not supported
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Only CONNECT is supported"));
        }
        
        let atyp = buf[3];
        let (host, port) = match atyp {
            0x01 => {
                // IPv4
                client.read_exact(&mut buf[..4])?;
                let ip = format!("{}.{}.{}.{}", buf[0], buf[1], buf[2], buf[3]);
                client.read_exact(&mut buf[..2])?;
                let port = u16::from_be_bytes([buf[0], buf[1]]);
                (ip, port)
            }
            0x03 => {
                // Domain name
                client.read_exact(&mut buf[..1])?;
                let len = buf[0] as usize;
                client.read_exact(&mut buf[..len])?;
                let domain = String::from_utf8_lossy(&buf[..len]).to_string();
                client.read_exact(&mut buf[..2])?;
                let port = u16::from_be_bytes([buf[0], buf[1]]);
                (domain, port)
            }
            0x04 => {
                // IPv6
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
                Self::send_reply(&mut client, 0x08)?; // Address type not supported
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid address type"));
            }
        };

        // Check if connection is allowed (F5: reverse DNS for IP direct connections)
        {
            let cfg = config.read().expect("proxy config lock");
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
                observability::security_blocked_network(
                    "unknown",
                    &blocked_target,
                    reason,
                );
                Self::send_reply(&mut client, 0x02)?; // Connection not allowed
                return Ok(());
            }
        }

        // Connect to target
        let target_addr = format!("{}:{}", host, port);
        let target_stream = match target_addr.to_socket_addrs() {
            Ok(mut addrs) => {
                if let Some(addr) = addrs.next() {
                    match TcpStream::connect_timeout(&addr, Duration::from_secs(30)) {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::warn!("[SOCKS5 Proxy] Failed to connect to {}: {}", target_addr, e);
                            Self::send_reply(&mut client, 0x05)?; // Connection refused
                            return Ok(());
                        }
                    }
                } else {
                    Self::send_reply(&mut client, 0x04)?; // Host unreachable
                    return Ok(());
                }
            }
            Err(e) => {
                tracing::warn!("[SOCKS5 Proxy] Failed to resolve {}: {}", host, e);
                Self::send_reply(&mut client, 0x04)?; // Host unreachable
                return Ok(());
            }
        };

        // Send success reply
        Self::send_reply(&mut client, 0x00)?;

        // Tunnel data (8KB buffer, 60s timeout, no nodelay)
        let mut target = target_stream;
        tunnel_data(&mut client, &mut target, 8192, Duration::from_secs(60), false)
    }

    /// Send SOCKS5 reply
    fn send_reply(client: &mut TcpStream, rep: u8) -> std::io::Result<()> {
        // Reply: VER REP RSV ATYP BND.ADDR BND.PORT
        let reply = [
            0x05, // VER
            rep,  // REP
            0x00, // RSV
            0x01, // ATYP (IPv4)
            0x00, 0x00, 0x00, 0x00, // BND.ADDR (0.0.0.0)
            0x00, 0x00, // BND.PORT (0)
        ];
        client.write_all(&reply)?;
        client.flush()
    }


}

// ============================================================================
// Proxy Manager
// ============================================================================

/// Manages both HTTP and SOCKS5 proxies
pub struct ProxyManager {
    http_proxy: Option<HttpProxy>,
    socks5_proxy: Option<Socks5Proxy>,
    http_handle: Option<thread::JoinHandle<()>>,
    socks5_handle: Option<thread::JoinHandle<()>>,
}

impl ProxyManager {
    /// Create a new proxy manager with the given configuration
    pub fn new(config: ProxyConfig) -> std::io::Result<Self> {
        let http_proxy = HttpProxy::new(config.clone())?;
        let socks5_proxy = Socks5Proxy::new(config)?;

        Ok(Self {
            http_proxy: Some(http_proxy),
            socks5_proxy: Some(socks5_proxy),
            http_handle: None,
            socks5_handle: None,
        })
    }

    /// Get the HTTP proxy port
    pub fn http_port(&self) -> Option<u16> {
        self.http_proxy.as_ref().map(|p| p.port())
    }

    /// Get the SOCKS5 proxy port
    pub fn socks5_port(&self) -> Option<u16> {
        self.socks5_proxy.as_ref().map(|p| p.port())
    }

    /// Start both proxies
    pub fn start(&mut self) -> std::io::Result<()> {
        if let Some(ref mut http) = self.http_proxy {
            self.http_handle = Some(http.start()?);
        }
        if let Some(ref mut socks5) = self.socks5_proxy {
            self.socks5_handle = Some(socks5.start()?);
        }
        Ok(())
    }

    /// Stop both proxies
    pub fn stop(&self) {
        if let Some(ref http) = self.http_proxy {
            http.stop();
        }
        if let Some(ref socks5) = self.socks5_proxy {
            socks5.stop();
        }
    }

    /// Generate environment variables for the sandboxed process
    pub fn get_proxy_env_vars(&self) -> Vec<(String, String)> {
        let mut vars = Vec::new();
        
        if let Some(port) = self.http_port() {
            let proxy_url = format!("http://127.0.0.1:{}", port);
            vars.push(("HTTP_PROXY".to_string(), proxy_url.clone()));
            vars.push(("http_proxy".to_string(), proxy_url.clone()));
            vars.push(("HTTPS_PROXY".to_string(), proxy_url.clone()));
            vars.push(("https_proxy".to_string(), proxy_url));
        }
        
        if let Some(port) = self.socks5_port() {
            let proxy_url = format!("socks5://127.0.0.1:{}", port);
            vars.push(("ALL_PROXY".to_string(), proxy_url.clone()));
            vars.push(("all_proxy".to_string(), proxy_url));
        }
        
        vars
    }
}

impl Drop for ProxyManager {
    fn drop(&mut self) {
        self.stop();
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_config_domain_matching() {
        let config = ProxyConfig {
            allowed_domains: vec![
                "github.com".to_string(),
                "*.github.com".to_string(),
                "api.example.com".to_string(),
            ],
            denied_domains: vec!["evil.github.com".to_string()],
            allow_all_if_empty: false,
            allow_loopback: true,
        };

        // Allowed domains
        assert!(config.is_domain_allowed("github.com"));
        assert!(config.is_domain_allowed("api.github.com"));
        assert!(config.is_domain_allowed("raw.github.com"));
        assert!(config.is_domain_allowed("api.example.com"));

        // Denied domains (takes precedence)
        assert!(!config.is_domain_allowed("evil.github.com"));

        // Not in allowlist
        assert!(!config.is_domain_allowed("google.com"));
        assert!(!config.is_domain_allowed("example.com"));
    }

    #[test]
    fn test_proxy_config_block_all() {
        let config = ProxyConfig::block_all();
        assert!(!config.is_domain_allowed("any-domain.com"));
        assert!(!config.is_domain_allowed("github.com"));
    }

    #[test]
    fn test_http_proxy_creation() {
        let config = ProxyConfig::default();
        let proxy = HttpProxy::new(config).expect("test HTTP proxy creation should succeed");
        assert!(proxy.port() > 0);
    }

    #[test]
    fn test_socks5_proxy_creation() {
        let config = ProxyConfig::default();
        let proxy = Socks5Proxy::new(config).expect("test SOCKS5 proxy creation should succeed");
        assert!(proxy.port() > 0);
    }

    #[test]
    fn test_proxy_manager() {
        let config = ProxyConfig::with_allowed_domains(vec!["github.com".to_string()]);
        let manager = ProxyManager::new(config).expect("test proxy manager creation should succeed");
        
        assert!(manager.http_port().is_some());
        assert!(manager.socks5_port().is_some());
        
        let env_vars = manager.get_proxy_env_vars();
        assert!(!env_vars.is_empty());
    }

    #[test]
    fn test_ip_direct_connection_blocked_with_domain_filter() {
        let config = ProxyConfig::with_allowed_domains(vec![
            "github.com".to_string(),
            "*.github.com".to_string(),
        ]);

        // Raw IPs should NOT pass domain matching
        assert!(!config.is_domain_allowed("140.82.112.4"));
        assert!(!config.is_domain_allowed("::1"));

        // is_ip_connection_allowed: no PTR record for RFC 5737 TEST-NET → blocked
        assert!(!config.is_ip_connection_allowed("192.0.2.1"));
    }

    #[test]
    fn test_ip_allowed_when_wildcard() {
        let config = ProxyConfig {
            allowed_domains: vec!["*".to_string()],
            denied_domains: vec![],
            allow_all_if_empty: false,
            allow_loopback: true,
        };

        // Wildcard allows all, including IP direct
        assert!(config.is_ip_connection_allowed("1.2.3.4"));
    }

    #[test]
    fn test_ip_allowed_when_no_filter() {
        let config = ProxyConfig {
            allowed_domains: vec![],
            denied_domains: vec![],
            allow_all_if_empty: true,
            allow_loopback: true,
        };

        // No domain filtering + allow_all → IP passes
        assert!(config.is_ip_connection_allowed("1.2.3.4"));
    }

    #[test]
    fn test_ip_blocked_when_block_all() {
        let config = ProxyConfig::block_all();
        assert!(!config.is_ip_connection_allowed("1.2.3.4"));
        // block_all sets allow_loopback = false
        assert!(!config.is_ip_connection_allowed("127.0.0.1"));
        assert!(!config.is_domain_allowed("localhost"));
    }

    #[test]
    fn test_loopback_allowed_by_default() {
        let config = ProxyConfig::with_allowed_domains(vec![
            "github.com".to_string(),
        ]);

        // Loopback addresses pass without needing explicit allowlist entry
        assert!(config.is_ip_connection_allowed("127.0.0.1"));
        assert!(config.is_ip_connection_allowed("127.0.0.2"));
        assert!(config.is_ip_connection_allowed("::1"));
        assert!(config.is_domain_allowed("localhost"));
        assert!(config.is_domain_allowed("app.localhost"));

        // External IPs still blocked
        assert!(!config.is_ip_connection_allowed("192.0.2.1"));
    }

    #[test]
    fn test_loopback_denied_takes_precedence() {
        let config = ProxyConfig {
            allowed_domains: vec!["github.com".to_string()],
            denied_domains: vec!["localhost".to_string()],
            allow_all_if_empty: false,
            allow_loopback: true,
        };

        // Denied list overrides allow_loopback for domains
        assert!(!config.is_domain_allowed("localhost"));

        // 127.0.0.1 as raw IP is not in denied_domains text,
        // but loopback IP is still allowed (denied only matches "localhost" text)
        assert!(config.is_ip_connection_allowed("127.0.0.1"));
    }

    #[test]
    fn test_loopback_ip_denied_by_ip_pattern() {
        let config = ProxyConfig {
            allowed_domains: vec!["github.com".to_string()],
            denied_domains: vec!["127.0.0.1".to_string()],
            allow_all_if_empty: false,
            allow_loopback: true,
        };

        // IP in denied list → blocked even with allow_loopback
        assert!(!config.is_ip_connection_allowed("127.0.0.1"));
        // Other loopback IPs still allowed
        assert!(config.is_ip_connection_allowed("127.0.0.2"));
    }
}
