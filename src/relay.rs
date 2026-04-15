use axum::{
    body::Body,
    extract::{Path, Query, State, WebSocketUpgrade, ws::Message},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
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
    catalogs: Arc<Mutex<HashMap<String, HostCatalog>>>,
    next_id: Arc<AtomicU64>,
}

#[derive(Clone)]
struct HostHandle {
    outbound: mpsc::UnboundedSender<BackendToHost>,
    pending: Arc<Mutex<HashMap<String, mpsc::Sender<Result<Bytes, String>>>>>,
}

#[derive(Clone)]
struct HostCatalog {
    mangas: HashMap<String, Vec<RegisteredVolume>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RegisteredPage {
    pub(crate) page_id: String,
    pub(crate) index: usize,
    pub(crate) is_landscape: bool,
    pub(crate) content_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RegisteredVolume {
    pub(crate) number: usize,
    pub(crate) label: String,
    pub(crate) pages: Vec<RegisteredPage>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum BackendToHost {
    StartStream { request_id: String, path: String },
    StartStreamById { request_id: String, page_id: String },
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
    RegisterCatalog {
        manga: String,
        volumes: Vec<RegisteredVolume>,
    },
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
            catalogs: Arc::new(Mutex::new(HashMap::new())),
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
            HostToBackend::RegisterCatalog { manga, mut volumes } => {
                volumes.sort_by_key(|v| v.number);

                let mut catalogs = state.catalogs.lock().await;
                let host_catalog = catalogs.entry(host_id.clone()).or_insert_with(|| HostCatalog {
                    mangas: HashMap::new(),
                });
                host_catalog.mangas.insert(manga, volumes);
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
    {
        let mut catalogs = state.catalogs.lock().await;
        catalogs.remove(&host_id);
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
        resolve_host_handle(&hosts, &host_id)
    };

    let Some(host) = host else {
        return (StatusCode::SERVICE_UNAVAILABLE, "hosting app not connected").into_response();
    };

    let body = match request_stream_body(
        state,
        &host,
        BackendToHost::StartStream {
            request_id: state.next_id.fetch_add(1, Ordering::Relaxed).to_string(),
            path: path.clone(),
        },
    )
    .await
    {
        Ok(body) => body,
        Err(err) => return err.into_response(),
    };

    let mut response = Response::new(body);
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type_for_path(FsPath::new(&path))),
    );
    response
}

pub(crate) async fn get_registered_manifest_pages(
    state: &RelayState,
    requested_host_id: &str,
    manga: &str,
    volume: usize,
) -> Option<Vec<RegisteredPage>> {
    let resolved_host_id = resolve_host_id(state, requested_host_id).await?;
    let catalogs = state.catalogs.lock().await;
    let host_catalog = catalogs.get(&resolved_host_id)?;
    let volumes = host_catalog.mangas.get(manga)?;
    let selected = volumes.iter().find(|entry| entry.number == volume)?;
    Some(selected.pages.clone())
}

pub(crate) async fn stream_page_by_id(state: &RelayState, page_id: String) -> Response {
    let Some((host_id, content_type)) = find_page_owner(state, &page_id).await else {
        return (StatusCode::NOT_FOUND, "page id not registered").into_response();
    };

    let host = {
        let hosts = state.hosts.lock().await;
        hosts.get(&host_id).cloned()
    };

    let Some(host) = host else {
        return (StatusCode::SERVICE_UNAVAILABLE, "hosting app not connected").into_response();
    };

    let request_id = state.next_id.fetch_add(1, Ordering::Relaxed).to_string();
    let body = match request_stream_body(
        state,
        &host,
        BackendToHost::StartStreamById {
            request_id,
            page_id,
        },
    )
    .await
    {
        Ok(body) => body,
        Err(err) => return err.into_response(),
    };

    let mut response = Response::new(body);
    let content_type = HeaderValue::from_str(&content_type)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, content_type);
    response
}

async fn request_stream_body(
    _state: &RelayState,
    host: &HostHandle,
    msg: BackendToHost,
) -> Result<Body, (StatusCode, String)> {
    let request_id = match &msg {
        BackendToHost::StartStream { request_id, .. } => request_id.clone(),
        BackendToHost::StartStreamById { request_id, .. } => request_id.clone(),
    };

    let (tx, mut rx) = mpsc::channel::<Result<Bytes, String>>(8);
    host.pending.lock().await.insert(request_id.clone(), tx);

    if host.outbound.send(msg).is_err() {
        host.pending.lock().await.remove(&request_id);
        return Err((StatusCode::BAD_GATEWAY, "hosting app send failed".to_string()));
    }

    let first = match tokio::time::timeout(Duration::from_secs(8), rx.recv()).await {
        Ok(Some(Ok(chunk))) => chunk,
        Ok(Some(Err(err))) => return Err((StatusCode::BAD_GATEWAY, format!("hosting app error: {err}"))),
        Ok(None) => {
            host.pending.lock().await.remove(&request_id);
            return Err((StatusCode::BAD_GATEWAY, "hosting app closed stream".to_string()));
        }
        Err(_) => {
            host.pending.lock().await.remove(&request_id);
            return Err((StatusCode::GATEWAY_TIMEOUT, "hosting app timeout".to_string()));
        }
    };

    let first_stream = tokio_stream::once(Ok::<Bytes, io::Error>(first));
    let rest_stream = ReceiverStream::new(rx).map(|item| match item {
        Ok(chunk) => Ok(chunk),
        Err(err) => Err(io::Error::other(err)),
    });

    Ok(Body::from_stream(first_stream.chain(rest_stream)))
}

async fn resolve_host_id(state: &RelayState, requested: &str) -> Option<String> {
    let hosts = state.hosts.lock().await;
    if hosts.contains_key(requested) {
        return Some(requested.to_string());
    }
    if requested == "local" {
        if let Some((id, _)) = hosts.iter().find(|(id, _)| id.starts_with("local/")) {
            return Some(id.clone());
        }
    }
    if hosts.len() == 1 {
        return hosts.keys().next().cloned();
    }
    None
}

async fn find_page_owner(state: &RelayState, page_id: &str) -> Option<(String, String)> {
    let catalogs = state.catalogs.lock().await;

    for (host_id, host_catalog) in catalogs.iter() {
        for volumes in host_catalog.mangas.values() {
            for volume in volumes {
                for page in &volume.pages {
                    if page.page_id == page_id {
                        return Some((host_id.clone(), page.content_type.clone()));
                    }
                }
            }
        }
    }
    None
}

fn resolve_host_handle(hosts: &HashMap<String, HostHandle>, host_id: &str) -> Option<HostHandle> {
    hosts
        .get(host_id)
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
