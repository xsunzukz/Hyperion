# Hyperion 
A blazing-fast, native desktop game launcher for the Epic Games Store, built entirely from scratch in **Rust**, **Tauri 2**, and **React**.

> **Note on Implementation**: This project is an independent clean-room native Rust implementation inspired by the open-source CLI project [Legendary](https://github.com/legendary-gl/legendary) by **derrod** (licensed under GNU GPL v3.0 or later). It contains **zero Python runtime dependencies, zero subprocess wrappers, and zero embedded legacy CLI binaries**. All protocols, manifests, chunks, and launching logic run natively in Rust.

---

## Key Features

- **Native Rust Core**:
  - Direct HTTP/TLS communication using `reqwest` and `rustls`.
  - Binary manifest parsing (`0x44BEC00C`), serializing, and delta patching.
  - Chunk download engine with bounded concurrency (`tokio::sync::Semaphore`), streaming decompression (`flate2`), and direct file offset assembly.
  - Full resume journal support (`.hyperion_resume.txt`) for interrupted downloads.
  - Streaming file integrity verification (SHA1) without loading whole archives into memory.
  - Secure offline and online launch exchange tokens.
  - Cloud save path resolution (`{installdir}`, `{epicid}`, `{appdata}`, `{userdir}`) and synchronization with datastorage service.
  - Local embedded persistence via SQLite (`rusqlite`).
- **Tauri 2 Desktop Shell**:
  - Typed IPC commands and asynchronous event streaming (`download-progress`).
  - Least-privilege capabilities configuration.
  - Native Windows process execution with working directory isolation.
- **Modern Dark UI**:
  - Built with React 19, TypeScript, Vite, and Tailwind CSS.
  - Collapsible sidebar (icon rail, `Ctrl+B`, choice persisted) that stays border-aligned with the top bar.
  - Responsive Grid and List views.
  - Real-time download progress bar with MB/s speeds and ETAs.
  - **Stop running games** from the card, the detail modal or the sidebar's *Running now* list — including
    titles that were started before the launcher (processes are adopted by install folder).
  - Full game maintenance modal: Launch, Verify Integrity, Cloud Save Sync, Open Directory, and Safe Uninstall.
  - Featured build panel on Discover: one-click install of a bare Epic manifest, with the chunk CDN
    resolved automatically (see *CDN resolution* below).

---

## Architecture Overview

```
hyperion/
├── src/                          # React + TypeScript Frontend
│   ├── components/               # UI Components
│   │   ├── Sidebar.tsx           # Navigation & Account Badge
│   │   ├── Header.tsx            # Search, Filter Pills & View Mode
│   │   ├── GameCard.tsx          # Grid & List Game Representation
│   │   ├── GameDetailModal.tsx   # Launch, Verify, Repair, Uninstall
│   │   ├── DownloadsView.tsx     # Active Chunk Stream Queue
│   │   ├── CloudSavesView.tsx    # Datastorage Sync Manager
│   │   ├── SettingsView.tsx      # Downloader Workers & Preferences
│   │   └── LoginModal.tsx        # Epic Games OAuth Flow
│   ├── lib/
│   │   └── ipc.ts                # Typed Tauri IPC & Browser Fallback
│   ├── types.ts                  # Shared Data Models
│   ├── index.css                 # Dark Glassmorphism Styling
│   ├── App.tsx                   # Main Layout & State Orchestration
│   └── main.tsx                  # React Entrypoint
│
└── src-tauri/                    # Native Rust Backend (Tauri 2)
    ├── src/
    │   ├── api.rs                # Epic Games API Client (OAuth, Assets, Manifests)
    │   ├── manifest.rs           # Binary Manifest 0x44BEC00C Parser & Serializer
    │   ├── chunk.rs              # 0xB1FE3AA2 Chunk Reader & Decompressor
    │   ├── downloader.rs         # Bounded Concurrency Chunk Streamer
    │   ├── installer.rs          # File Integrity Verifier & Uninstaller
    │   ├── launcher.rs           # Native Process Launcher & Epic Args
    │   ├── cloud_save.rs         # Path Template Resolver & Cloud Sync
    │   ├── storage.rs            # SQLite Database (Installed Games, Cache)
    │   ├── commands.rs           # Typed Tauri IPC Handlers
    │   ├── state.rs              # AppState Management
    │   ├── models.rs             # Core Data Structures
    │   ├── error.rs              # AppError Types
    │   ├── lib.rs                # App Lifecycle & Handlers Registration
    │   ├── main.rs               # WinMain Entrypoint
    │   └── tests.rs              # Unit & Integration Tests
    ├── capabilities/             # Tauri 2 Security Permissions
    ├── Cargo.toml                # Rust Dependencies
    └── tauri.conf.json           # Tauri Desktop Configuration
```

---

## Steam Integration

Installed Steam titles are discovered from `libraryfolders.vdf` / `appmanifest_*.acf`, and
store metadata comes from Valve's public APIs (no API key required).

**Steam downloads run inside Hyperion** — no Steam client window, no Steam library
involvement, and the files land in the install folder Hyperion was given (default
`<default_install_dir>/<title>` or whatever path the caller passes). The engine is Valve's
headless `SteamCMD`, fetched once into Hyperion's own data directory
(`%LOCALAPPDATA%/Hyperion/steamcmd`) and driven directly by the Rust backend, with live
download speed, byte progress, ETA and integrity validation streamed into the Downloads queue.

Valve only issues depot licences to an authenticated session, so a Steam sign-in is required
before the first download:

- Credentials are entered once per app run in the in-app Steam sign-in prompt.
- **They are verified with Valve before anything is stored**: Hyperion runs a throwaway SteamCMD
  login (`+login … +quit`, no download) and rejects wrong passwords, missing Guard codes and
  rate-limited accounts up front. Only a confirmed sign-in is remembered.
- They live in **process memory only** — never written to SQLite or any file — and are dropped
  when Hyperion exits. Ticking "Remember on this PC" stores an encrypted (DPAPI) copy instead.
- If Steam Guard is enabled, the current authenticator code is requested, and re-requested if
  it expires mid-download.
- SteamCMD caches its own session on the machine after a successful login, so later runs may
  not prompt at all.

Unlike the Epic path (pure Rust, no subprocesses), the Steam path deliberately drives Valve's
tool: a from-scratch implementation would have to reproduce the CM/protobuf protocol, ownership
tickets and per-depot decryption keys, which is what SteamCMD already does.

---

## Antivirus notes

A game launcher does several things heuristic engines dislike, so false positives are worth
explaining:

- **SteamCMD is downloaded and executed.** It is Valve's own signed tool
  (`steamcdn-a.akamaihd.net`), fetched once into `%LOCALAPPDATA%/Hyperion/steamcmd`, but
  "download an executable, then run it" is a classic dropper pattern.
- **The build is unsigned.** Windows SmartScreen and some engines flag unsigned installers; the
  fix is code signing, not code changes.
- **Steam's account files are never read.** The launcher does not open Steam's
  `config/loginusers.vdf`, `ssfn*` files or its run keys — the exact pattern session-stealer
  malware uses. The account name shown in the UI is whatever you signed in with (stored in
  Hyperion's own SQLite settings); Steam's library is located via the `SteamPath` registry value
  and `libraryfolders.vdf`, which contain no credentials.
- What this project *avoided* on purpose: it does not shell out to `taskkill` or `reg query`.
  Stopping games and SteamCMD uses `TerminateProcess` with a toolhelp process-tree walk, and the
  Steam install path is read straight from the registry — see
  [`winproc.rs`](src-tauri/src/winproc.rs) and [`scanner.rs`](src-tauri/src/steam/scanner.rs).
  The launcher also writes no services, scheduled tasks, startup entries or run keys.

### How the launcher watches processes

Process monitoring is deliberately cheap, because a launcher that enumerates every process and
reads its memory counters every few seconds looks exactly like a scanner:

| What | How | How often |
|---|---|---|
| "Is this game still running?" | `OpenProcess(SYNCHRONIZE)` + `WaitForSingleObject(h, 0)` on the PID it already knows | every 3 s (no enumeration, no memory reads) |
| Games started outside the launcher | toolhelp snapshot of process **names**, then `QueryFullProcessImageNameW` only for name matches | startup and once a minute |
| Machine diagnostics | `System::new()` + `refresh_memory()` — totals only | on demand |

Never used anywhere in the codebase: `ReadProcessMemory`/`WriteProcessMemory`,
`CreateRemoteThread`, `SetWindowsHookEx`, DLL injection, keyboard hooks or driver loading.
Earlier versions refreshed `sysinfo`'s full process table (CPU + memory for every process) every
3 seconds; that is gone, along with the `sysinfo` dependency in the polling path.

If Defender flags the app or `steamcmd.exe`, add an exclusion for the launcher folder and for
`%LOCALAPPDATA%/Hyperion`, and report the detection as a false positive.

---

## Build & Run Instructions

### Prerequisites
- Node.js 18+ and npm
- Rust 1.78+ (tested with Rust 1.97.1)
- Windows 10/11, Linux, or macOS
- Git (only needed to clone/push the repository)

### 1. Install Frontend Dependencies
```bash
npm install
```

### 2. Run Tests
```bash
cd src-tauri
cargo test
cd ..
npm run build
```

### 3. Launch Development Server
```bash
# Pure Browser Preview Mode (Mock backend enabled)
npm run dev

# Full Native Desktop App (Tauri 2 + Rust)
npm run tauri dev
```

### 4. Build Production Desktop Installer
```bash
npm run tauri build
```

---

## CDN resolution for custom manifests (`.manifest` installs)

Some community APIs only publish an Epic `.manifest` file; the game data itself still has to
come from a CDN. The launcher therefore **caches manifests on first launch**: the featured
build's manifest is fetched once when the app opens
(`%LOCALAPPDATA%/Hyperion/manifests`, keyed by URL) and every INSTALL afterwards reads it
from disk — no API round-trip, and the manifest still loads when the API is unreachable. The
banner shows a *Manifest cached* chip once it is stored.

When installing from a manifest, the launcher:

1. honours an explicit **CDN Chunk Base URL** if one is supplied;
2. otherwise resolves Epic's CDN for the build — cached catalog entry first, then the Epic
   assets API using both the manifest's app codename and the display title;
3. falls back to the manifest's own directory for self-hosted builds;
4. probes each candidate with a single range request and keeps the first base that actually
   serves the build's chunks (logged as `Using chunk CDN <url>`).

Epic OAuth tokens are attached **only** to `*epicgames*` hosts, never to third-party mirrors.
If no candidate serves the build, the error names the base that was tried and points at the
*CDN Chunk Base URL* field in **Direct Downloader → Options & Manifest**.

---

## Repository notes

- `package-lock.json` and `src-tauri/Cargo.lock` are committed so builds are reproducible.
- Build output (`node_modules/`, `dist/`, `src-tauri/target/`, `src-tauri/gen/schemas/`) and
  local state (`*.db`, `.env*`, `reference/`) are ignored — see `.gitignore`.
- Line endings are normalised to LF via `.gitattributes`; binary assets stay untouched.
- Licence: GNU GPL v3.0 or later — see [LICENSE](LICENSE).

### Pushing to a new remote

```bash
git init -b main
git add .
git commit -m "Hyperion: native Epic launcher (Tauri 2 + Rust + React)"
git remote add origin https://github.com/xsunzukz/Hyperion
git push -u origin main
```

---

## Attribution and Licensing
This project is licensed under the **GNU General Public License v3.0 or later** (GPL-3.0-or-later).
Protocol details, manifest structures, and chunk serialization are based on research from the [Legendary project](https://github.com/legendary-gl/legendary) created by derrod.

Built by **xsunzukz**, with implementation assistance from **DeepSeek Harness** (AI coding agent) —
see [CONTRIBUTORS.md](CONTRIBUTORS.md) for the breakdown of contributions.
