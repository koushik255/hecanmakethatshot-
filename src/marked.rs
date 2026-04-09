use axum::{
    body::Body,
    extract::State,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use tokio::fs;

use crate::{AppState, ViewStep};

const BOOKMARKS_PATH: &str = "bookmarks.json";
const PAGEMARKS_PATH: &str = "pagemarks.json";

#[derive(serde::Serialize, serde::Deserialize)]
struct Bookmark {
    volume: usize,
    kind: String,
    right_path: String,
    left_path: Option<String>,
}

//left-path is optioned on both becuaes its possible for it to
//not be shown (if its only 1 page on screen then would
//save to rightpath)
#[derive(serde::Serialize, serde::Deserialize)]
struct PageMark {
    pathleft: Option<String>,
    pathright: String,
    volume: usize,
}

pub(crate) async fn add_bookmark(State(state): State<AppState>) -> Response {
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

pub(crate) async fn add_pagemark(State(state): State<AppState>) -> Response {
    let step_idx = *state.current_step.read().await;

    let Some(step) = state.steps.get(step_idx) else {
        return (StatusCode::NOT_FOUND, "No step found").into_response();
    };

    let pagemark = match *step {
        ViewStep::Single(i) => {
            let Some(page) = state.pages.get(i) else {
                return (StatusCode::NOT_FOUND, "No page found").into_response();
            };
            PageMark {
                pathleft: None,
                pathright: page.path.display().to_string(),
                volume: state.current_volume,
            }
        }
        ViewStep::Spread { right, left } => {
            let Some(right_page) = state.pages.get(right) else {
                return (StatusCode::NOT_FOUND, "No right page found").into_response();
            };
            let Some(left_page) = state.pages.get(left) else {
                return (StatusCode::NOT_FOUND, "No left page found").into_response();
            };
            PageMark {
                pathleft: Some(left_page.path.display().to_string()),
                pathright: right_page.path.display().to_string(),
                volume: state.current_volume,
            }
        }
    };

    let mut pagemarks: Vec<PageMark> = match fs::read_to_string(PAGEMARKS_PATH).await {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    };

    pagemarks.push(pagemark);

    let body = match serde_json::to_string_pretty(&pagemarks) {
        Ok(json) => json,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to serialize pagemarks: {err}"),
            )
                .into_response();
        }
    };

    match fs::write(PAGEMARKS_PATH, body).await {
        Ok(_) => (StatusCode::CREATED, "pagemark saved").into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to write pagemarks: {err}"),
        )
            .into_response(),
    }
}

pub(crate) async fn list_pagemarks() -> Response {
    let mut pagemarks: Vec<PageMark> = match fs::read_to_string(PAGEMARKS_PATH).await {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    };

    pagemarks.reverse();

    let body = match serde_json::to_string_pretty(&pagemarks) {
        Ok(json) => json,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to serialize pagemarks: {err}"),
            )
                .into_response();
        }
    };

    let mut res = Response::new(Body::from(body));
    res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    res
}
