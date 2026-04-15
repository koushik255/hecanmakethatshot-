# hecanmakethatshot-

## Runtime config

- `BACKEND_BIND_ADDR` (default: `127.0.0.1:3000`)
- `MANGA_ROOT` (default: `/home/koushikk/MANGA`)

## Tailscale split-machine flow

- Computer A (`sendit` hosting app with manga files)
- Computer B (Rust backend)
- Computer C (frontend/browser)

Start backend on Computer B:

```bash
BACKEND_BIND_ADDR=0.0.0.0:3000 cargo run
```

Start `sendit` on Computer A (replace `<backend-tailscale-ip>`):

```bash
cd sendit
SENDIT_BACKEND_WS=ws://<backend-tailscale-ip>:3000/hosting/ws?host_id=usogui-host \
SENDIT_MANGA_ROOT=/mnt/d/MANGA/Usogui \
SENDIT_HOST_ID=usogui-host \
uv run sendit
```
