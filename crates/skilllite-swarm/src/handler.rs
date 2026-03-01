//! SwarmHandler — full daemon loop: mDNS register, browse, HTTP task API, block until shutdown.
//!
//! Phase 3 routing:
//! - POST /task: receive NodeTask, match capabilities, execute locally or forward to peer.

use anyhow::{Context, Result};
use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bytes::Bytes;
use futures_util::stream::{self, StreamExt};
use mdns_sd::ServiceEvent;
use skilllite_core::protocol::{NodeResult, NodeTask};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::discovery::{parse_capabilities_from_txt, Discovery};
use crate::routing::{route_task, RouteTarget, TaskExecutor};

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

/// Shared state for the HTTP server.
#[derive(Clone)]
struct AppState {
    local_capabilities: Vec<String>,
    peers: Arc<std::sync::Mutex<Vec<crate::discovery::PeerInfo>>>,
    executor: Option<Arc<dyn TaskExecutor>>,
    /// Current task being executed (for GET /status feedback).
    current_task: Arc<std::sync::Mutex<Option<String>>>,
}

/// GET /status — execution status for client polling (avoids "empty wait" UX).
async fn handle_status(State(state): State<AppState>) -> impl IntoResponse {
    let task_id = state.current_task.lock().map(|g| g.clone()).unwrap_or(None);
    let (status, task_id_val) = match &task_id {
        Some(id) => ("busy", serde_json::Value::String(id.clone())),
        None => ("idle", serde_json::Value::Null),
    };
    (StatusCode::OK, Json(serde_json::json!({ "status": status, "current_task_id": task_id_val })))
}

#[derive(serde::Deserialize, Default)]
struct TaskQuery {
    #[serde(rename = "stream")]
    stream: Option<u8>,
}

/// POST /task — receive NodeTask, route, execute or forward.
/// Add ?stream=1 for NDJSON progress (received → executing → done).
async fn handle_task(
    State(state): State<AppState>,
    Query(query): Query<TaskQuery>,
    Json(task): Json<NodeTask>,
) -> impl IntoResponse {
    let peers = state.peers.lock().map(|p| p.clone()).unwrap_or_default();

    // When required_capabilities is empty, optionally infer via LLM (SKILLLITE_SWARM_LLM_ROUTING=1)
    let required = if task.context.required_capabilities.is_empty() {
        let all_tags: Vec<String> = {
            let mut s = std::collections::HashSet::new();
            s.extend(state.local_capabilities.iter().cloned());
            for p in &peers {
                s.extend(p.capabilities.iter().cloned());
            }
            let mut v: Vec<_> = s.into_iter().collect();
            v.sort();
            v
        };
        let inferred = crate::llm_routing::infer_required_capabilities(
            &task.description,
            &all_tags,
        )
        .await;
        if inferred.is_empty() {
            task.context.required_capabilities.clone()
        } else {
            inferred
        }
    } else {
        task.context.required_capabilities.clone()
    };

    // Route with effective required_capabilities
    let mut task_for_route = task.clone();
    task_for_route.context.required_capabilities = required.clone();
    let target = route_task(&task_for_route, &state.local_capabilities, &peers);

    // Log routing decision for observability
    match &target {
        RouteTarget::Local => {
            tracing::info!(
                task_id = %task.id,
                local_caps = ?state.local_capabilities,
                required = ?required,
                "Routing: LOCAL (capabilities match)"
            );
        }
        RouteTarget::Forward(peers) => {
            let peer = &peers[0];
            tracing::info!(
                task_id = %task.id,
                peer = %peer.instance_name,
                peer_addr = %peer.addr,
                peer_caps = ?peer.capabilities,
                required = ?required,
                fallback_count = peers.len().saturating_sub(1),
                "Routing: FORWARD to peer"
            );
        }
        RouteTarget::NoMatch => {
            tracing::info!(
                task_id = %task.id,
                required = ?required,
                local_caps = ?state.local_capabilities,
                peer_count = peers.len(),
                "Routing: NO_MATCH (no local or peer has required capabilities)"
            );
        }
    }

    match target {
        RouteTarget::Local => {
            let task_id = task.id.clone();
            tracing::info!(task_id = %task_id, "Task received, executing locally...");
            if let Ok(mut cur) = state.current_task.lock() {
                *cur = Some(task_id.clone());
            }
            let Some(ref exec) = state.executor else {
                if let Ok(mut cur) = state.current_task.lock() {
                    *cur = None;
                }
                tracing::warn!(task_id = %task_id, "Local execution requested but no TaskExecutor configured");
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(serde_json::json!({
                        "error": "local_executor_not_configured",
                        "message": "Swarm received task but agent/executor not wired. Build with --features agent."
                    })),
                )
                    .into_response();
            };
            if query.stream == Some(1) {
                // Stream progress: received → executing → done
                let exec = exec.clone();
                let task = task.clone();
                let task_id = task_id.clone();
                let current_task = state.current_task.clone();
                let s1 = stream::iter([
                    Ok::<_, anyhow::Error>(Bytes::from(
                        format!("{{\"event\":\"received\",\"task_id\":\"{}\"}}\n", task_id),
                    )),
                    Ok(Bytes::from("{\"event\":\"executing\"}\n")),
                ]);
                let s2 = stream::once(async move {
                    let result = tokio::task::spawn_blocking(move || exec.execute(task))
                        .await
                        .map_err(|e| anyhow::anyhow!("{:?}", e))?
                        .map_err(|e| anyhow::anyhow!("{}", e));
                    if let Ok(mut cur) = current_task.lock() {
                        *cur = None;
                    }
                    match result {
                        Ok(res) => {
                            let json = serde_json::to_string(&res).map_err(anyhow::Error::msg)?;
                            Ok(Bytes::from(format!("{{\"event\":\"done\",\"result\":{}}}\n", json)))
                        }
                        Err(e) => {
                            let json =
                                serde_json::json!({"error":"execution_failed","message":e.to_string()});
                            Ok(Bytes::from(format!("{{\"event\":\"error\",\"error\":{}}}\n", json)))
                        }
                    }
                });
                let body = Body::from_stream(s1.chain(s2));
                return Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "application/x-ndjson")
                    .body(body)
                    .unwrap()
                    .into_response();
            }
            let start = std::time::Instant::now();
            let exec = exec.clone();
            let result = tokio::task::spawn_blocking(move || exec.execute(task)).await;
            if let Ok(mut cur) = state.current_task.lock() {
                *cur = None;
            }
            let result = result
                .map_err(|e| anyhow::anyhow!("{:?}", e))
                .and_then(|r| r.map_err(|e| anyhow::anyhow!("{}", e)));
            match result {
                Ok(res) => {
                    tracing::info!(task_id = %task_id, elapsed_ms = start.elapsed().as_millis(), "Task completed");
                    (StatusCode::OK, Json(res)).into_response()
                }
                Err(e) => {
                    tracing::error!(task_id = %task_id, err = %e, elapsed_ms = start.elapsed().as_millis(), "Local execution failed");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({
                            "error": "execution_failed",
                            "message": e.to_string()
                        })),
                    )
                        .into_response()
                }
            }
        }
        RouteTarget::Forward(peers) => {
            tracing::info!(task_id = %task.id, "Task received, forwarding to peer...");
            let client = reqwest::Client::new();
            let mut last_err = String::new();
            for (i, peer) in peers.iter().enumerate() {
                let url = format!("http://{}/task", peer.addr);
                tracing::info!(
                    task_id = %task.id,
                    peer = %peer.instance_name,
                    peer_addr = %peer.addr,
                    attempt = i + 1,
                    total = peers.len(),
                    "Forwarding task to peer"
                );
                match client
                    .post(&url)
                    .json(&task)
                    .timeout(Duration::from_secs(120))
                    .send()
                    .await
                {
                    Ok(resp) if resp.status().is_success() => {
                        match resp.json::<NodeResult>().await {
                            Ok(result) => return (StatusCode::OK, Json(result)).into_response(),
                            Err(e) => {
                                last_err = e.to_string();
                                tracing::warn!(task_id = %task.id, peer = %peer.instance_name, "Peer returned invalid JSON: {}", last_err);
                            }
                        }
                    }
                    Ok(resp) => {
                        let status = resp.status();
                        let body = resp.text().await.unwrap_or_default();
                        last_err = format!("status={} body={}", status, body);
                        tracing::warn!(task_id = %task.id, peer = %peer.instance_name, status = %status, "Peer returned error: {}", body);
                    }
                    Err(e) => {
                        last_err = e.to_string();
                        tracing::warn!(task_id = %task.id, peer = %peer.instance_name, err = %e, "Forward to peer failed, trying next");
                    }
                }
            }
            tracing::warn!(task_id = %task.id, "All peers failed for forward");
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "error": "forward_failed",
                    "message": last_err
                })),
            )
                .into_response()
        }
        RouteTarget::NoMatch => {
            tracing::info!(
                task_id = %task.id,
                required = ?task.context.required_capabilities,
                "No matching node for task"
            );
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "no_match",
                    "message": "No local or peer node has required capabilities",
                    "required_capabilities": task.context.required_capabilities
                })),
            )
                .into_response()
        }
    }
}

/// Run the swarm daemon: register via mDNS, browse for peers, serve HTTP task API, block until Ctrl+C.
///
/// - `executor`: Optional. When set, local tasks are executed via this; otherwise returns 503.
/// - Sets `SKILLLITE_SWARM_URL` so agent's delegate_to_swarm can route to this swarm (skill sharing).
/// - When `skills_dir` is set, loads .env from its parent (project root) so OPENAI_API_KEY is available for LLM routing.
pub fn serve_swarm(
    listen_addr: &str,
    capability_tags: Vec<String>,
    skills_dir: Option<&[String]>,
    executor: Option<Arc<dyn TaskExecutor>>,
) -> Result<()> {
    // Load .env from project root (parent of skills_dir) so LLM routing works when started from different cwd
    if let Some(dirs) = skills_dir.and_then(|d| d.first()) {
        let path = std::path::Path::new(dirs);
        let mut tried = false;
        if let Ok(canonical) = path.canonicalize() {
            for dir in canonical.ancestors().take(3) {
                let env_path = dir.join(".env");
                if env_path.exists() {
                    skilllite_core::config::load_dotenv_from_dir(dir);
                    tracing::debug!(env_dir = ?dir, "Loaded .env from project for LLM routing");
                    tried = true;
                    break;
                }
            }
        }
        if !tried {
            path.parent().map(|p| skilllite_core::config::load_dotenv_from_dir(p));
        }
    }

    let (host, port) = parse_listen_addr(listen_addr)?;
    let instance_name = uuid::Uuid::new_v4().to_string();

    // Enable delegate_to_swarm to route to this node (for skill sharing)
    let swarm_url = format!("http://127.0.0.1:{}", port);
    if std::env::var("SKILLLITE_SWARM_URL").is_err() {
        std::env::set_var("SKILLLITE_SWARM_URL", &swarm_url);
    }

    let discovery = Discovery::new()?;
    discovery.register(&instance_name, &host, port, &capability_tags)?;

    let browse_rx = discovery.browse()?;
    let peers: Arc<std::sync::Mutex<Vec<crate::discovery::PeerInfo>>> = Arc::new(std::sync::Mutex::new(Vec::new()));

    // Spawn browse loop — populate peers, exclude self
    let peers_browse = peers.clone();
    let my_instance = instance_name.clone();
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_browse = shutdown.clone();
    std::thread::spawn(move || {
        while !shutdown_browse.load(Ordering::SeqCst) {
            match browse_rx.recv_timeout(Duration::from_millis(500)) {
                Ok(ServiceEvent::ServiceResolved(resolved)) => {
                    let instance_name = resolved.fullname.split('.').next().unwrap_or("").to_string();
                    if instance_name == my_instance {
                        continue; // skip self
                    }
                    let caps = parse_capabilities_from_txt(&resolved.txt_properties);
                    let addr = resolved
                        .addresses
                        .iter()
                        .next()
                        .map(|a| format!("{}:{}", a, resolved.port))
                        .unwrap_or_else(|| format!("{}:{}", resolved.host, resolved.port));
                    let info = crate::discovery::PeerInfo {
                        instance_name: instance_name.clone(),
                        addr,
                        capabilities: caps.clone(),
                    };
                    if let Ok(mut p) = peers_browse.lock() {
                        if let Some(existing) = p.iter_mut().find(|x| x.instance_name == info.instance_name) {
                            *existing = info;
                        } else {
                            tracing::info!(
                                peer = %info.instance_name,
                                addr = %info.addr,
                                capabilities = ?info.capabilities,
                                "Peer discovered via mDNS"
                            );
                            p.push(info);
                        }
                    }
                }
                Ok(ServiceEvent::ServiceRemoved(name, _)) => {
                    if let Ok(mut p) = peers_browse.lock() {
                        p.retain(|x| !name.contains(&x.instance_name));
                    }
                }
                Ok(_) => {}
                Err(_) => {}
            }
        }
    });

    let state = AppState {
        local_capabilities: capability_tags.clone(),
        peers,
        executor,
        current_task: Arc::new(std::sync::Mutex::new(None)),
    };

    let app = Router::new()
        .route("/task", post(handle_task))
        .route("/status", get(handle_status))
        .with_state(state);

    let llm_routing = std::env::var("SKILLLITE_SWARM_LLM_ROUTING").as_deref() != Ok("0");
    tracing::info!(
        listen = %listen_addr,
        instance = %instance_name,
        llm_routing = llm_routing,
        "Swarm daemon running (mDNS + HTTP). POST /task, GET /status for execution feedback. Ctrl+C to stop."
    );

    // ctrlc: force exit on Ctrl+C — tokio::signal::ctrl_c() can fail to fire on macOS
    // when runtime is busy; ctrlc runs in a dedicated thread and exits immediately.
    ctrlc::set_handler(move || {
        tracing::info!("Ctrl+C received, exiting...");
        std::process::exit(0);
    })
    .context("Failed to set Ctrl+C handler")?;

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let std_listener = std::net::TcpListener::bind(listen_addr)
            .context("Failed to bind TCP listener")?;
        let listener = tokio::net::TcpListener::from_std(std_listener)?;
        axum::serve(listener, app).await?;
        Ok::<(), anyhow::Error>(())
    })?;

    let _ = discovery.shutdown();
    Ok(())
}
