use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query,
    },
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use serde::Deserialize;
use tokio::time::{interval_at, Duration, Instant};
use uuid::Uuid;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/socket.io", get(websocket_upgrade))
        .route("/api/socket.io/", get(websocket_upgrade))
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SocketIoQuery {
    eio: Option<String>,
    transport: Option<String>,
}

async fn websocket_upgrade(
    ws: Option<WebSocketUpgrade>,
    headers: HeaderMap,
    Query(query): Query<SocketIoQuery>,
) -> Response {
    let transport = query.transport.as_deref().unwrap_or_default();
    let is_websocket_transport = transport.eq_ignore_ascii_case("websocket");
    let has_upgrade_header = headers
        .get(header::UPGRADE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("websocket"));

    if let Some(ws) = ws {
        let _ = query.eio.as_deref();
        return ws.on_upgrade(handle_socket).into_response();
    }

    if is_websocket_transport || has_upgrade_header {
        return (
            StatusCode::BAD_REQUEST,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "websocket upgrade required",
        )
            .into_response();
    }

    let sid = Uuid::new_v4().to_string();
    let open_packet = format!(
        "0{{\"sid\":\"{sid}\",\"upgrades\":[\"websocket\"],\"pingInterval\":25000,\"pingTimeout\":20000,\"maxPayload\":1000000}}"
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        open_packet,
    )
        .into_response()
}

async fn handle_socket(mut socket: WebSocket) {
    let sid = Uuid::new_v4().to_string();
    let open_packet = format!(
        "0{{\"sid\":\"{sid}\",\"upgrades\":[],\"pingInterval\":25000,\"pingTimeout\":20000,\"maxPayload\":1000000}}"
    );
    let connect_packet = format!("40{{\"sid\":\"{sid}\"}}");

    if socket.send(Message::Text(open_packet.into())).await.is_err() {
        return;
    }

    let mut connect_ack_sent = false;
    let mut ping_interval = interval_at(
        Instant::now() + Duration::from_secs(25),
        Duration::from_secs(25),
    );

    loop {
        tokio::select! {
            _ = ping_interval.tick() => {
                if socket.send(Message::Text("2".into())).await.is_err() {
                    break;
                }
            }
            message = socket.recv() => {
                let Some(Ok(message)) = message else {
                    break;
                };

                match message {
                    Message::Text(text) => {
                        if text == "2" {
                            if socket.send(Message::Text("3".into())).await.is_err() {
                                break;
                            }
                        } else if text.starts_with("40") && !connect_ack_sent {
                            connect_ack_sent = true;
                            if socket.send(Message::Text(connect_packet.clone().into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Message::Ping(payload) => {
                        if socket.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
        }
    }
}
