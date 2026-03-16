//! Manages both HTTP and SOCKS5 proxies.

use super::config::ProxyConfig;
use super::http::HttpProxy;
use super::socks5::Socks5Proxy;

/// Manages both HTTP and SOCKS5 proxies
pub struct ProxyManager {
    http_proxy: Option<HttpProxy>,
    socks5_proxy: Option<Socks5Proxy>,
    http_handle: Option<std::thread::JoinHandle<()>>,
    socks5_handle: Option<std::thread::JoinHandle<()>>,
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
