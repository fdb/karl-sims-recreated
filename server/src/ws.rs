use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;

use crate::api::AppState;

pub type UpdateSender = broadcast::Sender<String>;

pub fn ws_route() -> axum::routing::MethodRouter<AppState> {
    axum::routing::get(ws_handler)
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let rx = state.tx.subscribe();
    ws.on_upgrade(move |socket| handle_socket(socket, rx))
}

/// Interval at which we send Ping frames when no broadcast traffic has
/// flowed. Keeps idle connections (paused evolutions, or clients behind
/// timeout-happy proxies) from silently dying.
const HEARTBEAT: Duration = Duration::from_secs(20);

async fn handle_socket(socket: WebSocket, mut rx: broadcast::Receiver<String>) {
    // Split the socket so we can concurrently (a) forward broadcast messages
    // from the coordinator, (b) send periodic Ping heartbeats, and (c) react
    // to client-sent Close/Ping frames. The old handler did only (a), so it
    // couldn't notice a disconnected client until the next broadcast arrived.
    let (mut sender, mut receiver) = socket.split();
    let mut heartbeat = tokio::time::interval(HEARTBEAT);
    heartbeat.tick().await; // consume the immediate first tick

    loop {
        tokio::select! {
            // Broadcast from the coordinator → forward to the client.
            recv = rx.recv() => match recv {
                Ok(msg) => {
                    if sender.send(Message::Text(msg.into())).await.is_err() {
                        break; // client gone
                    }
                }
                // Lagged: this subscriber fell behind the 100-slot broadcast
                // buffer. It's recoverable — just drop the missed messages
                // and keep going. Previously we closed the socket on Lagged,
                // causing spurious client reconnects under fast traffic.
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    log::warn!("WS client lagged by {n} messages; continuing");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            },

            // Heartbeat tick → Ping the client. If send fails, client is gone.
            _ = heartbeat.tick() => {
                if sender.send(Message::Ping(Vec::new().into())).await.is_err() {
                    break;
                }
            }

            // Client frame arrived. We mostly care about Close; Ping/Pong
            // are handled automatically by axum's WebSocket layer, but
            // polling the receiver lets us detect client disconnect
            // promptly instead of waiting for the next outbound send.
            client = receiver.next() => match client {
                None => break, // stream ended
                Some(Err(_)) => break, // socket error
                Some(Ok(Message::Close(_))) => break,
                Some(Ok(_)) => {} // ignore other client frames
            },
        }
    }
}
