import asyncio
import base64
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
from typing import Any

import websockets

DEFAULT_BACKEND_WS = "ws://127.0.0.1:3000/hosting/ws?host_id=local"
DEFAULT_MANGA_ROOT = "/home/koushikk/MANGA/Usogui"
DEFAULT_HELLO_HOST_ID = "local"
CHUNK_SIZE = 64 * 1024
RECONNECT_DELAY_SECONDS = 2


def sanitize_relative_path(raw: str) -> Path | None:
    if not raw:
        return None

    p = PurePosixPath(raw)
    if p.is_absolute():
        return None

    safe_parts: list[str] = []
    for part in p.parts:
        if part in ("", "."):
            continue
        if part == "..":
            return None
        safe_parts.append(part)

    if not safe_parts:
        return None

    return Path(*safe_parts)


def is_image_file(path: Path) -> bool:
    return path.suffix.lower() in {".jpg", ".jpeg", ".png", ".webp", ".gif"}


def content_type_for_path(path: Path) -> str:
    suffix = path.suffix.lower()
    if suffix in {".jpg", ".jpeg"}:
        return "image/jpeg"
    if suffix == ".png":
        return "image/png"
    if suffix == ".webp":
        return "image/webp"
    if suffix == ".gif":
        return "image/gif"
    return "application/octet-stream"


def volume_number(path: Path) -> int:
    try:
        return int(path.name.rsplit("_", 1)[-1])
    except ValueError:
        return 2**31 - 1


def read_sorted_volume_dirs(root: Path) -> list[Path]:
    dirs = [entry for entry in root.iterdir() if entry.is_dir() and volume_number(entry) != 2**31 - 1]
    return sorted(dirs, key=volume_number)


def find_volume_root(manga_dir: Path) -> Path:
    current = manga_dir
    while True:
        volumes = read_sorted_volume_dirs(current)
        if volumes:
            return current

        child_dirs = sorted([entry for entry in current.iterdir() if entry.is_dir()])
        if len(child_dirs) == 1:
            current = child_dirs[0]
            continue

        raise FileNotFoundError(f"Could not find volume folders in {manga_dir}")


def collect_images_recursive(dir_path: Path, out: list[Path]) -> None:
    for entry in sorted(dir_path.iterdir()):
        if entry.is_dir():
            collect_images_recursive(entry, out)
        elif entry.is_file() and is_image_file(entry):
            out.append(entry)


def chosen_volume(volume_dir: Path) -> list[Path]:
    top_entries = sorted(list(volume_dir.iterdir()))
    top_images = [entry for entry in top_entries if entry.is_file() and is_image_file(entry)]

    if top_images:
        return sorted(top_images)

    nested: list[Path] = []
    collect_images_recursive(volume_dir, nested)
    return sorted(nested)


def page_id_for(host_id: str, manga: str, volume: int, index: int, rel_path: str) -> str:
    seed = f"{host_id}|{manga}|{volume}|{index}|{rel_path}".encode("utf-8")
    digest = hashlib.sha256(seed).digest()
    return base64.urlsafe_b64encode(digest).decode("ascii").rstrip("=")


def build_catalog(root: Path, host_id: str) -> tuple[str, list[dict[str, Any]], dict[str, Path]]:
    manga = root.name
    volume_root = find_volume_root(root)
    volume_dirs = read_sorted_volume_dirs(volume_root)

    volumes: list[dict[str, Any]] = []
    page_lookup: dict[str, Path] = {}

    for volume_index, volume_dir in enumerate(volume_dirs, start=1):
        images = chosen_volume(volume_dir)
        pages: list[dict[str, Any]] = []

        for idx, image_path in enumerate(images):
            rel = image_path.relative_to(root).as_posix()
            page_id = page_id_for(host_id, manga, volume_index, idx, rel)

            pages.append(
                {
                    "page_id": page_id,
                    "index": idx,
                    "is_landscape": False,
                    "content_type": content_type_for_path(image_path),
                }
            )
            page_lookup[page_id] = image_path

        volumes.append(
            {
                "number": volume_index,
                "label": volume_dir.name,
                "pages": pages,
            }
        )

    return manga, volumes, page_lookup


async def send_json(ws: websockets.WebSocketClientProtocol, payload: dict[str, Any]) -> None:
    await ws.send(json.dumps(payload, separators=(",", ":")))


async def send_error(
    ws: websockets.WebSocketClientProtocol, request_id: str, error: str
) -> None:
    await send_json(
        ws,
        {
            "type": "stream_error",
            "request_id": request_id,
            "error": error,
        },
    )


async def stream_file(
    ws: websockets.WebSocketClientProtocol,
    root: Path,
    request_id: str,
    relative_path: str,
) -> None:
    safe_rel = sanitize_relative_path(relative_path)
    if safe_rel is None:
        await send_error(ws, request_id, "invalid path")
        return

    full_path = (root / safe_rel).resolve()

    try:
        full_path.relative_to(root)
    except ValueError:
        await send_error(ws, request_id, "path escaped root")
        return

    await stream_bytes_for_path(ws, request_id, full_path)


async def stream_bytes_for_path(
    ws: websockets.WebSocketClientProtocol, request_id: str, full_path: Path
) -> None:
    if not full_path.is_file():
        await send_error(ws, request_id, f"file not found: {full_path}")
        return

    try:
        with full_path.open("rb") as f:
            while True:
                chunk = f.read(CHUNK_SIZE)
                if not chunk:
                    break

                await send_json(
                    ws,
                    {
                        "type": "stream_chunk",
                        "request_id": request_id,
                        "data": base64.b64encode(chunk).decode("ascii"),
                        "last": False,
                    },
                )

        await send_json(
            ws,
            {
                "type": "stream_chunk",
                "request_id": request_id,
                "data": "",
                "last": True,
            },
        )
    except Exception as exc:  # noqa: BLE001
        await send_error(ws, request_id, f"stream failed: {exc}")


async def run_forever(backend_ws: str, root: Path, hello_host_id: str) -> None:
    manga, volumes, page_lookup = build_catalog(root, hello_host_id)

    while True:
        try:
            print(f"hosting app root: {root}")
            print(f"connecting to {backend_ws}")

            async with websockets.connect(
                backend_ws,
                max_size=None,
                ping_interval=20,
                ping_timeout=20,
            ) as ws:
                await send_json(ws, {"type": "hello", "host_id": hello_host_id})
                await send_json(
                    ws,
                    {
                        "type": "register_catalog",
                        "manga": manga,
                        "volumes": volumes,
                    },
                )

                async for raw in ws:
                    try:
                        msg = json.loads(raw)
                    except json.JSONDecodeError as exc:
                        print(f"invalid request payload: {exc}")
                        continue

                    msg_type = msg.get("type")
                    if msg_type == "start_stream":
                        request_id = msg.get("request_id")
                        rel_path = msg.get("path")

                        if not isinstance(request_id, str) or not isinstance(rel_path, str):
                            print("invalid start_stream payload")
                            continue

                        await stream_file(ws, root, request_id, rel_path)
                        continue

                    if msg_type == "start_stream_by_id":
                        request_id = msg.get("request_id")
                        page_id = msg.get("page_id")

                        if not isinstance(request_id, str) or not isinstance(page_id, str):
                            print("invalid start_stream_by_id payload")
                            continue

                        full_path = page_lookup.get(page_id)
                        if full_path is None:
                            await send_error(ws, request_id, f"page id not found: {page_id}")
                            continue

                        await stream_bytes_for_path(ws, request_id, full_path)

            print("backend disconnected")
        except Exception as exc:  # noqa: BLE001
            print(f"connection error: {exc}")

        await asyncio.sleep(RECONNECT_DELAY_SECONDS)


def main() -> None:
    backend_ws = os.getenv("SENDIT_BACKEND_WS", DEFAULT_BACKEND_WS)
    manga_root = os.getenv("SENDIT_MANGA_ROOT", DEFAULT_MANGA_ROOT)
    hello_host_id = os.getenv("SENDIT_HOST_ID", DEFAULT_HELLO_HOST_ID)

    root = Path(manga_root).resolve()
    if not root.exists() or not root.is_dir():
        raise SystemExit(f"root path does not exist or is not a directory: {root}")
    asyncio.run(run_forever(backend_ws, root, hello_host_id))


if __name__ == "__main__":
    main()
