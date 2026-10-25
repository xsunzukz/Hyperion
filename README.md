<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="public/github_banner.png">
  <source media="(prefers-color-scheme: light)" srcset="public/banner_white.png">
  <img src="public/github_banner.png" alt="Hyperion Launcher" width="620">
</picture>

### A game launcher written from scratch in Rust.

Binary manifests, parallel chunk streams and SHA-1 verification — no Python runtime, no CLI wrappers, no embedded store client.

[![Rust](https://img.shields.io/badge/Rust-1.78%2B-ff6600?style=for-the-badge&logo=rust&logoColor=white&labelColor=0a0a0b)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-2.x-24C8DB?style=for-the-badge&logo=tauri&logoColor=white&labelColor=0a0a0b)](https://tauri.app/)
[![React](https://img.shields.io/badge/React-19-61DAFB?style=for-the-badge&logo=react&logoColor=0a0a0b&labelColor=0a0a0b)](https://react.dev/)
[![TypeScript](https://img.shields.io/badge/TypeScript-strict-3178C6?style=for-the-badge&logo=typescript&logoColor=white&labelColor=0a0a0b)](https://www.typescriptlang.org/)
[![Tests](https://img.shields.io/badge/tests-62%20passing-2ea043?style=for-the-badge&labelColor=0a0a0b)](#build--run)
[![License](https://img.shields.io/badge/license-GPL--3.0--or--later-8b5cf6?style=for-the-badge&labelColor=0a0a0b)](LICENSE)

[**Features**](#features) · [**Why it's fast**](#why-it-downloads-fast) · [**Steam**](#steam-integration) · [**Build**](#build--run) · [**Security**](#security--antivirus) · [**Credits**](#credits)

</div>

<br>

---

Hyperion is a native desktop launcher that talks to Epic's services directly: it parses the binary manifest format, downloads content chunks in parallel, decrypts and inflates them, and assembles files by absolute offset — all in Rust, with a Tauri 2 shell and a React 19 interface.

It exists because the interesting part of a launcher is the download pipeline, and rebuilding that pipeline from scratch is a genuinely hard piece of systems work. Every byte on disk arrives through code in this repository.

**No Python. No `legendary` subprocess. No embedded CLI binary.**

<br>

## Features

<table>
<tr>
<td width="50%" valign="top">

**Download engine**

- Parallel chunk fetching with a bounded semaphore (16 workers by default, up to 32 from the UI, hard cap 64)
- Streaming zlib inflate and AES-256-GCM decryption
- Decode and disk writes pushed to the blocking pool, so the network never stalls behind the CPU
- Manual HTTP/TLS via `reqwest` + `rustls`
- Resume journal (`.hyperion_resume.txt`) written per finished file and checked by hash, so a crash resumes and an update only fetches changed files
- Every chunk is checked against the manifest's SHA-1 before it is written, and bad chunks are refetched
- Verify hashes the install against the exact manifest it came from, and Repair re-downloads only the files that fail
- CDN resolution: cached catalog → Epic assets API → manifest-relative, each candidate probed with a range request
- Community manifests are fetched when you open them and kept in a local cache, so an offline install falls back to the last copy

</td>
<td width="50%" valign="top">

**Launcher**

- Epic OAuth sign-in through Epic's own web login — Hyperion never sees your password
- Launch, verify, repair, cloud-sync and safe-uninstall from one modal
- **Stop a running game**, including titles started outside the launcher (matched by install folder)
- Closing the window parks it in the **system tray** (minimising stays a normal minimise), with a live menu: active downloads with percent, speed and ETA, and every installed game — click one to launch it. Left-click toggles the window
- Cloud saves using `{installdir}`, `{epicid}`, `{appdata}`, `{userdir}` path templates
- Library scan for Steam titles via `libraryfolders.vdf` / `appmanifest_*.acf`
- Local state in SQLite — no account database in the cloud
- Single instance: launching the app again raises the running window instead of starting a second copy
- Staged startup splash: runtime files → Epic + Steam library, with real per-stage progress and a "continue anyway" escape hatch
- Sign-in gate after startup, with **Epic sign-in as the primary path** (Epic's own login page handles the password); a one-session offline skip keeps installed games playable

</td>
</tr>
</table>

**Interface** — React 19 + Tailwind, dark glass styling, collapsible icon rail (`Ctrl+B`, remembered), grid and list views, live MB/s and ETA, and a real event stream (`download-progress`) from the Rust core rather than polled guesses.

<br>

## Why it downloads fast

Speed here is a sum of small decisions, not one magic knob:

| Decision | Effect |
|---|---|
| Decode (`zlib`/AES) and file writes run on `spawn_blocking` | CPU work no longer starves the async runtime; other requests keep draining |
| 16 chunk workers by default | High-latency routes need concurrency, not a bigger single stream |
| 15 s connect / 120 s transfer timeouts | A slow-but-progressing chunk is never aborted and restarted from byte 0 |
| `tcp_nodelay`, 30 s keepalive, 64-connection idle pool | Removes per-chunk handshake and Nagle stalls |
| Absolute-offset writes, one handle per worker | No seek contention, no shared cursor between parallel writers |
| Failed writes abort the download | A dropped slice used to be counted as a finished chunk and produced a corrupt install |

<br>

## Architecture

```
hyperion/
├── src/                       React 19 + TypeScript frontend
│   ├── components/            Sidebar, Header, GameCard, GameDetailModal,
│   │                          DownloadsView, CloudSavesView, SettingsView, LoginModal
│   ├── lib/ipc.ts             Typed Tauri IPC + browser mock fallback
│   └── App.tsx                Layout & state orchestration
│
└── src-tauri/src/             Rust core (Tauri 2)
    ├── api.rs                 Epic OAuth, assets API, manifest fetch
    ├── manifest.rs            Binary manifest 0x44BEC00C — parser & serializer
    ├── chunk.rs               Chunk container 0xB1FE3AA2 — inflate & AES-GCM
    ├── downloader.rs          Bounded-concurrency chunk streamer
    ├── installer.rs           Integrity verification & uninstall
    ├── launcher.rs            Native process launch with Epic arguments
    ├── cloud_save.rs          Path templates & datastorage sync
    ├── storage.rs             SQLite persistence
    ├── winproc.rs             TerminateProcess + toolhelp process-tree walk
    └── steam/                 SteamCMD driver, scanner, VDF parser
```

<details>
<summary><b>Protocol notes</b></summary>

<br>

- **Manifests** are the binary `0x44BEC00C` format: a chunk data list (GUID, rolling hash, window size) plus a file manifest list whose parts reference chunk GUIDs with `offset` / `size` / `file_offset`.
- **Chunks** are `0xB1FE3AA2` containers. The header carries a version, compressed size, 128-bit GUID, rolling hash, SHA-1, `stored_as` flags, and — from version 4 — a secret GUID plus a 16-byte GCM tag.
- `stored_as` bit 0 means zlib-compressed, bit 1 means AES-256-GCM encrypted; the nonce is the first 12 bytes of the chunk's SHA-1 and the tag is appended before decryption.
- Files are pre-sized with `set_len`, then every chunk slice is written at its absolute `file_offset`.
- Epic bearer tokens are attached **only** to `*epicgames*` hosts, never to third-party mirrors.

</details>

<br>

## Steam integration

Installed Steam titles are discovered from `libraryfolders.vdf` and `appmanifest_*.acf`, with metadata straight from Valve's public APIs (no key needed).

Steam *downloads* run inside Hyperion — no Steam client window, no Steam library involvement — by driving Valve's headless `SteamCMD`, fetched into `%LOCALAPPDATA%/Hyperion/steamcmd` the first time you use Steam, with speed, byte progress, ETA and validation streamed into the Downloads queue. Valve's archive is only a ~1.6 MB bootstrapper, so the Steam sign-in dialog runs it once (`+quit`) to fetch the real engine while you type — otherwise that ~30 MB download happens inside the Steam Guard window and the login times out.

Valve only issues depot licences to an authenticated session, so a sign-in is required before the first download:

- Credentials are used once per app run and verified with Valve *before* anything is stored — a throwaway `+login … +quit` rejects wrong passwords, missing Guard codes and rate-limited accounts up front.
- They live in **process memory only**, and are dropped when Hyperion exits. Opting into "Remember on this PC" stores a DPAPI-encrypted copy instead.
- Steam Guard codes are requested when needed. SteamCMD caches its own session after a clean successful sign-in, so later downloads authenticate silently.

Unlike the Epic path, this one deliberately delegates to Valve's tool: a from-scratch implementation would have to reproduce the CM/protobuf protocol, ownership tickets and per-depot keys.

<br>

## Build & run

**Prerequisites** — Node 18+, Rust 1.78+ (tested on 1.97.1), and a Windows/Linux/macOS desktop.

```bash
npm install                 # frontend deps
cd src-tauri && cargo test  # 62 Rust tests
cd .. && npm run tauri dev  # full native app
```

| Command | What it does |
|---|---|
| `npm run dev` | Browser preview against the mock IPC backend |
| `npm run tauri dev` | Real desktop app (Rust core + React UI) |
| `npm run tauri build` | Production installer via Tauri bundler |

`package-lock.json` and `Cargo.lock` are committed so builds are reproducible. Line endings are normalised to LF; binary assets are excluded from that.

<br>

## Security & antivirus

A launcher does several things heuristics dislike, so the choices here are deliberate and verifiable in the code:

- **SteamCMD is downloaded and executed** — Valve's own signed tool, fetched from `steamcdn-a.akamaihd.net` and unpacked in-process, but only once you open the Steam sign-in. Nothing is downloaded or run at startup. "Download an executable, then run it" is still a dropper pattern; that is what it is.
- **The build is unsigned**, so SmartScreen will warn. The fix is code signing, not behaviour.
- **The Steam client's account files are never read.** Nothing under the Steam install's `config/` (`loginusers.vdf`, `ssfn*`), no run keys — the exact pattern session-stealer malware uses. Only SteamCMD's *own* config inside `%LOCALAPPDATA%/Hyperion/steamcmd` is checked, to tell whether its sign-in is cached. Scanning this PC's Steam library (`libraryfolders.vdf`, app manifests, `userdata/*/config/localconfig.vdf`) is off until you turn it on.
- **No `taskkill`, no `reg query`, no `tar`, no PowerShell.** Stopping a game uses `TerminateProcess` over a toolhelp process-tree walk, the Steam path comes straight from the registry, archives are unpacked in-process, and folders and links open through `ShellExecute`. The only programs Hyperion starts are your games and SteamCMD.
- **No services, scheduled tasks, startup entries or run keys.**
- **No memory reading or injection** anywhere: no `ReadProcessMemory`, `CreateRemoteThread`, `SetWindowsHookEx`, DLL injection or drivers.

Process monitoring stays cheap on purpose: a known PID is watched with `OpenProcess(SYNCHRONIZE)` + `WaitForSingleObject` every 3 s, and games started outside the launcher are found by process *name* snapshot at startup and once a minute — only processes whose name matches an executable in that game's folder are opened.

If Defender flags the app or `steamcmd.exe`, add an exclusion for the launcher folder and `%LOCALAPPDATA%/Hyperion`, then report the false positive.

<br>

## Repository notes

- Build output (`node_modules/`, `dist/`, `src-tauri/target/`, `src-tauri/gen/schemas/`) and local state (`*.db`, `.env*`, `reference/`) are ignored.
- Branding is generated, not hand-edited: `public/app logos.png` is the source sheet, `scripts/build_logo.py` crops and keys it into `public/hyperion logo.png`, and `scripts/build_icons.py` regenerates the Tauri icon set. Replace the sheet and rerun both.
- `brag-output/` holds locally rendered promo video and licensed audio, so it is gitignored and never published with the source.

<br>

## Scope and disclaimer

Hyperion is an **unofficial, unaffiliated** project. It is not endorsed by, or connected to, Epic Games or Valve.

It is a **client**, not a store and not a DRM bypass: it signs in to *your* account and downloads only the titles that account is entitled to. It includes no content, no keys and no licence circumvention. Protocol understanding is derived from the open-source [Legendary](https://github.com/legendary-gl/legendary) project by **derrod** (GPL-3.0-or-later).

<br>

## Credits

| | |
|---|---|
| **Author** | [xsunzukz](https://github.com/xsunzukz) |
| **Assistance** | DeepSeek Harness (AI coding agent) — breakdown in [CONTRIBUTORS.md](CONTRIBUTORS.md) |
| **Protocol research** | [Legendary](https://github.com/legendary-gl/legendary) by derrod |
| **Licence** | [GPL-3.0-or-later](LICENSE) |

<div align="center">

<br>

**[⬆ back to top](#)** · Built with Rust, Tauri 2 and React — because the download pipeline was the fun part.

</div>
