use axum::{
    body::Body,
    extract::{Path, Query, State, WebSocketUpgrade, ws::Message},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use std::{
    collections::HashMap,
    io,
    path::{Component, Path as FsPath, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::{Mutex, mpsc};
use tokio_stream::wrappers::ReceiverStream;

use crate::manga::content_type_for_path;

#[derive(Clone)]
pub(crate) struct RelayState {
    hosts: Arc<Mutex<HashMap<String, HostHandle>>>,
    next_id: Arc<AtomicU64>,
}

#[derive(Clone)]
struct HostHandle {
    outbound: mpsc::UnboundedSender<BackendToHost>,
    pending: Arc<Mutex<HashMap<String, mpsc::Sender<Result<Bytes, String>>>>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BackendToHost {
    StartStream { request_id: String, path: String },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum HostToBackend {
    Hello { host_id: String },
    StreamChunk {
        request_id: String,
        data: String,
        last: bool,
    },
    StreamError { request_id: String, error: String },
}

#[derive(serde::Deserialize)]
pub(crate) struct HostQuery {
    pub(crate) host_id: Option<String>,
    pub(crate) agent_id: Option<String>,
}

#[derive(serde::Deserialize)]
pub(crate) struct StreamQuery {
    pub(crate) host_id: Option<String>,
    pub(crate) agent_id: Option<String>,
}

impl RelayState {
    pub(crate) fn new() -> Self {
        Self {
            hosts: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
        }
    }
}

pub(crate) async fn hosting_ws(
    State(state): State<RelayState>,
    Query(query): Query<HostQuery>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let host_id = query
        .host_id
        .or(query.agent_id)
        .unwrap_or_else(|| "local".to_string());

    ws.on_upgrade(move |socket| handle_host_socket(state, host_id, socket))
}

async fn handle_host_socket(state: RelayState, host_id: String, socket: axum::extract::ws::WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel::<BackendToHost>();

    let pending: Arc<Mutex<HashMap<String, mpsc::Sender<Result<Bytes, String>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    {
        let mut hosts = state.hosts.lock().await;
        hosts.insert(
            host_id.clone(),
            HostHandle {
                outbound: outbound_tx,
                pending: pending.clone(),
            },
        );
    }

    println!("hosting app connected: {host_id}");

    let writer = tokio::spawn(async move {
        while let Some(msg) = outbound_rx.recv().await {
            let payload = match serde_json::to_string(&msg) {
                Ok(value) => value,
                Err(err) => {
                    eprintln!("failed to serialize outbound message: {err}");
                    break;
                }
            };

            if sink.send(Message::Text(payload.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(Ok(message)) = stream.next().await {
        let Message::Text(text) = message else {
            continue;
        };

        let incoming = match serde_json::from_str::<HostToBackend>(&text) {
            Ok(msg) => msg,
            Err(err) => {
                eprintln!("invalid host message: {err}");
                continue;
            }
        };

        match incoming {
            HostToBackend::Hello { host_id: hello_id } => {
                println!("hello from hosting app {hello_id}");
            }
            HostToBackend::StreamChunk {
                request_id,
                data,
                last,
            } => {
                let bytes = match STANDARD.decode(data) {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        deliver_error(&pending, &request_id, format!("bad base64 chunk: {err}"))
                            .await;
                        continue;
                    }
                };

                let tx = {
                    let map = pending.lock().await;
                    map.get(&request_id).cloned()
                };

                if let Some(tx) = tx {
                    let _ = tx.send(Ok(Bytes::from(bytes))).await;
                }

                if last {
                    pending.lock().await.remove(&request_id);
                }
            }
            HostToBackend::StreamError { request_id, error } => {
                deliver_error(&pending, &request_id, error).await;
            }
        }
    }

    {
        let mut hosts = state.hosts.lock().await;
        hosts.remove(&host_id);
    }

    pending.lock().await.clear();
    writer.abort();

    println!("hosting app disconnected: {host_id}");
}

async fn deliver_error(
    pending: &Arc<Mutex<HashMap<String, mpsc::Sender<Result<Bytes, String>>>>>,
    request_id: &str,
    error: String,
) {
    let tx = pending.lock().await.remove(request_id);
    if let Some(tx) = tx {
        let _ = tx.send(Err(error)).await;
    }
}

pub(crate) async fn stream_image(
    State(state): State<RelayState>,
    Path(path): Path<String>,
    Query(query): Query<StreamQuery>,
) -> Response {
    let host_id = query
        .host_id
        .or(query.agent_id)
        .unwrap_or_else(|| "local".to_string());

    stream_from_host(&state, host_id, path).await
}

pub(crate) async fn stream_from_host(state: &RelayState, host_id: String, path: String) -> Response {
    if sanitize_relative_path(&path).is_none() {
        return (StatusCode::BAD_REQUEST, "invalid path").into_response();
    }

    let host = {
        let hosts = state.hosts.lock().await;

        hosts
            .get(&host_id)
            .cloned()
            .or_else(|| {
                if host_id == "local" {
                    hosts
                        .iter()
                        .find(|(id, _)| id.starts_with("local/"))
                        .map(|(_, handle)| handle.clone())
                } else {
                    None
                }
            })
            .or_else(|| {
                if hosts.len() == 1 {
                    hosts.values().next().cloned()
                } else {
                    None
                }
            })
    };

    let Some(host) = host else {
        return (StatusCode::SERVICE_UNAVAILABLE, "hosting app not connected").into_response();
    };

    let request_id = state.next_id.fetch_add(1, Ordering::Relaxed).to_string();
    let (tx, mut rx) = mpsc::channel::<Result<Bytes, String>>(8);

    host.pending.lock().await.insert(request_id.clone(), tx);

    if host
        .outbound
        .send(BackendToHost::StartStream {
            request_id: request_id.clone(),
            path: path.clone(),
        })
        .is_err()
    {
        host.pending.lock().await.remove(&request_id);
        return (StatusCode::BAD_GATEWAY, "hosting app send failed").into_response();
    }

    let first = match tokio::time::timeout(Duration::from_secs(8), rx.recv()).await {
        Ok(Some(Ok(chunk))) => chunk,
        Ok(Some(Err(err))) => {
            return (StatusCode::BAD_GATEWAY, format!("hosting app error: {err}")).into_response();
        }
        Ok(None) => {
            host.pending.lock().await.remove(&request_id);
            return (StatusCode::BAD_GATEWAY, "hosting app closed stream").into_response();
        }
        Err(_) => {
            host.pending.lock().await.remove(&request_id);
            return (StatusCode::GATEWAY_TIMEOUT, "hosting app timeout").into_response();
        }
    };

    let first_stream = tokio_stream::once(Ok::<Bytes, io::Error>(first));
    let rest_stream = ReceiverStream::new(rx).map(|item| match item {
        Ok(chunk) => Ok(chunk),
        Err(err) => Err(io::Error::other(err)),
    });

    let stream = first_stream.chain(rest_stream);
    let body = Body::from_stream(stream);

    let mut response = Response::new(body);
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type_for_path(FsPath::new(&path))),
    );
    response
}

fn sanitize_relative_path(input: &str) -> Option<PathBuf> {
    let raw = FsPath::new(input);

    if raw.is_absolute() || input.is_empty() {
        return None;
    }

    let mut out = PathBuf::new();

    for component in raw.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    if out.as_os_str().is_empty() {
        None
    } else {
        Some(out)
    }
}
