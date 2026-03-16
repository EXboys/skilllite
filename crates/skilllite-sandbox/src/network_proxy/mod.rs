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

mod config;
mod dns;
mod http;
mod manager;
mod socks5;
mod tunnel;

#[cfg(test)]
mod tests;

pub use config::ProxyConfig;
pub use http::HttpProxy;
pub use manager::ProxyManager;
pub use socks5::Socks5Proxy;
