use axum::{
    Router,
    body::Body,
    extract::State,
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use std::{
    io,
    path::{self, PathBuf},
    sync::Arc,
};
use tokio::{fs, sync::RwLock};

const IMAGE_PATH: &str = "/home/koushikk/Desktop/akane.jpg";

#[derive(Clone)]
struct AppState {
    pages: Arc<Vec<Page>>,
    current_page: Arc<RwLock<usize>>,
}
// so i should just make a path which has the next page
// right now im just updating the bytes for the 1 image wihc is showing

#[tokio::main]
async fn main() {
    let volume_path = select_volume(list_volumes(), 1);
    let pages = chosen_volume(volume_path.as_path()).expect("failed to read selected volume");

    let state = AppState {
        pages: Arc::new(pages),
        current_page: Arc::new(RwLock::new(0)),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/api/akane", get(right_page_bytes))
        .route("/api/right", get(right_page_bytes))
        .route("/api/left", get(left_page_bytes))
        .route("/api/next", get(next_page))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("failed to bind to 127.0.0.1:3000");

    println!("Server running on http://127.0.0.1:3000");

    axum::serve(listener, app).await.expect("server crashed");
}

fn gimme() -> PathBuf {
    let bomba = select_volume(list_volumes(), 4);
    let fuck = chosen_volume(bomba.as_path()).expect("FUUUUCK");
    //   quick(fuck);
    let first_page = fuck[0].path.clone();
    first_page
}

async fn index() -> Html<&'static str> {
    Html(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Axum Raw Image Bytes</title>
</head>
<body style="margin:0;min-height:100vh;display:flex;align-items:center;justify-content:center;background:#111;">
  <div style="display:flex;gap:8px;align-items:flex-start;">
    <img id="rightPhoto" alt="Right page" width="700" />
    <img id="leftPhoto" alt="Left page" width="700" />
  </div>

  <script>
    async function loadImageInto(id, url, allowNoContent = false) {
      const res = await fetch(url);
      const img = document.getElementById(id);

      if (allowNoContent && res.status === 204) {
        if (img.dataset.url) URL.revokeObjectURL(img.dataset.url);
        img.dataset.url = '';
        img.removeAttribute('src');
        return;
      }

      if (!res.ok) {
        throw new Error(`Failed to fetch ${url}: ${res.status}`);
      }

      const bytes = await res.arrayBuffer();
      const blob = new Blob([bytes], { type: 'image/jpeg' });
      const objectUrl = URL.createObjectURL(blob);

      if (img.dataset.url) URL.revokeObjectURL(img.dataset.url);
      img.dataset.url = objectUrl;
      img.src = objectUrl;
    }

    async function loadSpread() {
      await loadImageInto('rightPhoto', '/api/right');
      await loadImageInto('leftPhoto', '/api/left', true);
    }

    async function nextPage() {
      const res = await fetch('/api/next');
      if (res.status === 200) {
        await loadSpread();
      } else if (res.status === 204) {
        console.log('Already at last spread');
      } else {
        console.error('Failed to go to next spread', res.status);
      }
    }

    window.addEventListener('keydown', async (e) => {
      if (e.code === 'Space') {
        e.preventDefault();
        await nextPage();
      }
    });

    loadSpread().catch(err => {
      document.body.innerHTML = `<pre>${err}</pre>`;
      console.error(err);
    });
  </script>
</body>
</html>
"#,
    )
}

fn list_volumes() -> std::fs::ReadDir {
    println!("Listing volumes");
    let volumes = std::fs::read_dir("/home/koushikk/MANGA/nana").expect("Erorring getting volumes");
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

async fn next_page(State(state): State<AppState>) -> Response {
    let mut idx = state.current_page.write().await;

    if *idx + 2 < state.pages.len() {
        *idx += 2;
        (StatusCode::OK, format!("next spread worked mud {}", *idx)).into_response()
    } else {
        (StatusCode::NO_CONTENT, "Already at last spread").into_response()
    }
}

async fn right_page_bytes(State(state): State<AppState>) -> Response {
    let idx = *state.current_page.read().await;
    image_bytes_for_idx(&state, idx).await
}

async fn left_page_bytes(State(state): State<AppState>) -> Response {
    let idx = state.current_page.read().await.saturating_add(1);

    if idx >= state.pages.len() {
        return (StatusCode::NO_CONTENT, "No left page for this spread").into_response();
    }

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
