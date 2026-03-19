//! Network proxy tests.

use super::config::ProxyConfig;
use super::http::HttpProxy;
use super::manager::ProxyManager;
use super::socks5::Socks5Proxy;

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

    assert!(config.is_domain_allowed("github.com"));
    assert!(config.is_domain_allowed("api.github.com"));
    assert!(config.is_domain_allowed("raw.github.com"));
    assert!(config.is_domain_allowed("api.example.com"));

    assert!(!config.is_domain_allowed("evil.github.com"));

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

    assert!(!config.is_domain_allowed("140.82.112.4"));
    assert!(!config.is_domain_allowed("::1"));

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

    assert!(config.is_ip_connection_allowed("1.2.3.4"));
}

#[test]
fn test_ip_blocked_when_block_all() {
    let config = ProxyConfig::block_all();
    assert!(!config.is_ip_connection_allowed("1.2.3.4"));
    assert!(!config.is_ip_connection_allowed("127.0.0.1"));
    assert!(!config.is_domain_allowed("localhost"));
}

#[test]
fn test_loopback_allowed_by_default() {
    let config = ProxyConfig::with_allowed_domains(vec!["github.com".to_string()]);

    assert!(config.is_ip_connection_allowed("127.0.0.1"));
    assert!(config.is_ip_connection_allowed("127.0.0.2"));
    assert!(config.is_ip_connection_allowed("::1"));
    assert!(config.is_domain_allowed("localhost"));
    assert!(config.is_domain_allowed("app.localhost"));

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

    assert!(!config.is_domain_allowed("localhost"));
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

    assert!(!config.is_ip_connection_allowed("127.0.0.1"));
    assert!(config.is_ip_connection_allowed("127.0.0.2"));
}
