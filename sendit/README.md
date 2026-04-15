# sendit (Python + uv)

`sendit` is only the hosting app.
It serves local image files to the yorknew backend over WebSocket.

Flow:

`sendit -> yorknew backend -> frontend`

## Config (environment variables)

- `SENDIT_BACKEND_WS` (default: `ws://127.0.0.1:3000/hosting/ws?host_id=local`)
- `SENDIT_MANGA_ROOT` (default: `/home/koushikk/MANGA/Usogui`)
- `SENDIT_HOST_ID` (default: `local`)

## Run

```bash
cd /home/koushikk/yorknew/sendit
uv run sendit
```

## Example (Tailscale to remote backend)

```bash
SENDIT_BACKEND_WS=ws://100.101.102.103:3000/hosting/ws?host_id=usogui-host \
SENDIT_MANGA_ROOT=/mnt/d/MANGA/Usogui \
SENDIT_HOST_ID=usogui-host \
uv run sendit
```

## Notes

- Path traversal is blocked (`..`, absolute paths).
- File bytes are sent as base64 websocket chunks (`stream_chunk`).
- If backend restarts, sendit auto-reconnects.
