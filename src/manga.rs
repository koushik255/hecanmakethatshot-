use axum::http::StatusCode;
use std::{
    env,
    io,
    path::{Path, PathBuf},
};

pub(crate) const DEFAULT_MANGA_ROOT: &str = "/home/koushikk/MANGA";

#[derive(Clone, Copy)]
pub(crate) enum ViewStep {
    Single(usize),
    Spread { right: usize, left: usize },
}

#[derive(Debug, Clone)]
pub(crate) struct Page {
    pub(crate) path: PathBuf,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl Page {
    pub(crate) fn is_landscape(&self) -> bool {
        self.width > self.height
    }
}

pub(crate) fn is_safe_name(name: &str) -> bool {
    !name.is_empty() && !name.contains("..") && !name.contains('/') && !name.contains('\\')
}

pub(crate) fn list_available_manga() -> io::Result<Vec<String>> {
    let mut manga = Vec::new();
    let root = manga_root();

    for entry in std::fs::read_dir(&root)? {
        let path = entry?.path();
        if !path.is_dir() {
            continue;
        }

        if find_volume_root(&path).is_err() {
            continue;
        }

        if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
            manga.push(name.to_string());
        }
    }

    manga.sort();
    Ok(manga)
}

pub(crate) fn manga_dir(name: &str) -> PathBuf {
    manga_root().join(name)
}

pub(crate) fn list_volumes_for_manga(name: &str) -> io::Result<Vec<PathBuf>> {
    let manga_dir = manga_dir(name);
    let volume_root = find_volume_root(&manga_dir)?;
    read_sorted_volume_dirs(&volume_root)
}

pub(crate) fn load_volume_pages(name: &str, volume: usize) -> io::Result<Vec<Page>> {
    let manga_dir = manga_dir(name);
    let volume_root = find_volume_root(&manga_dir)?;
    let volume_path = resolve_volume_path(&volume_root, volume)?;
    chosen_volume(&volume_path)
}

pub(crate) fn find_volume_root(manga_dir: &Path) -> io::Result<PathBuf> {
    let mut current = manga_dir.to_path_buf();

    loop {
        let volumes = read_sorted_volume_dirs(&current)?;
        if !volumes.is_empty() {
            return Ok(current);
        }

        let mut child_dirs = Vec::new();
        for entry in std::fs::read_dir(&current)? {
            let path = entry?.path();
            if path.is_dir() {
                child_dirs.push(path);
            }
        }

        child_dirs.sort();

        if child_dirs.len() == 1 {
            current = child_dirs.remove(0);
            continue;
        }

        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Could not find volume folders in {}", manga_dir.display()),
        ));
    }
}

pub(crate) fn read_sorted_volume_dirs(manga_dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut dirs = Vec::new();

    for entry in std::fs::read_dir(manga_dir)? {
        let path = entry?.path();
        if path.is_dir() && volume_number(&path) != u32::MAX {
            dirs.push(path);
        }
    }

    dirs.sort_by_key(|p| volume_number(p));
    Ok(dirs)
}

pub(crate) fn resolve_volume_path(manga_dir: &Path, volume: usize) -> io::Result<PathBuf> {
    let volumes = read_sorted_volume_dirs(manga_dir)?;

    if volume == 0 || volume > volumes.len() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Volume {volume} does not exist"),
        ));
    }

    Ok(volumes[volume - 1].clone())
}

pub(crate) fn map_io_error(err: io::Error) -> (StatusCode, String) {
    let status = match err.kind() {
        io::ErrorKind::NotFound => StatusCode::NOT_FOUND,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };

    (status, err.to_string())
}

pub(crate) fn content_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => "application/octet-stream",
    }
}

fn volume_number(path: &Path) -> u32 {
    path.file_name()
        .and_then(|s| s.to_str())
        .and_then(|name| name.rsplit('_').next())
        .and_then(|n| n.parse::<u32>().ok())
        .unwrap_or(u32::MAX)
}

fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| {
            matches!(
                ext.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "webp" | "gif"
            )
        })
        .unwrap_or(false)
}

fn collect_images_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();

        if path.is_dir() {
            collect_images_recursive(&path, out)?;
        } else if path.is_file() && is_image_file(&path) {
            out.push(path);
        }
    }
    Ok(())
}

pub(crate) fn chosen_volume(cv: &Path) -> io::Result<Vec<Page>> {
    let top_entries: Vec<PathBuf> = std::fs::read_dir(cv)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;

    let mut top_level_images: Vec<PathBuf> = top_entries
        .iter()
        .filter(|p| p.is_file() && is_image_file(p))
        .cloned()
        .collect();

    let mut paths: Vec<PathBuf> = if !top_level_images.is_empty() {
        top_level_images.sort();
        top_level_images
    } else {
        let mut nested = Vec::new();
        collect_images_recursive(cv, &mut nested)?;
        nested
    };

    paths.sort();

    let pages = paths
        .into_iter()
        .map(|path| {
            let (width, height) = image::image_dimensions(&path).unwrap_or((0, 0));
            Page {
                path,
                width,
                height,
            }
        })
        .collect();

    Ok(pages)
}

pub(crate) fn build_view_steps(pages: &[Page]) -> Vec<ViewStep> {
    let mut steps = Vec::new();
    if pages.is_empty() {
        return steps;
    }

    let last = pages.len() - 1;
    let mut i = 0;

    while i < pages.len() {
        let solo = i == 0 || i == last || pages[i].is_landscape();

        if solo {
            steps.push(ViewStep::Single(i));
            i += 1;
            continue;
        }

        if i < last && i + 1 != last && !pages[i + 1].is_landscape() {
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

fn manga_root() -> PathBuf {
    env::var("MANGA_ROOT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_MANGA_ROOT))
}
