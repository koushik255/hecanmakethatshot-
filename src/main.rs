mod marked;

use axum::{
    Router,
    body::Body,
    extract::{Query, State},
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use marked::{add_bookmark, add_pagemark, list_pagemarks};
use std::{
    io,
    path::{self, Path, PathBuf},
    sync::Arc,
};
use tokio::{fs, sync::RwLock};

const IMAGE_PATH: &str = "/home/koushikk/Desktop/akane.jpg";
const MANGA_ROOT: &str = "/home/koushikk/MANGA/Kingdom/";
const SELECTED_MANGA: &str = "Kingdom";
const INDEX_HTML: &str = include_str!("../static/index.html");
const INDEX_JS: &str = include_str!("../static/index.js");
const PM_HTML: &str = include_str!("../static/pm.html");
const PM_JS: &str = include_str!("../static/pm.js");

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) pages: Arc<Vec<Page>>,
    pub(crate) steps: Arc<Vec<ViewStep>>,
    pub(crate) current_step: Arc<RwLock<usize>>,
    pub(crate) current_volume: usize,
}
#[derive(Clone, Copy)]
pub(crate) enum ViewStep {
    Single(usize),
    Spread { right: usize, left: usize },
}

#[tokio::main]
async fn main() {
    let manga_dir = PathBuf::from(MANGA_ROOT).join(SELECTED_MANGA);

    let volume_number: usize = 49;
    let volume_path = select_volume(list_volumes(manga_dir.as_path()), volume_number);
    let pages = chosen_volume(volume_path.as_path()).expect("failed to read selected volume");
    let steps = build_view_steps(&pages);

    let state = AppState {
        pages: Arc::new(pages),
        steps: Arc::new(steps),
        current_step: Arc::new(RwLock::new(0)),
        current_volume: volume_number,
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/index.js", get(index_js))
        .route("/pm", get(pm_page))
        .route("/pm.js", get(pm_js))
        .route("/api/akane", get(right_page_bytes))
        .route("/api/right", get(right_page_bytes))
        .route("/api/left", get(left_page_bytes))
        .route("/api/next", get(next_page))
        .route("/api/prev", get(prev_page))
        .route("/api/bookmark", post(add_bookmark))
        .route("/api/pagemark", post(add_pagemark))
        .route("/api/pagemarks", get(list_pagemarks))
        .route("/api/image-by-path", get(image_by_path))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("failed to bind to 127.0.0.1:3000");

    println!("Server running on http://127.0.0.1:3000");

    axum::serve(listener, app).await.expect("server crashed");
}

fn gimme() -> PathBuf {
    let manga_dir = PathBuf::from(MANGA_ROOT).join(SELECTED_MANGA);
    let bomba = select_volume(list_volumes(manga_dir.as_path()), 4);
    let fuck = chosen_volume(bomba.as_path()).expect("FUUUUCK");
    //   quick(fuck);
    let first_page = fuck[0].path.clone();
    first_page
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn index_js() -> Response {
    let mut res = Response::new(Body::from(INDEX_JS));
    res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/javascript; charset=utf-8"),
    );
    res
}

async fn pm_page() -> Html<&'static str> {
    Html(PM_HTML)
}

async fn pm_js() -> Response {
    let mut res = Response::new(Body::from(PM_JS));
    res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/javascript; charset=utf-8"),
    );
    res
}

fn list_volumes(manga_dir: &std::path::Path) -> std::fs::ReadDir {
    println!("Listing volumes in {}", manga_dir.display());
    let volumes = std::fs::read_dir(manga_dir).expect("Erorring getting volumes");
    println!("{:?}", volumes);

    volumes
}
#[derive(Ord, PartialEq, PartialOrd, Eq)]
struct Volume {
    location: std::path::PathBuf,
}

struct SelectedVolume {
    page_count: usize,
}

fn volume_number(path: &std::path::Path) -> u32 {
    path.file_name()
        .and_then(|s| s.to_str())
        .and_then(|name| name.rsplit('_').next()) // "7" from "nana_7"
        .and_then(|n| n.parse::<u32>().ok())
        .unwrap_or(u32::MAX) // e.g. "zips" goes last
}

fn select_volume(volumes: std::fs::ReadDir, num: usize) -> path::PathBuf {
    let mut sorted: Vec<Volume> = Vec::new();

    for v in volumes {
        let w = Volume {
            location: v.expect("cannot get path").path(),
        };
        if volume_number(&w.location) == u32::MAX {
            continue;
        }
        sorted.push(w);
    }
    sorted.sort_by_key(|v| volume_number(&v.location));

    // for v in sorted {
    //     println!("{}", v.location.display());
    // }
    let kys = num - 1;
    //   println!("The {}th volume is {}", num, sorted[kys].location.display());
    let selected_volume = sorted[kys].location.clone();
    selected_volume
}

//next function would be getting the pages into a list
//

#[derive(Debug)]
pub(crate) struct Page {
    number: usize,
    pub(crate) path: PathBuf,
}

fn chosen_volume(cv: &std::path::Path) -> io::Result<Vec<Page>> {
    println!("Selected to read this {}", cv.display());

    //putpagesintoalistthencangetlenandpaths
    //im make page path aswell tbf

    let mut pages: Vec<path::PathBuf> = std::fs::read_dir(cv)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    pages.sort();

    let pages_structs: Vec<Page> = pages
        .into_iter()
        .enumerate()
        .map(|(i, path)| Page {
            number: i + 1,
            path,
        })
        .collect();

    //   println!("Page Count {}", pages.len());

    Ok(pages_structs)
}

fn quick(page_structs: Vec<Page>) {
    let page_structs = page_structs;

    for p in page_structs {
        println!(
            "Page number {} and page count {}",
            p.number,
            p.path.display()
        );
    }
}

fn is_landscape(path: &Path) -> bool {
    match image::image_dimensions(path) {
        Ok((w, h)) => w > h,
        Err(_) => false,
    }
}

fn build_view_steps(pages: &[Page]) -> Vec<ViewStep> {
    let mut steps = Vec::new();
    if pages.is_empty() {
        return steps;
    }

    let last = pages.len() - 1;
    let mut i = 0;

    while i < pages.len() {
        let solo = i == 0 || i == last || is_landscape(&pages[i].path);

        if solo {
            steps.push(ViewStep::Single(i));
            i += 1;
            continue;
        }

        if i + 1 <= last && i + 1 != last && !is_landscape(&pages[i + 1].path) {
            steps.push(ViewStep::Spread {
                right: i,
                left: i + 1,
            });
            i += 2;
        } else {
            steps.push(ViewStep::Single(i));
            i += 1;
        }
    }

    steps
}

async fn next_page(State(state): State<AppState>) -> Response {
    let mut idx = state.current_step.write().await;

    if *idx + 1 < state.steps.len() {
        *idx += 1;
        (StatusCode::OK, format!("next spread worked mud {}", *idx)).into_response()
    } else {
        (StatusCode::NO_CONTENT, "Already at last spread").into_response()
    }
}

async fn prev_page(State(state): State<AppState>) -> Response {
    let mut idx = state.current_step.write().await;

    if *idx > 0 {
        *idx -= 1;
        (StatusCode::OK, format!("prev spread worked mud {}", *idx)).into_response()
    } else {
        (StatusCode::NO_CONTENT, "Already at first spread").into_response()
    }
}

async fn right_page_bytes(State(state): State<AppState>) -> Response {
    let step_idx = *state.current_step.read().await;

    let Some(step) = state.steps.get(step_idx) else {
        return (StatusCode::NOT_FOUND, "No step found").into_response();
    };

    let idx = match *step {
        ViewStep::Single(i) => i,
        ViewStep::Spread { right, .. } => right,
    };

    image_bytes_for_idx(&state, idx).await
}

async fn left_page_bytes(State(state): State<AppState>) -> Response {
    let step_idx = *state.current_step.read().await;

    let Some(step) = state.steps.get(step_idx) else {
        return (StatusCode::NOT_FOUND, "No step found").into_response();
    };

    let idx = match *step {
        ViewStep::Single(_) => {
            return (StatusCode::NO_CONTENT, "No left page for this spread").into_response();
        }
        ViewStep::Spread { left, .. } => left,
    };

    image_bytes_for_idx(&state, idx).await
}

async fn image_bytes_for_idx(state: &AppState, idx: usize) -> Response {
    let Some(page) = state.pages.get(idx) else {
        return (StatusCode::NOT_FOUND, format!("no page found mud")).into_response();
    };

    match fs::read(&page.path).await {
        Ok(bytes) => {
            let mut res = Response::new(Body::from(bytes));
            res.headers_mut()
                .insert(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
            res
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Could not read image at {IMAGE_PATH}: {err}"),
        )
            .into_response(),
    }
}

#[derive(serde::Deserialize)]
struct ImagePathQuery {
    path: String,
}

async fn image_by_path(Query(q): Query<ImagePathQuery>) -> Response {
    image_bytes_for_path(Path::new(&q.path)).await
}

async fn image_bytes_for_path(p: &Path) -> Response {
    match fs::read(p).await {
        Ok(bytes) => {
            let mut res = Response::new(Body::from(bytes));
            res.headers_mut()
                .insert(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
            res
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Could not read image at {IMAGE_PATH}: {err}"),
        )
            .into_response(),
    }
}
