//! SwarmHandler — full daemon loop: mDNS register, browse, block until shutdown.

use anyhow::{Context, Result};
use mdns_sd::ServiceEvent;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::discovery::{parse_capabilities_from_txt, Discovery};

/// Parse listen address "host:port" into (host, port).
fn parse_listen_addr(addr: &str) -> Result<(String, u16)> {
    let parts: Vec<&str> = addr.splitn(2, ':').collect();
    let (host, port_str) = match parts.as_slice() {
        [h, p] => (*h, *p),
        [p] if p.parse::<u16>().is_ok() => ("0.0.0.0", *p),
        _ => anyhow::bail!("Invalid listen address: expected host:port or :port, got {}", addr),
    };
    let port: u16 = port_str.parse().context("Invalid port number")?;
    Ok((host.to_string(), port))
}

/// Run the swarm daemon: register via mDNS, browse for peers, block until Ctrl+C.
pub fn serve_swarm(listen_addr: &str, capability_tags: Vec<String>) -> Result<()> {
    let (host, port) = parse_listen_addr(listen_addr)?;
    let instance_name = uuid::Uuid::new_v4().to_string();

    let discovery = Discovery::new()?;
    discovery.register(&instance_name, &host, port, &capability_tags)?;

    let browse_rx = discovery.browse()?;
    let shutdown = Arc::new(AtomicBool::new(false));

    // Handle Ctrl+C
    let shutdown_clone = shutdown.clone();
    ctrlc::set_handler(move || {
        tracing::info!("Received Ctrl+C, shutting down swarm...");
        shutdown_clone.store(true, Ordering::SeqCst);
    })
    .context("Failed to set Ctrl+C handler")?;

    // Spawn browse loop
    let shutdown_browse = shutdown.clone();
    std::thread::spawn(move || {
        while !shutdown_browse.load(Ordering::SeqCst) {
            match browse_rx.recv_timeout(Duration::from_millis(500)) {
                Ok(ServiceEvent::ServiceResolved(resolved)) => {
                    let caps = parse_capabilities_from_txt(&resolved.txt_properties);
                    let addr = resolved
                        .addresses
                        .iter()
                        .next()
                        .map(|a| format!("{}:{}", a, resolved.port))
                        .unwrap_or_else(|| format!("{}:{}", resolved.host, resolved.port));
                    tracing::info!(
                        peer = %resolved.fullname,
                        addr = %addr,
                        capabilities = ?caps,
                        "Discovered swarm peer"
                    );
                }
                Ok(ServiceEvent::ServiceFound(_, _)) => {
                    // Will be followed by ServiceResolved
                }
                Ok(ServiceEvent::ServiceRemoved(_, _)) => {
                    // Peer left
                }
                Ok(_) => {}
                Err(_) => {
                    // Timeout, loop again
                }
            }
        }
    });

    tracing::info!(
        listen = %listen_addr,
        instance = %instance_name,
        "Swarm daemon running (mDNS discovery active). Press Ctrl+C to stop."
    );

    // Block until shutdown
    while !shutdown.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(200));
    }

    discovery.shutdown()?;
    tracing::info!("Swarm daemon stopped");
    Ok(())
}
