// `fsn bus` — Message bus daemon + CLI commands.
//
// REST API routes (port 8081 by default):
//   POST   /api/bus/publish              → publish an event
//   GET    /api/bus/subscriptions        → list active subscriptions
//   POST   /api/bus/subscribe            → add subscription
//   DELETE /api/bus/subscribe/:id        → remove subscription
//   GET    /api/bus/standing-orders      → list standing orders
//   POST   /api/bus/standing-orders      → add standing order
//   DELETE /api/bus/standing-orders/:id  → remove standing order
//   GET    /api/bus/events               → recent event log (in-memory ring)
//   GET    /api/bus/ws                   → WebSocket live event stream
//   POST   /api/bus/role/:role/trigger   → trigger standing orders for a role

use std::collections::VecDeque;
use std::sync::Arc;

use anyhow::Result;
use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{delete, get, post},
};
use fsn_bus::{
    BusMessage, Event, MessageBus, RoutingConfig, StandingOrder, Subscription,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::{Mutex, broadcast};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;
use uuid::Uuid;

// ── Shared state ──────────────────────────────────────────────────────────────

/// Recent events kept in memory for the event log endpoint.
const EVENT_RING_SIZE: usize = 500;

/// App state shared across all axum handlers.
#[derive(Clone)]
struct BusState {
    bus:    Arc<Mutex<MessageBus>>,
    /// In-memory ring buffer of recent events (serialized JSON).
    events: Arc<Mutex<VecDeque<serde_json::Value>>>,
    /// Broadcast channel for WebSocket live streaming.
    tx:     broadcast::Sender<String>,
}

impl BusState {
    fn new(bus: MessageBus) -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self {
            bus:    Arc::new(Mutex::new(bus)),
            events: Arc::new(Mutex::new(VecDeque::with_capacity(EVENT_RING_SIZE))),
            tx,
        }
    }

    async fn record_event(&self, ev: &Event) {
        let val = json!({
            "id":        ev.meta.id.to_string(),
            "topic":     ev.topic(),
            "source":    ev.meta.source,
            "timestamp": ev.meta.timestamp.to_rfc3339(),
            "payload":   ev.payload,
        });
        let s = val.to_string();
        let mut ring = self.events.lock().await;
        if ring.len() >= EVENT_RING_SIZE {
            ring.pop_front();
        }
        ring.push_back(val);
        let _ = self.tx.send(s);
    }
}

// ── Request / response DTOs ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct PublishReq {
    topic:   String,
    source:  String,
    payload: Option<serde_json::Value>,
    #[serde(default)]
    delivery: Option<String>,
    #[serde(default)]
    storage:  Option<String>,
}

#[derive(Deserialize)]
struct SubscribeReq {
    subscriber_role: String,
    topic_filter:    String,
    inst_tag:        Option<String>,
}

#[derive(Deserialize)]
struct StandingOrderReq {
    name:         String,
    trigger_role: String,
    topic:        String,
    payload:      Option<serde_json::Value>,
}

#[derive(Serialize)]
struct SubJson {
    id:              String,
    subscriber_role: String,
    topic_filter:    String,
    inst_tag:        Option<String>,
    granted_read:    bool,
}

#[derive(Serialize)]
struct OrderJson {
    id:           String,
    name:         String,
    trigger_role: String,
    topic:        String,
    enabled:      bool,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn handle_publish(
    State(s): State<BusState>,
    Json(req): Json<PublishReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let payload = req.payload.unwrap_or(json!({}));
    let ev = Event::new(req.topic, req.source, payload)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    s.record_event(&ev).await;

    let msg = BusMessage::fire(ev);
    let result = s.bus.lock().await.publish(msg).await;

    Ok(Json(json!({
        "delivered_to":   result.delivered_to,
        "delivery":       result.delivery.as_str(),
        "storage":        result.storage.as_str(),
        "handler_errors": result.handler_results.iter().filter(|r| r.is_err()).count(),
    })))
}

async fn handle_list_subscriptions(
    State(s): State<BusState>,
) -> Json<serde_json::Value> {
    let bus = s.bus.lock().await;
    let all: Vec<SubJson> = bus.subscriptions_iter().map(sub_to_json).collect();
    Json(json!({ "subscriptions": all }))
}

async fn handle_subscribe(
    State(s): State<BusState>,
    Json(req): Json<SubscribeReq>,
) -> Json<serde_json::Value> {
    let mut sub = Subscription::new(req.subscriber_role, req.topic_filter);
    if let Some(tag) = req.inst_tag {
        sub = sub.with_inst_tag(tag);
    }
    let stored = s.bus.lock().await.subscribe(sub);
    Json(json!({ "id": stored.id.to_string() }))
}

async fn handle_unsubscribe(
    State(s): State<BusState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let uuid = Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let removed = s.bus.lock().await.unsubscribe(uuid);
    if removed {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn handle_list_orders(
    State(s): State<BusState>,
) -> Json<serde_json::Value> {
    let bus = s.bus.lock().await;
    let orders: Vec<OrderJson> = bus.standing_orders_iter()
        .map(order_to_json)
        .collect();
    Json(json!({ "standing_orders": orders }))
}

async fn handle_add_order(
    State(s): State<BusState>,
    Json(req): Json<StandingOrderReq>,
) -> Json<serde_json::Value> {
    let payload = req.payload.unwrap_or(json!({}));
    let order = StandingOrder::new(req.name, req.trigger_role, req.topic, payload);
    let id = order.id;
    s.bus.lock().await.add_standing_order(order);
    Json(json!({ "id": id.to_string() }))
}

async fn handle_remove_order(
    State(s): State<BusState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let uuid = Uuid::parse_str(&id).map_err(|_| StatusCode::BAD_REQUEST)?;
    let removed = s.bus.lock().await.remove_standing_order(uuid);
    if removed {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn handle_events(
    State(s): State<BusState>,
) -> Json<serde_json::Value> {
    let ring = s.events.lock().await;
    let events: Vec<_> = ring.iter().cloned().collect();
    Json(json!({ "events": events, "count": events.len() }))
}

async fn handle_trigger_role(
    State(s): State<BusState>,
    Path(role): Path<String>,
) -> Json<serde_json::Value> {
    let bus = s.bus.lock().await;
    let generated = bus.trigger_role(&role);
    drop(bus);

    let mut published = 0usize;
    for result in generated {
        if let Ok(ev) = result {
            s.record_event(&ev).await;
            s.bus.lock().await.publish(BusMessage::fire(ev)).await;
            published += 1;
        }
    }
    Json(json!({ "triggered": published, "role": role }))
}

async fn handle_ws(
    State(s): State<BusState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_handler(socket, s.tx.subscribe()))
}

async fn ws_handler(mut socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    loop {
        tokio::select! {
            Ok(msg) = rx.recv() => {
                if socket.send(WsMessage::Text(msg.into())).await.is_err() {
                    break;
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(WsMessage::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
}

// ── Helper serializers ────────────────────────────────────────────────────────

fn sub_to_json(s: &fsn_bus::Subscription) -> SubJson {
    SubJson {
        id:              s.id.to_string(),
        subscriber_role: s.subscriber_role.clone(),
        topic_filter:    s.topic_filter.clone(),
        inst_tag:        s.inst_tag.clone(),
        granted_read:    s.granted_read,
    }
}

fn order_to_json(o: &fsn_bus::StandingOrder) -> OrderJson {
    OrderJson {
        id:           o.id.to_string(),
        name:         o.name.clone(),
        trigger_role: o.trigger_role.clone(),
        topic:        o.topic.clone(),
        enabled:      o.enabled,
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Start the bus HTTP/WS server.
pub async fn serve(bind: &str, port: u16, config_path: Option<&str>) -> Result<()> {
    let mut bus = MessageBus::new();

    if let Some(path) = config_path {
        match bus.load_config_file(path) {
            Ok(()) => info!("Bus routing config loaded from {path}"),
            Err(e) => tracing::warn!("Could not load bus config: {e}"),
        }
    }

    let state = BusState::new(bus);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/bus/publish",              post(handle_publish))
        .route("/api/bus/subscriptions",        get(handle_list_subscriptions))
        .route("/api/bus/subscribe",            post(handle_subscribe))
        .route("/api/bus/subscribe/:id",        delete(handle_unsubscribe))
        .route("/api/bus/standing-orders",      get(handle_list_orders))
        .route("/api/bus/standing-orders",      post(handle_add_order))
        .route("/api/bus/standing-orders/:id",  delete(handle_remove_order))
        .route("/api/bus/events",               get(handle_events))
        .route("/api/bus/role/:role/trigger",   post(handle_trigger_role))
        .route("/api/bus/ws",                   get(handle_ws))
        .layer(cors)
        .with_state(state);

    let addr = format!("{bind}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("FSN Message Bus listening on http://{addr}");
    println!("Bus API : http://{addr}/api/bus/");
    println!("Bus WS  : ws://{addr}/api/bus/ws");
    println!("Press Ctrl+C to stop.");

    axum::serve(listener, app).await?;
    Ok(())
}

/// Print current bus status (subscriptions + standing orders) to stdout.
pub async fn status() -> Result<()> {
    // For now: connect to running bus via HTTP
    let client = reqwest::Client::new();
    let base = "http://127.0.0.1:8081";

    match client.get(format!("{base}/api/bus/subscriptions")).send().await {
        Ok(resp) => {
            let v: serde_json::Value = resp.json().await?;
            let subs = v["subscriptions"].as_array().cloned().unwrap_or_default();
            println!("Active subscriptions: {}", subs.len());
            for s in &subs {
                println!("  [{role}] → {filter}",
                    role   = s["subscriber_role"].as_str().unwrap_or("?"),
                    filter = s["topic_filter"].as_str().unwrap_or("?"),
                );
            }
        }
        Err(_) => println!("Bus not running (try `fsn bus serve`)"),
    }

    match client.get(format!("{base}/api/bus/standing-orders")).send().await {
        Ok(resp) => {
            let v: serde_json::Value = resp.json().await?;
            let orders = v["standing_orders"].as_array().cloned().unwrap_or_default();
            println!("Standing orders: {}", orders.len());
            for o in &orders {
                println!("  [{}] {} → {}",
                    if o["enabled"].as_bool().unwrap_or(false) { "✓" } else { "✗" },
                    o["name"].as_str().unwrap_or("?"),
                    o["topic"].as_str().unwrap_or("?"),
                );
            }
        }
        Err(_) => {}
    }

    Ok(())
}

/// Publish a single event via the running bus.
pub async fn publish_event(topic: &str, source: &str, payload_json: Option<&str>) -> Result<()> {
    let payload: serde_json::Value = payload_json
        .map(|s| serde_json::from_str(s).unwrap_or(json!({})))
        .unwrap_or(json!({}));

    let client = reqwest::Client::new();
    let resp = client
        .post("http://127.0.0.1:8081/api/bus/publish")
        .json(&json!({ "topic": topic, "source": source, "payload": payload }))
        .send()
        .await?;

    let v: serde_json::Value = resp.json().await?;
    println!("Published: delivered_to={:?} delivery={}",
        v["delivered_to"],
        v["delivery"].as_str().unwrap_or("?"),
    );
    Ok(())
}
