# Hyperion updater Worker

A tiny Cloudflare Worker that serves the updater manifest from the latest GitHub
release at:

`https://github.com/xsunzukz/Hyperion/releases/latest/download/latest.json`

The release workflow uploads `latest.json` next to the signed Windows installer.
The Worker caches the response for five minutes, so GitHub is not hit on every
launch. It does not need R2, KV, a GitHub token, or a database.

## Deploy

From this directory:

```bash
npx wrangler deploy
```

The deploy output prints the `*.workers.dev` URL. The updater endpoint is that
URL with `/latest.json` appended, for example:

`https://hyperion-updater.<account>.workers.dev/latest.json`

`GET /health` returns a small JSON status payload.

## Manifest shape

The Worker forwards the release's `latest.json` unchanged. The Windows entry is
expected to look like:

```json
{
  "version": "1.0.3",
  "notes": "Release notes",
  "pub_date": "2026-10-28T00:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "<contents of the .sig asset>",
      "url": "https://github.com/xsunzukz/Hyperion/releases/download/v1.0.3/Hyperion_1.0.3_x64-setup.exe"
    }
  }
}
```
