import asyncio
import base64
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

                async for raw in ws:
                    try:
                        msg = json.loads(raw)
                    except json.JSONDecodeError as exc:
                        print(f"invalid request payload: {exc}")
                        continue

                    if msg.get("type") != "start_stream":
                        continue

                    request_id = msg.get("request_id")
                    rel_path = msg.get("path")

                    if not isinstance(request_id, str) or not isinstance(rel_path, str):
                        print("invalid start_stream payload")
                        continue

                    await stream_file(ws, root, request_id, rel_path)

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
