export async function fetchJson(url, options) {
  const res = await fetch(url, options);
  if (!res.ok) {
    throw new Error(`Request failed (${res.status}) for ${url}`);
  }
  return res.json();
}

export function getMangaList() {
  return fetchJson('/api/manga');
}

export function getVolumes(manga) {
  return fetchJson(`/api/manga/${encodeURIComponent(manga)}/volumes`);
}

export function getManifest(manga, volume) {
  return fetchJson(`/api/manga/${encodeURIComponent(manga)}/volumes/${volume}/manifest`);
}

