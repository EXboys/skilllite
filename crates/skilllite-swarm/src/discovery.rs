//! mDNS Discovery — advertise node capabilities and discover peers.
//!
//! Uses `_skilllite-swarm._udp.local.` service type for SkillLite P2P nodes.
//! TXT record `capabilities` = JSON array of capability tags.

use anyhow::{Context, Result};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// SkillLite swarm mDNS service type (RFC 6763: _service._proto.local.)
pub const SERVICE_TYPE: &str = "_skilllite-swarm._udp.local.";

/// Discovered peer node info.
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// Instance name (unique per node, e.g. hostname or UUID)
    pub instance_name: String,
    /// Resolved address (host:port)
    pub addr: String,
    /// Capability tags advertised by the peer
    pub capabilities: Vec<String>,
}

/// mDNS Discovery: register self and browse for peers.
pub struct Discovery {
    daemon: ServiceDaemon,
    shutdown: Arc<AtomicBool>,
}

impl Discovery {
    /// Create a new Discovery daemon.
    pub fn new() -> Result<Self> {
        let daemon = ServiceDaemon::new().context("Failed to create mDNS daemon")?;
        Ok(Self {
            daemon,
            shutdown: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Register this node on the network.
    ///
    /// - `instance_name`: Unique name (e.g. hostname or UUID)
    /// - `host`: IP or hostname (e.g. "192.168.1.10" or "0.0.0.0" → use local IP)
    /// - `port`: Listen port
    /// - `capabilities`: Capability tags from skills
    pub fn register(
        &self,
        instance_name: &str,
        host: &str,
        port: u16,
        capabilities: &[String],
    ) -> Result<()> {
        let caps_json = serde_json::to_string(capabilities).unwrap_or_else(|_| "[]".to_string());
        let properties: Vec<(&str, &str)> = vec![("capabilities", caps_json.as_str())];

        let host_name = format!("{}.local.", instance_name);
        let ip = if host == "0.0.0.0" || host.is_empty() {
            local_ip_address::local_ip()
                .map(|a| a.to_string())
                .unwrap_or_else(|_| "127.0.0.1".to_string())
        } else {
            host.to_string()
        };

        let service = ServiceInfo::new(
            SERVICE_TYPE,
            instance_name,
            &host_name,
            &ip,
            port,
            &properties[..],
        )
        .context("Invalid ServiceInfo")?;

        self.daemon.register(service).context("Failed to register mDNS service")?;
        tracing::info!(
            instance = %instance_name,
            addr = %format!("{}:{}", ip, port),
            capabilities = ?capabilities,
            "Registered swarm node via mDNS"
        );
        Ok(())
    }

    /// Browse for peer nodes. Returns a receiver for `ServiceEvent`s.
    pub fn browse(&self) -> Result<mdns_sd::Receiver<ServiceEvent>> {
        self.daemon
            .browse(SERVICE_TYPE)
            .context("Failed to browse for swarm peers")
    }

    /// Shutdown the daemon.
    pub fn shutdown(&self) -> Result<()> {
        self.shutdown.store(true, Ordering::SeqCst);
        let _rx = self.daemon.shutdown().context("Failed to shutdown mDNS daemon")?;
        Ok(())
    }
}

/// Parse capabilities from mDNS TXT record.
pub fn parse_capabilities_from_txt(txt: &mdns_sd::TxtProperties) -> Vec<String> {
    txt.get_property_val_str("capabilities")
        .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
        .unwrap_or_default()
}
