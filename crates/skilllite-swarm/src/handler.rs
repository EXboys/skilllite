//! SwarmHandler — full daemon loop: mDNS register, browse, HTTP task API, block until shutdown.
//!
//! Phase 3 routing:
//! - POST /task: receive NodeTask, match capabilities, execute locally or forward to peer.

use crate::error::bail;
use crate::Result;
use anyhow::Context;
use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bytes::Bytes;
use futures_util::stream::{self, StreamExt};
use mdns_sd::ServiceEvent;
use skilllite_core::config::env_keys::swarm;
use skilllite_core::protocol::{NodeResult, NodeTask};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::discovery::{parse_capabilities_from_txt, Discovery};
use crate::routing::{capabilities_match, route_task, RouteTarget, TaskExecutor};

/// Parse listen address `host:port`, `:port`, or `port` into `(host, port)`.
///
/// Port-only / `:port` default host is **127.0.0.1** (loopback). Use explicit `0.0.0.0:PORT` for all interfaces.
fn parse_listen_addr(addr: &str) -> Result<(String, u16)> {
    let addr = addr.trim();
    let parts: Vec<&str> = addr.splitn(2, ':').collect();
    let (host, port_str) = match parts.as_slice() {
        ["", p] => ("127.0.0.1", *p),
        [h, p] => (*h, *p),
        [p] if p.parse::<u16>().is_ok() => ("127.0.0.1", *p),
        _ => bail!(
            "Invalid listen address: expected host:port, :port, or port, got {}",
            addr
        ),
    };
    let port: u16 = port_str.parse().context("Invalid port number")?;
    Ok((host.to_string(), port))
}

fn reqwest_swarm_auth(b: reqwest::RequestBuilder, token: Option<&str>) -> reqwest::RequestBuilder {
    match token {
        Some(t) if !t.is_empty() => b.header(header::AUTHORIZATION, format!("Bearer {}", t)),
        _ => b,
    }
}

/// Shared state for the HTTP server.
#[derive(Clone)]
struct AppState {
    /// This node's instance name (for "I can" response in can-do).
    instance_name: String,
    local_capabilities: Vec<String>,
    peers: Arc<std::sync::Mutex<Vec<crate::discovery::PeerInfo>>>,
    executor: Option<Arc<dyn TaskExecutor>>,
    /// Current task being executed (for GET /status feedback).
    current_task: Arc<std::sync::Mutex<Option<String>>>,
    /// When set, all HTTP routes require `Authorization: Bearer <token>`; forwarded peer requests include it.
    swarm_token: Option<Arc<str>>,
}

/// GET /status — execution status for client polling (avoids "empty wait" UX).
async fn handle_status(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Some(resp) =
        crate::swarm_auth::reject_if_unauthorized(state.swarm_token.as_deref(), &headers)
    {
        return resp;
    }
    let task_id = state.current_task.lock().map(|g| g.clone()).unwrap_or(None);
    let (status, task_id_val) = match &task_id {
        Some(id) => ("busy", serde_json::Value::String(id.clone())),
        None => ("idle", serde_json::Value::Null),
    };
    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": status, "current_task_id": task_id_val })),
    )
        .into_response()
}

#[derive(serde::Deserialize)]
struct CanDoQuery {
    /// Comma-separated capability tags (e.g. "python,web").
    required: Option<String>,
}

/// GET /can-do — "谁能做" broadcast target: respond "我来" when local capabilities match.
/// Query: ?required=tag1,tag2. Returns { "can_do": true, "instance_name": "..." } or can_do: false.
async fn handle_can_do(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<CanDoQuery>,
) -> impl IntoResponse {
    if let Some(resp) =
        crate::swarm_auth::reject_if_unauthorized(state.swarm_token.as_deref(), &headers)
    {
        return resp;
    }
    let required: Vec<String> = q
        .required
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let can_do = capabilities_match(&required, &state.local_capabilities);
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "can_do": can_do,
            "instance_name": state.instance_name,
        })),
    )
        .into_response()
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
    headers: HeaderMap,
    Query(query): Query<TaskQuery>,
    Json(task): Json<NodeTask>,
) -> impl IntoResponse {
    if let Some(resp) =
        crate::swarm_auth::reject_if_unauthorized(state.swarm_token.as_deref(), &headers)
    {
        return resp;
    }
    let token = state.swarm_token.as_deref();
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
        let inferred =
            crate::llm_routing::infer_required_capabilities(&task.description, &all_tags).await;
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
    let mut target = route_task(&task_for_route, &state.local_capabilities, &peers);

    // NoMatch 时广播「谁能做」→ 收集「我来」再转发（简单路由）
    if matches!(&target, RouteTarget::NoMatch) && !required.is_empty() && !peers.is_empty() {
        let can_do_query = required.join(",");
        let client = reqwest::Client::new();
        let mut respondents: Vec<crate::discovery::PeerInfo> = Vec::new();
        for peer in &peers {
            let url = format!(
                "http://{}/can-do?required={}",
                peer.addr,
                urlencoding::encode(&can_do_query)
            );
            match reqwest_swarm_auth(client.get(&url).timeout(Duration::from_secs(3)), token)
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(json) = resp.json::<serde_json::Value>().await {
                        if json
                            .get("can_do")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                        {
                            respondents.push(peer.clone());
                        }
                    }
                }
                _ => {}
            }
        }
        if !respondents.is_empty() {
            let n = respondents.len();
            target = RouteTarget::Forward(respondents);
            tracing::info!(
                task_id = %task.id,
                required = ?required,
                respondent_count = n,
                "Routing: broadcast who-can-do → forward to respondent(s)"
            );
        }
    }

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
                    Ok::<_, crate::Error>(Bytes::from(format!(
                        "{{\"event\":\"received\",\"task_id\":\"{}\"}}\n",
                        task_id
                    ))),
                    Ok(Bytes::from("{\"event\":\"executing\"}\n")),
                ]);
                let s2 = stream::once(async move {
                    let result = tokio::task::spawn_blocking(move || exec.execute(task))
                        .await
                        .map_err(|e| crate::Error::validation(format!("{:?}", e)))?
                        .map_err(|e| crate::Error::validation(format!("{}", e)));
                    if let Ok(mut cur) = current_task.lock() {
                        *cur = None;
                    }
                    match result {
                        Ok(res) => {
                            let json = serde_json::to_string(&res)
                                .map_err(|e| crate::Error::validation(e.to_string()))?;
                            Ok(Bytes::from(format!(
                                "{{\"event\":\"done\",\"result\":{}}}\n",
                                json
                            )))
                        }
                        Err(e) => {
                            let json = serde_json::json!({"error":"execution_failed","message":e.to_string()});
                            Ok(Bytes::from(format!(
                                "{{\"event\":\"error\",\"error\":{}}}\n",
                                json
                            )))
                        }
                    }
                });
                let body = Body::from_stream(s1.chain(s2));
                return match Response::builder()
                    .status(StatusCode::OK)
                    .header(header::CONTENT_TYPE, "application/x-ndjson")
                    .body(body)
                {
                    Ok(resp) => resp.into_response(),
                    Err(e) => {
                        tracing::error!(err = %e, "Response::builder failed");
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({"error":"internal","message":e.to_string()})),
                        )
                            .into_response()
                    }
                };
            }
            let start = std::time::Instant::now();
            let exec = exec.clone();
            let result = tokio::task::spawn_blocking(move || exec.execute(task)).await;
            if let Ok(mut cur) = state.current_task.lock() {
                *cur = None;
            }
            let result = result
                .map_err(|e| crate::Error::validation(format!("{:?}", e)))
                .and_then(|r| r.map_err(|e| crate::Error::validation(format!("{}", e))));
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
                match reqwest_swarm_auth(
                    client
                        .post(&url)
                        .json(&task)
                        .timeout(Duration::from_secs(120)),
                    token,
                )
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

/// Run the swarm daemon: register via mDNS (unless bind is loopback-only), browse for peers, serve HTTP task API, block until Ctrl+C.
///
/// - `executor`: Optional. When set, local tasks are executed via this; otherwise returns 503.
/// - Sets `SKILLLITE_SWARM_URL` so agent's delegate_to_swarm can route to this swarm (skill sharing).
/// - When `skills_dir` is set, loads .env from its parent (project root) so OPENAI_API_KEY is available for LLM routing.
/// - When `SKILLLITE_SWARM_TOKEN` is set (after dotenv), all HTTP routes require `Authorization: Bearer`; peer forwards include it.
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
            path.parent()
                .map(skilllite_core::config::load_dotenv_from_dir);
        }
    }

    let (host, port) = parse_listen_addr(listen_addr)?;
    let bind_addr = format!("{}:{}", host, port);
    let swarm_token = crate::swarm_auth::swarm_token_from_env().map(Arc::<str>::from);
    let instance_name = uuid::Uuid::new_v4().to_string();

    // Enable delegate_to_swarm to route to this node (for skill sharing)
    let swarm_url = format!("http://127.0.0.1:{}", port);
    if std::env::var(swarm::SKILLLITE_SWARM_URL).is_err() {
        std::env::set_var(swarm::SKILLLITE_SWARM_URL, &swarm_url);
    }

    let loopback_only =
        host == "127.0.0.1" || host == "::1" || host.eq_ignore_ascii_case("localhost");

    if host == "0.0.0.0" && swarm_token.is_none() {
        tracing::warn!(
            "Swarm listening on all interfaces without SKILLLITE_SWARM_TOKEN: any host that can reach this port may call the HTTP API. Set SKILLLITE_SWARM_TOKEN for Bearer authentication."
        );
    }
    if swarm_token.is_some() {
        tracing::info!("Swarm HTTP API requires Authorization: Bearer <SKILLLITE_SWARM_TOKEN>.");
    }

    let discovery = Discovery::new()?;
    if !loopback_only {
        discovery.register(&instance_name, &host, port, &capability_tags)?;
    } else {
        tracing::info!(
            port = port,
            "Swarm bind is loopback-only: skipped mDNS registration. For LAN peer mesh use e.g. --listen 0.0.0.0:{}",
            port
        );
    }

    let browse_rx = discovery.browse()?;
    let peers: Arc<std::sync::Mutex<Vec<crate::discovery::PeerInfo>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));

    // Spawn browse loop — populate peers, exclude self
    let peers_browse = peers.clone();
    let my_instance = instance_name.clone();
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_browse = shutdown.clone();
    std::thread::spawn(move || {
        while !shutdown_browse.load(Ordering::SeqCst) {
            match browse_rx.recv_timeout(Duration::from_millis(500)) {
                Ok(ServiceEvent::ServiceResolved(resolved)) => {
                    let instance_name = resolved
                        .fullname
                        .split('.')
                        .next()
                        .unwrap_or("")
                        .to_string();
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
                        if let Some(existing) =
                            p.iter_mut().find(|x| x.instance_name == info.instance_name)
                        {
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
        instance_name: instance_name.clone(),
        local_capabilities: capability_tags.clone(),
        peers,
        executor,
        current_task: Arc::new(std::sync::Mutex::new(None)),
        swarm_token: swarm_token.clone(),
    };

    let app = Router::new()
        .route("/task", post(handle_task))
        .route("/status", get(handle_status))
        .route("/can-do", get(handle_can_do))
        .with_state(state);

    let llm_routing = std::env::var("SKILLLITE_SWARM_LLM_ROUTING").as_deref() != Ok("0");
    tracing::info!(
        listen = %bind_addr,
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
        let std_listener =
            std::net::TcpListener::bind(&bind_addr).context("Failed to bind TCP listener")?;
        let listener = tokio::net::TcpListener::from_std(std_listener)?;
        axum::serve(listener, app).await?;
        Ok::<(), crate::Error>(())
    })?;

    let _ = discovery.shutdown();
    Ok(())
}

#[cfg(test)]
mod listen_addr_tests {
    use super::parse_listen_addr;

    #[test]
    fn port_only_defaults_loopback() {
        let (h, p) = parse_listen_addr("7700").unwrap();
        assert_eq!(h, "127.0.0.1");
        assert_eq!(p, 7700);
    }

    #[test]
    fn colon_port_defaults_loopback() {
        let (h, p) = parse_listen_addr(":7700").unwrap();
        assert_eq!(h, "127.0.0.1");
        assert_eq!(p, 7700);
    }

    #[test]
    fn explicit_all_interfaces_preserved() {
        let (h, p) = parse_listen_addr("0.0.0.0:7700").unwrap();
        assert_eq!(h, "0.0.0.0");
        assert_eq!(p, 7700);
    }
}
