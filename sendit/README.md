# sendit (Python + uv)

`sendit` is only the hosting app.
It serves local image files to the yorknew backend over WebSocket.

Flow:

`sendit -> yorknew backend -> frontend`

## Hardcoded config (for now)

In `sendit.py`:
- Backend WS: `ws://127.0.0.1:3000/hosting/ws?host_id=local`
- Manga root: `/home/koushikk/MANGA/Usogui`

## Run

```bash
cd /home/koushikk/yorknew/sendit
uv run sendit
```

## Notes

- Path traversal is blocked (`..`, absolute paths).
- File bytes are sent as base64 websocket chunks (`stream_chunk`).
- If backend restarts, sendit auto-reconnects.
