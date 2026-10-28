//! Resolve `https_proxy` on the host before a sandbox starts.

use std::net::{IpAddr, SocketAddr, ToSocketAddrs};

use crate::error::bail;
use crate::Result;

/// Proxy endpoints the sandbox is allowed to contact, plus the URL exported
/// to the process. The exported URL uses a resolved address so the sandbox
/// does not need DNS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProxy {
    pub endpoints: Vec<SocketAddr>,
    pub env_url: String,
}

/// Parse and resolve a proxy URL. Userinfo is kept in `env_url` and is not
/// included in error text.
pub fn resolve_proxy(raw: &str) -> Result<ResolvedProxy> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("https_proxy is required");
    }
    let (scheme, rest) = if let Some(rest) = trimmed.strip_prefix("http://") {
        ("http", rest)
    } else if let Some(rest) = trimmed.strip_prefix("https://") {
        ("https", rest)
    } else {
        bail!("https_proxy must start with http:// or https://");
    };
    let (userinfo, hostport) = match rest.rsplit_once('@') {
        Some((user, host)) if !user.is_empty() && !host.is_empty() => (Some(user), host),
        _ => (None, rest),
    };
    if hostport.is_empty() {
        bail!("https_proxy is missing a host");
    }
    let default_port: u16 = if scheme == "http" { 80 } else { 443 };
    let (host, port) = split_host_port(hostport, default_port)?;
    let mut endpoints: Vec<SocketAddr> = format!("{host}:{port}")
        .to_socket_addrs()
        .map_err(|_| crate::Error::validation("https_proxy host could not be resolved"))?
        .filter(|addr| addr.port() == port)
        .collect();
    endpoints.sort_by_key(|addr| (addr.ip().to_string(), addr.port()));
    endpoints.dedup();
    if endpoints.is_empty() {
        bail!("https_proxy host could not be resolved");
    }
    let chosen = &endpoints[0];
    let host_for_env = match chosen.ip() {
        IpAddr::V4(ip) => ip.to_string(),
        IpAddr::V6(ip) => format!("[{ip}]"),
    };
    let env_url = match userinfo {
        Some(user) => format!("{scheme}://{user}@{host_for_env}:{}", chosen.port()),
        None => format!("{scheme}://{host_for_env}:{}", chosen.port()),
    };
    Ok(ResolvedProxy { endpoints, env_url })
}

fn split_host_port(hostport: &str, default_port: u16) -> Result<(String, u16)> {
    if let Some(rest) = hostport.strip_prefix('[') {
        let Some((host, port_part)) = rest.split_once(']') else {
            bail!("https_proxy has an invalid IPv6 host");
        };
        let port = if let Some(port) = port_part.strip_prefix(':') {
            parse_port(port)?
        } else if port_part.is_empty() {
            default_port
        } else {
            bail!("https_proxy has an invalid IPv6 host");
        };
        if host.is_empty() {
            bail!("https_proxy is missing a host");
        }
        return Ok((host.to_string(), port));
    }
    match hostport.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() && !port.is_empty() && !host.contains(':') => {
            Ok((host.to_string(), parse_port(port)?))
        }
        _ => {
            if hostport.is_empty() || hostport.contains(':') {
                bail!("https_proxy is missing a host");
            }
            Ok((hostport.to_string(), default_port))
        }
    }
}

fn parse_port(raw: &str) -> Result<u16> {
    raw.parse::<u16>()
        .map_err(|_| crate::Error::validation("https_proxy has an invalid port"))
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

    use super::*;

    #[test]
    fn rejects_missing_proxy() {
        let err = resolve_proxy("  ").unwrap_err();
        assert!(err.to_string().contains("required"));
    }

    #[test]
    fn rejects_non_http_scheme() {
        let err = resolve_proxy("socks5://127.0.0.1:1080").unwrap_err();
        assert!(err.to_string().contains("http://"));
    }

    #[test]
    fn resolves_loopback_without_dns_name() {
        let proxy = resolve_proxy("http://127.0.0.1:9").unwrap();
        assert_eq!(
            proxy.endpoints,
            vec![SocketAddr::from((Ipv4Addr::new(127, 0, 0, 1), 9))]
        );
        assert_eq!(proxy.env_url, "http://127.0.0.1:9");
        assert!(proxy.endpoints.iter().any(|endpoint| {
            endpoint.ip() == IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)) && endpoint.port() == 9
        }));
        assert!(!proxy
            .endpoints
            .iter()
            .any(|endpoint| { endpoint.ip() == IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)) }));
    }

    #[test]
    fn keeps_userinfo_out_of_the_error_path_and_in_the_env_url() {
        let proxy = resolve_proxy("http://user:secret@127.0.0.1:9").unwrap();
        assert_eq!(proxy.env_url, "http://user:secret@127.0.0.1:9");
        assert!(!proxy.endpoints[0].ip().to_string().contains("secret"));
    }

    #[test]
    fn brackets_ipv6_env_host() {
        let proxy = resolve_proxy("http://[::1]:9").unwrap();
        assert_eq!(
            proxy.endpoints,
            vec![SocketAddr::from((Ipv6Addr::LOCALHOST, 9))]
        );
        assert_eq!(proxy.env_url, "http://[::1]:9");
    }
}
