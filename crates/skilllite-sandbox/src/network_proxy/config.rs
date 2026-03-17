//! Proxy configuration and domain allowlist/denylist logic.

use super::dns;

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

        match dns::reverse_dns_lookup(&ip) {
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
            let base = &pattern_clean[2..]; // example.com
            domain.ends_with(suffix) || domain == base
        } else {
            domain == pattern_clean
        }
    }
}
