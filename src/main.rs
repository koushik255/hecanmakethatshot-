mod manga;

use axum::{
    Router,
    body::Body,
    extract::Path as AxumPath,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use manga::{
    ViewStep, build_view_steps, content_type_for_path, is_safe_name, list_available_manga,
    list_volumes_for_manga, load_volume_pages, map_io_error,
};
use std::path::Path;
use tokio::fs;

#[derive(serde::Serialize)]
struct MangaItem {
    name: String,
}

#[derive(serde::Serialize)]
struct VolumeItem {
    number: usize,
    label: String,
}

#[derive(serde::Serialize)]
struct ManifestPage {
    index: usize,
    image_url: String,
    is_landscape: bool,
}

#[derive(serde::Serialize)]
#[serde(tag = "kind")]
enum ManifestStep {
    #[serde(rename = "single")]
    Single { page: usize },
    #[serde(rename = "spread")]
    Spread { right: usize, left: usize },
}

#[derive(serde::Serialize)]
struct VolumeManifest {
    manga: String,
    volume: usize,
    page_count: usize,
    pages: Vec<ManifestPage>,
    steps: Vec<ManifestStep>,
}


#[tokio::main]
async fn main() {

    let app = Router::new()
        .route("/api/manga", get(list_manga))
        .route("/api/manga/{name}/volumes", get(list_manga_volumes))
        .route(
            "/api/manga/{name}/volumes/{volume}/manifest",
            get(volume_manifest),
        )
        .route(
            "/api/manga/{name}/volumes/{volume}/pages/{page}",
            get(page_image),
        );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("failed to bind to 127.0.0.1:3000");

    println!("Server running on http://127.0.0.1:3000");

    axum::serve(listener, app).await.expect("server crashed");
}

async fn list_manga() -> Response {
    match list_available_manga() {
        Ok(manga) => axum::Json(
            manga
                .into_iter()
                .map(|name| MangaItem { name })
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to read manga root: {err}"),
        )
            .into_response(),
    }
}

async fn list_manga_volumes(AxumPath(name): AxumPath<String>) -> Response {
    if !is_safe_name(&name) {
        return (StatusCode::BAD_REQUEST, "Invalid manga name").into_response();
    }

    match list_volumes_for_manga(&name) {
        Ok(volumes) => {
            let items: Vec<VolumeItem> = volumes
                .iter()
                .enumerate()
                .map(|(idx, path)| VolumeItem {
                    number: idx + 1,
                    label: path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Unknown")
                        .to_string(),
                })
                .collect();

            axum::Json(items).into_response()
        }
        Err(err) => map_io_error(err).into_response(),
    }
}

async fn volume_manifest(AxumPath((name, volume)): AxumPath<(String, usize)>) -> Response {
    if !is_safe_name(&name) {
        return (StatusCode::BAD_REQUEST, "Invalid manga name").into_response();
    }

    let pages = match load_volume_pages(&name, volume) {
        Ok(pages) => pages,
        Err(err) => return map_io_error(err).into_response(),
    };

    let steps = build_view_steps(&pages);

    let manifest_pages: Vec<ManifestPage> = pages
        .iter()
        .enumerate()
        .map(|(idx, page)| ManifestPage {
            index: idx,
            image_url: format!("/api/manga/{name}/volumes/{volume}/pages/{idx}"),
            is_landscape: page.is_landscape(),
        })
        .collect();

    axum::Json(VolumeManifest {
        manga: name,
        volume,
        page_count: manifest_pages.len(),
        pages: manifest_pages,
        steps: to_manifest_steps(&steps),
    })
    .into_response()
}

async fn page_image(AxumPath((name, volume, page)): AxumPath<(String, usize, usize)>) -> Response {
    if !is_safe_name(&name) {
        return (StatusCode::BAD_REQUEST, "Invalid manga name").into_response();
    }

    let pages = match load_volume_pages(&name, volume) {
        Ok(pages) => pages,
        Err(err) => return map_io_error(err).into_response(),
    };

    let Some(selected_page) = pages.get(page) else {
        return (StatusCode::NOT_FOUND, "Page not found").into_response();
    };

    image_bytes_for_path(&selected_page.path).await
}

async fn image_bytes_for_path(path: &Path) -> Response {
    match fs::read(path).await {
        Ok(bytes) => {
            let mut res = Response::new(Body::from(bytes));
            res.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(content_type_for_path(path)),
            );
            res
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to read image: {err}"),
        )
            .into_response(),
    }
}

fn to_manifest_steps(steps: &[ViewStep]) -> Vec<ManifestStep> {
    steps
        .iter()
        .map(|step| match *step {
            ViewStep::Single(page) => ManifestStep::Single { page },
            ViewStep::Spread { right, left } => ManifestStep::Spread { right, left },
        })
        .collect()
}
