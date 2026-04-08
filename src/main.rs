use axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use std::{
    io,
    path::{self, Path, PathBuf},
    sync::Arc,
};
use tokio::{fs, sync::RwLock};

const IMAGE_PATH: &str = "/home/koushikk/Desktop/akane.jpg";
const MANGA_ROOT: &str = "/home/koushikk/MANGA/Kingdom/";
const SELECTED_MANGA: &str = "Kingdom";
const BOOKMARKS_PATH: &str = "bookmarks.json";
const INDEX_HTML: &str = include_str!("../static/index.html");
const INDEX_JS: &str = include_str!("../static/index.js");

#[derive(Clone)]
struct AppState {
    pages: Arc<Vec<Page>>,
    steps: Arc<Vec<ViewStep>>,
    current_step: Arc<RwLock<usize>>,
    current_volume: usize,
}
#[derive(Clone, Copy)]
enum ViewStep {
    Single(usize),
    Spread { right: usize, left: usize },
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Bookmark {
    volume: usize,
    kind: String,
    right_path: String,
    left_path: Option<String>,
}

#[tokio::main]
async fn main() {
    let manga_dir = PathBuf::from(MANGA_ROOT).join(SELECTED_MANGA);

    let volume_number: usize = 45;
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
        .route("/api/akane", get(right_page_bytes))
        .route("/api/right", get(right_page_bytes))
        .route("/api/left", get(left_page_bytes))
        .route("/api/next", get(next_page))
        .route("/api/prev", get(prev_page))
        .route("/api/bookmark", post(add_bookmark))
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
struct Page {
    number: usize,
    path: PathBuf,
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

async fn add_bookmark(State(state): State<AppState>) -> Response {
    let step_idx = *state.current_step.read().await;

    let Some(step) = state.steps.get(step_idx) else {
        return (StatusCode::NOT_FOUND, "No step found").into_response();
    };

    let bookmark = match *step {
        ViewStep::Single(i) => {
            let Some(page) = state.pages.get(i) else {
                return (StatusCode::NOT_FOUND, "No page found").into_response();
            };
            Bookmark {
                volume: state.current_volume,
                kind: "single".to_string(),
                right_path: page.path.display().to_string(),
                left_path: None,
            }
        }
        ViewStep::Spread { right, left } => {
            let Some(right_page) = state.pages.get(right) else {
                return (StatusCode::NOT_FOUND, "No right page found").into_response();
            };
            let Some(left_page) = state.pages.get(left) else {
                return (StatusCode::NOT_FOUND, "No left page found").into_response();
            };
            Bookmark {
                volume: state.current_volume,
                kind: "spread".to_string(),
                right_path: right_page.path.display().to_string(),
                left_path: Some(left_page.path.display().to_string()),
            }
        }
    };

    let mut bookmarks: Vec<Bookmark> = match fs::read_to_string(BOOKMARKS_PATH).await {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    };

    bookmarks.push(bookmark);

    let body = match serde_json::to_string_pretty(&bookmarks) {
        Ok(json) => json,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to serialize bookmarks: {err}"),
            )
                .into_response();
        }
    };

    match fs::write(BOOKMARKS_PATH, body).await {
        Ok(_) => (StatusCode::CREATED, "bookmark saved").into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to write bookmarks: {err}"),
        )
            .into_response(),
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
