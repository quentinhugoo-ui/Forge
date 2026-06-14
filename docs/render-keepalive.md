# Render keepalive

Forge keeps the Render resolver warm with two small guards:

- `render.yaml` defines the `forge-keepalive` Render Cron Job. It runs every ten minutes and calls `https://forge-6cai.onrender.com/health`.
- `services/real-estate-resolver` also starts an internal keepalive loop when `RENDER=true` or `FORGE_RENDER_KEEPALIVE=true`.
- The same `/health` cron keeps the resolver warm for both Google Places and
  Banger Google Photorealistic 3D Tiles. Do not cron the tiles endpoint itself:
  it would spend Google Map Tiles quota without a user-visible render.

The Cron Job is the important wake-up path because it sends inbound traffic from outside the sleeping service. The internal loop only runs while the service process is already awake.

Environment knobs:

- `FORGE_KEEPALIVE_URL`: URL used by the Render Cron Job. Defaults to the Forge Render health endpoint in `scripts/render-keepalive.mjs`.
- `FORGE_KEEPALIVE_TIMEOUT_MS`: Cron request timeout. Defaults to `8000`.
- `FORGE_RENDER_KEEPALIVE`: set to `false` or `0` to disable the internal resolver loop.
- `FORGE_RENDER_KEEPALIVE_URL`: URL used by the internal resolver loop. Defaults to `RENDER_EXTERNAL_URL + /health`.
- `FORGE_RENDER_KEEPALIVE_SECONDS`: interval for the internal resolver loop. Defaults to `600`, with a minimum of `60`.
- `GOOGLE_MAP_TILES_API_KEY` or `FORGE_GOOGLE_MAP_TILES_API_KEY`: server-side key
  used by the Banger proxy endpoint at `/api/banger/google-tiles/root.json`.

No API keys or Render secrets are stored in this repository.
