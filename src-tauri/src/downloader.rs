use crate::chunk::Chunk;
use crate::error::{AppError, Result};
use crate::installer::{safe_join, same_dir, Installer, RESUME_JOURNAL};
use crate::manifest::{ChunkInfo, FileManifest, Manifest};
use crate::models::{DownloadProgress, DownloadStatus, InstalledGame};
use crate::storage::Storage;
use reqwest::Client;
use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex, Semaphore};
use tracing::{error, info, warn};

pub struct ActiveDownload {
    pub app_name: String,
    pub title: String,
    pub install_dir: PathBuf,
    pub cancel_flag: Arc<AtomicBool>,
    pub pause_flag: Arc<AtomicBool>,
    pub progress: Arc<Mutex<DownloadProgress>>,
    pub app_handle: AppHandle,
}

pub struct DownloadManager {
    client: Client,
    active_downloads: Mutex<HashMap<String, Arc<ActiveDownload>>>,
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadManager {
    pub fn new() -> Self {
        // Tuned for many small parallel chunk fetches. Epic's CDN stalls on
        // cold chunks of old builds: a transfer can sit with no bytes arriving
        // for minutes, and with a 120 s ceiling one stalled chunk held a worker
        // that long (measured: 0.5 MiB/s on a 200 Mbit line). A transfer that
        // goes quiet for 15 s is dropped and retried; one that keeps receiving
        // data — a slow link — is never cut off before the 90 s cap.
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(90))
            .tcp_nodelay(true)
            .tcp_keepalive(Duration::from_secs(30))
            .pool_max_idle_per_host(64)
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            active_downloads: Mutex::new(HashMap::new()),
        }
    }

    pub async fn get_download_progress(&self, app_name: &str) -> Option<DownloadProgress> {
        let downloads = self.active_downloads.lock().await;
        if let Some(dl) = downloads.get(app_name) {
            let p = dl.progress.lock().await;
            Some(p.clone())
        } else {
            None
        }
    }

    pub async fn get_all_active(&self) -> Vec<DownloadProgress> {
        let downloads = self.active_downloads.lock().await;
        let mut list = Vec::new();
        for dl in downloads.values() {
            let p = dl.progress.lock().await;
            list.push(p.clone());
        }
        list
    }

    pub async fn cancel_download(&self, app_name: &str) -> Result<()> {
        let mut downloads = self.active_downloads.lock().await;
        if let Some(dl) = downloads.remove(app_name) {
            dl.cancel_flag.store(true, Ordering::SeqCst);
            let mut p = dl.progress.lock().await;
            p.status = DownloadStatus::Cancelled;
            p.speed_bytes_per_sec = 0;
            p.eta_seconds = 0;
            let _ = dl.app_handle.emit("download-progress", p.clone());
            info!("Cancelled download for {}", app_name);
        }
        Ok(())
    }

    pub async fn pause_download(&self, app_name: &str) -> Result<()> {
        let downloads = self.active_downloads.lock().await;
        if let Some(dl) = downloads.get(app_name) {
            dl.pause_flag.store(true, Ordering::SeqCst);
            let mut p = dl.progress.lock().await;
            p.status = DownloadStatus::Paused;
            p.speed_bytes_per_sec = 0;
            let _ = dl.app_handle.emit("download-progress", p.clone());
            info!("Paused download for {}", app_name);
        }
        Ok(())
    }

    pub async fn resume_download(&self, app_name: &str) -> Result<()> {
        let downloads = self.active_downloads.lock().await;
        if let Some(dl) = downloads.get(app_name) {
            dl.pause_flag.store(false, Ordering::SeqCst);
            let mut p = dl.progress.lock().await;
            p.status = DownloadStatus::Downloading;
            let _ = dl.app_handle.emit("download-progress", p.clone());
            info!("Resumed download for {}", app_name);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn start_download(
        self: Arc<Self>,
        app_handle: AppHandle,
        app_name: String,
        title: String,
        manifest: Manifest,
        // The raw manifest, kept beside the install once it completes
        manifest_bytes: Vec<u8>,
        base_url: String,
        // Epic OAuth token, attached only to Epic CDN requests
        auth_token: Option<String>,
        target_dir: PathBuf,
        max_workers: usize,
        installed_game: Option<InstalledGame>,
        storage: Arc<Storage>,
    ) -> Result<()> {
        // A second pipeline for the same app would write the same files in
        // parallel and leave the first one unreachable for cancel.
        if self.active_downloads.lock().await.contains_key(&app_name) {
            return Err(AppError::Download(format!(
                "{} is already downloading",
                title
            )));
        }

        let cancel_flag = Arc::new(AtomicBool::new(false));
        let pause_flag = Arc::new(AtomicBool::new(false));

        let total_bytes: u64 = manifest
            .file_manifest_list
            .iter()
            .map(|f| f.file_size)
            .sum();
        let total_chunks = manifest.chunk_data_list.len() as u64;

        let initial_progress = DownloadProgress {
            app_name: app_name.clone(),
            title: title.clone(),
            status: DownloadStatus::Analyzing,
            total_bytes,
            downloaded_bytes: 0,
            total_chunks,
            downloaded_chunks: 0,
            speed_bytes_per_sec: 0,
            eta_seconds: 0,
            progress_percent: 0.0,
            current_file: None,
            error_message: None,
        };

        let progress = Arc::new(Mutex::new(initial_progress));
        let active = Arc::new(ActiveDownload {
            app_name: app_name.clone(),
            title: title.clone(),
            install_dir: target_dir.clone(),
            cancel_flag: cancel_flag.clone(),
            pause_flag: pause_flag.clone(),
            progress: progress.clone(),
            app_handle: app_handle.clone(),
        });

        {
            let mut downloads = self.active_downloads.lock().await;
            downloads.insert(app_name.clone(), active);
        }

        let mgr = self.clone();
        let app_name_clone = app_name.clone();

        tokio::spawn(async move {
            let res = mgr
                .run_download_pipeline(
                    app_handle.clone(),
                    app_name_clone.clone(),
                    title.clone(),
                    manifest,
                    manifest_bytes,
                    base_url,
                    auth_token,
                    target_dir,
                    max_workers,
                    cancel_flag.clone(),
                    pause_flag,
                    progress.clone(),
                    installed_game,
                    storage,
                )
                .await;

            if let Err(e) = res {
                if !cancel_flag.load(Ordering::SeqCst) {
                    error!("Download pipeline failed for {}: {}", app_name_clone, e);
                    let mut p = progress.lock().await;
                    p.status = DownloadStatus::Failed;
                    p.error_message = Some(e.to_string());
                    let _ = app_handle.emit("download-progress", p.clone());
                }
            }

            let mut downloads = mgr.active_downloads.lock().await;
            downloads.remove(&app_name_clone);
        });

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_download_pipeline(
        &self,
        app_handle: AppHandle,
        app_name: String,
        _title: String,
        manifest: Manifest,
        manifest_bytes: Vec<u8>,
        base_url: String,
        auth_token: Option<String>,
        target_dir: PathBuf,
        max_workers: usize,
        cancel_flag: Arc<AtomicBool>,
        pause_flag: Arc<AtomicBool>,
        progress: Arc<Mutex<DownloadProgress>>,
        installed_game: Option<InstalledGame>,
        storage: Arc<Storage>,
    ) -> Result<()> {
        // 0. Every file must land inside the install folder. A crafted manifest
        //    is refused before anything is created or written.
        let mut file_paths = Vec::with_capacity(manifest.file_manifest_list.len());
        for fm in &manifest.file_manifest_list {
            file_paths.push(safe_join(&target_dir, &fm.filename)?);
        }
        if !manifest.meta.launch_exe.is_empty() {
            safe_join(&target_dir, &manifest.meta.launch_exe)?;
        }

        // 1. Create installation directory, remembering whether this run made it:
        //    only a folder we created may be removed again on cancel.
        let created_dir = !target_dir.exists();
        std::fs::create_dir_all(&target_dir)?;

        // 2. Decide which files are already on disk and correct. Existence
        //    alone proves nothing: updates and reinstalls used to keep every old
        //    or corrupt file. A file is skipped when the journal records the
        //    hash this manifest wants. In a folder Hyperion has never written
        //    (no journal: another tool's install, an older launcher) files are
        //    hashed instead. Where a journal exists it is complete — files are
        //    journaled as they finish — so an unlisted file is unfinished and
        //    re-hashing it (after a crash: GBs of pre-sized, half-empty files on
        //    a hard disk) would only delay the resume.
        let resume_path = target_dir.join(RESUME_JOURNAL);
        let trust_journal = resume_path.is_file();
        let journal = read_journal(&resume_path);
        let checks: Vec<(PathBuf, u64, [u8; 20], bool)> = manifest
            .file_manifest_list
            .iter()
            .zip(&file_paths)
            .map(|(fm, path)| {
                let vouched = journal.get(&fm.filename.to_ascii_lowercase())
                    == Some(&hex::encode(fm.hash));
                (path.clone(), fm.file_size, fm.hash, vouched)
            })
            .collect();
        let cancel_check = cancel_flag.clone();
        let already_done: Vec<bool> = tokio::task::spawn_blocking(move || {
            checks
                .into_iter()
                .map(|(path, size, hash, vouched)| {
                    if cancel_check.load(Ordering::SeqCst) {
                        return false;
                    }
                    let Ok(meta) = std::fs::metadata(&path) else {
                        return false;
                    };
                    if !meta.is_file() || meta.len() != size {
                        return false;
                    }
                    vouched
                        || (!trust_journal
                            && Installer::compute_file_sha1(&path).is_ok_and(|h| h == hash))
                })
                .collect()
        })
        .await
        .map_err(|e| AppError::Download(format!("Could not check existing files: {}", e)))?;

        if cancel_flag.load(Ordering::SeqCst) {
            return Err(AppError::Download("Download cancelled by user".to_string()));
        }

        // The journal is rewritten to vouch only for files just confirmed, so it
        // never claims a file this run is about to overwrite.
        let journal_file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&resume_path)?;
        let journal_file = Arc::new(std::sync::Mutex::new(journal_file));
        {
            let mut jf = journal_file.lock().unwrap();
            for (fm, done) in manifest.file_manifest_list.iter().zip(&already_done) {
                if *done {
                    writeln!(jf, "{}:{}", hex::encode(fm.hash), fm.filename)?;
                }
            }
        }
        let skipped = already_done.iter().filter(|d| **d).count();
        if skipped > 0 {
            info!(
                "{} of {} files are already correct on disk",
                skipped,
                manifest.file_manifest_list.len()
            );
        }

        // 3. Build chunk requirements for the files still needed
        #[derive(Clone)]
        struct ChunkRef {
            file_idx: usize,
            file_path: PathBuf,
            chunk_offset: u32,
            size: u32,
            file_offset: u64,
        }

        let mut chunk_refs: HashMap<[u32; 4], Vec<ChunkRef>> = HashMap::new();
        let mut files_to_create = Vec::new();
        // (journal line, parts still to write) for every file being written
        let mut journal_lines: Vec<String> = Vec::new();
        let mut parts_left: Vec<AtomicU64> = Vec::new();

        for ((fm, full_path), done) in manifest
            .file_manifest_list
            .iter()
            .zip(&file_paths)
            .zip(&already_done)
        {
            if *done {
                continue;
            }

            let file_idx = journal_lines.len();
            journal_lines.push(format!("{}:{}", hex::encode(fm.hash), fm.filename));
            parts_left.push(AtomicU64::new(fm.chunk_parts.len() as u64));
            files_to_create.push((full_path.clone(), fm.file_size, fm.chunk_parts.is_empty()));

            for part in &fm.chunk_parts {
                chunk_refs.entry(part.guid).or_default().push(ChunkRef {
                    file_idx,
                    file_path: full_path.clone(),
                    chunk_offset: part.offset,
                    size: part.size,
                    file_offset: part.file_offset,
                });
            }
        }
        let journal_lines = Arc::new(journal_lines);
        let parts_left = Arc::new(parts_left);

        // 4. Pre-create directories and target files with exact sizes. A failing
        //    `set_len` (a full disk) stops here instead of hours into the download.
        for (fp, size, _) in &files_to_create {
            if let Some(parent) = fp.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let f = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(fp)?;
            f.set_len(*size).map_err(|e| {
                AppError::Io(format!("Could not allocate {}: {}", fp.display(), e))
            })?;
        }
        // Empty files are complete as soon as they exist
        {
            let mut jf = journal_file.lock().unwrap();
            for (idx, (_, _, no_parts)) in files_to_create.iter().enumerate() {
                if *no_parts {
                    writeln!(jf, "{}", journal_lines[idx])?;
                }
            }
        }

        // 5. Build list of needed chunks. A file part pointing at a chunk the
        //    manifest does not list would silently stay zero-filled.
        let needed_chunks: Vec<ChunkInfo> = manifest
            .chunk_data_list
            .into_iter()
            .filter(|c| chunk_refs.contains_key(&c.guid))
            .collect();
        if needed_chunks.len() != chunk_refs.len() {
            return Err(AppError::Manifest(format!(
                "Manifest is inconsistent: {} file parts reference chunks it does not list",
                chunk_refs.len() - needed_chunks.len()
            )));
        }

        let total_needed_chunks = needed_chunks.len() as u64;
        let total_needed_bytes: u64 = needed_chunks.iter().map(|c| c.window_size as u64).sum();

        {
            let mut p = progress.lock().await;
            p.status = DownloadStatus::Downloading;
            p.total_chunks = total_needed_chunks;
            p.total_bytes = total_needed_bytes;
            let _ = app_handle.emit("download-progress", p.clone());
        }

        let workers = max_workers.clamp(1, 64);
        let semaphore = Arc::new(Semaphore::new(workers));
        // Set when any chunk fails, so already-spawned workers stop instead of
        // writing on in the background after the download has been reported.
        let halt = Arc::new(AtomicBool::new(false));
        // How long a chunk usually takes right now (moving average), which sets
        // when a slow one gets a second, racing request.
        let typical_ms = Arc::new(AtomicU64::new(INITIAL_TYPICAL_CHUNK_MS));
        let hedged_chunks = Arc::new(AtomicU64::new(0));
        let downloaded_bytes = Arc::new(AtomicU64::new(0));
        let downloaded_chunks = Arc::new(AtomicU64::new(0));

        let start_time = Instant::now();

        let chunk_refs = Arc::new(chunk_refs);
        let base_url = Arc::new(base_url);
        let manifest_version = manifest.version;

        // Channel for task completions
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Result<()>>((workers * 4).max(8));

        // Spawn workers
        for chunk_info in needed_chunks {
            if cancel_flag.load(Ordering::SeqCst) {
                return Err(AppError::Download("Download cancelled by user".to_string()));
            }

            while pause_flag.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(250)).await;
                if cancel_flag.load(Ordering::SeqCst) {
                    return Err(AppError::Download("Download cancelled by user".to_string()));
                }
            }

            let sem = semaphore.clone();
            let client = self.client.clone();
            let base_url = base_url.clone();
            let auth_token = auth_token.clone();
            let chunk_refs = chunk_refs.clone();
            let downloaded_bytes = downloaded_bytes.clone();
            let downloaded_chunks = downloaded_chunks.clone();
            let tx = tx.clone();
            let cancel_flag_task = cancel_flag.clone();
            let halt_task = halt.clone();
            let cancel = move || {
                cancel_flag_task.load(Ordering::SeqCst) || halt_task.load(Ordering::SeqCst)
            };
            let pause = pause_flag.clone();
            let journal_file = journal_file.clone();
            let journal_lines = journal_lines.clone();
            let parts_left = parts_left.clone();
            let typical_ms = typical_ms.clone();
            let hedged_chunks = hedged_chunks.clone();

            tokio::spawn(async move {
                // If paused, wait before acquiring permit
                while pause.load(Ordering::SeqCst) {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    if cancel() {
                        return;
                    }
                }

                let _permit = match sem.acquire().await {
                    Ok(p) => p,
                    Err(_) => return,
                };

                if cancel() {
                    return;
                }

                // If paused while waiting for permit, wait again
                while pause.load(Ordering::SeqCst) {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    if cancel() {
                        return;
                    }
                }

                let chunk_rel_path = chunk_info.path(manifest_version);
                let chunk_url = format!("{}/{}", base_url.trim_end_matches('/'), chunk_rel_path);

                // Fetch, decode and hash-check the chunk, retrying the whole
                // sequence: a truncated body, a bad inflate or a SHA-1 mismatch
                // is refetched just like a network error. Decoding runs on the
                // blocking pool so the async runtime keeps draining requests.
                let expected_sha = chunk_info.sha_hash;
                let mut chunk_data: Option<Vec<u8>> = None;
                let mut last_error = String::new();
                let mut missing_on_cdn = false;
                for attempt in 0..CHUNK_ATTEMPTS {
                    if cancel() {
                        return;
                    }
                    if attempt > 0 {
                        tokio::time::sleep(retry_delay(attempt)).await;
                        if cancel() {
                            return;
                        }
                    }

                    let started = Instant::now();
                    let hedge_after = hedge_delay(typical_ms.load(Ordering::Relaxed));
                    let (result, hedged) = fetch_chunk_hedged(
                        &client,
                        &chunk_url,
                        auth_token.as_deref(),
                        expected_sha,
                        hedge_after,
                    )
                    .await;
                    if hedged {
                        hedged_chunks.fetch_add(1, Ordering::Relaxed);
                    }

                    match result {
                        Ok(data) => {
                            record_chunk_time(&typical_ms, started.elapsed());
                            chunk_data = Some(data);
                            break;
                        }
                        Err(FetchFailure::Missing(e)) => {
                            missing_on_cdn = true;
                            warn!("Chunk download {} on attempt {}", e, attempt + 1);
                            last_error = e;
                        }
                        Err(FetchFailure::Other(e)) => {
                            missing_on_cdn = false;
                            warn!("Chunk {} failed on attempt {}: {}", chunk_url, attempt + 1, e);
                            last_error = e;
                        }
                    }
                }

                if cancel() {
                    return;
                }

                let guid = chunk_info.guid;
                let Some(chunk_data) = chunk_data else {
                    let message = if missing_on_cdn {
                        // A 404/403 here almost always means the base URL points at a
                        // host that only mirrors the manifest, not the chunk files.
                        format!(
                            "Chunk CDN '{}' does not serve this build (missing: {}). Sign in to Epic so the game data can be pulled from Epic's CDN, or set the chunk base URL in Options & Manifest.",
                            base_url, chunk_url
                        )
                    } else {
                        format!(
                            "Chunk {:08x}-{:08x}-{:08x}-{:08x} failed after {} attempts: {}",
                            guid[0], guid[1], guid[2], guid[3], CHUNK_ATTEMPTS, last_error
                        )
                    };
                    let _ = tx.send(Err(AppError::Download(message))).await;
                    return;
                };

                // Commit this chunk off the async runtime as well
                let refs: Vec<ChunkRef> = chunk_refs
                    .get(&chunk_info.guid)
                    .map(|refs| refs.to_vec())
                    .unwrap_or_default();

                let stored = tokio::task::spawn_blocking(move || -> std::result::Result<(), String> {
                    for r in refs {
                        let slice_start = r.chunk_offset as usize;
                        let slice_end = slice_start + r.size as usize;
                        if slice_end > chunk_data.len() {
                            return Err(format!(
                                "payload is {} bytes but a file slice needs {}",
                                chunk_data.len(),
                                slice_end
                            ));
                        }
                        write_at(&r.file_path, &chunk_data[slice_start..slice_end], r.file_offset)
                            .map_err(|e| format!("could not write {:?}: {}", r.file_path, e))?;

                        // Last part of this file written: journal it now, so a
                        // crash or kill resumes from here rather than from zero.
                        if parts_left[r.file_idx].fetch_sub(1, Ordering::SeqCst) == 1 {
                            if let Ok(mut jf) = journal_file.lock() {
                                let _ = writeln!(jf, "{}", journal_lines[r.file_idx]);
                            }
                        }
                    }

                    Ok(())
                })
                .await;

                match stored {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        let _ = tx
                            .send(Err(AppError::Download(format!(
                                "Chunk {:08x}-{:08x}-{:08x}-{:08x} failed: {}",
                                guid[0], guid[1], guid[2], guid[3], e
                            ))))
                            .await;
                        return;
                    }
                    Err(e) => {
                        let _ = tx
                            .send(Err(AppError::Download(format!(
                                "Chunk worker did not finish: {}",
                                e
                            ))))
                            .await;
                        return;
                    }
                }

                downloaded_bytes.fetch_add(chunk_info.window_size as u64, Ordering::Relaxed);
                downloaded_chunks.fetch_add(1, Ordering::Relaxed);

                let _ = tx.send(Ok(())).await;
            });
        }

        drop(tx); // drop original sender so rx closes when all tasks finish

        // Monitor progress and updates. Progress is published on a steady tick
        // as well as when chunks finish: during a stall nothing finishes, and
        // the UI used to freeze on the last (high) speed it was given.
        let mut chunks_completed = 0u64;
        let mut meter = SpeedMeter::new(Duration::from_secs(5));
        let mut ticker = tokio::time::interval(Duration::from_millis(500));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                res = rx.recv() => {
                    let Some(res) = res else { break };
                    if cancel_flag.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Err(e) = res {
                        halt.store(true, Ordering::SeqCst);
                        return Err(e);
                    }
                    chunks_completed += 1;
                    if chunks_completed < total_needed_chunks {
                        continue;
                    }
                }
                _ = ticker.tick() => {
                    if cancel_flag.load(Ordering::SeqCst) {
                        break;
                    }
                }
            }

            let now = Instant::now();
            let cur_bytes = downloaded_bytes.load(Ordering::Relaxed);
            let speed = if pause_flag.load(Ordering::SeqCst) {
                0
            } else {
                meter.record(now, cur_bytes)
            };
            let remaining_bytes = total_needed_bytes.saturating_sub(cur_bytes);
            let eta = remaining_bytes.checked_div(speed).unwrap_or(0);
            let percent = if total_needed_bytes > 0 {
                (cur_bytes as f32 / total_needed_bytes as f32) * 100.0
            } else {
                100.0
            };

            let mut p = progress.lock().await;
            p.downloaded_bytes = cur_bytes;
            p.downloaded_chunks = chunks_completed;
            p.speed_bytes_per_sec = speed;
            p.eta_seconds = eta;
            p.progress_percent = percent.min(100.0);
            let _ = app_handle.emit("download-progress", p.clone());
        }

        let hedged = hedged_chunks.load(Ordering::Relaxed);
        if hedged > 0 {
            info!(
                "{} of {} chunks were slow enough to race a second request",
                hedged, total_needed_chunks
            );
        }

        // Check if download was cancelled
        if cancel_flag.load(Ordering::SeqCst) {
            info!("Download cancelled for {}", app_name);
            let mut p = progress.lock().await;
            p.status = DownloadStatus::Cancelled;
            p.speed_bytes_per_sec = 0;
            p.eta_seconds = 0;
            let _ = app_handle.emit("download-progress", p.clone());
            drop(p);

            // A fresh install's folder is removed again — once in-flight workers
            // have let go of it. A folder that existed before (an update, a
            // resumed install, a folder the user picked) is left alone: its
            // journal stays accurate, so the next attempt resumes.
            if created_dir {
                let _ = tokio::time::timeout(
                    Duration::from_secs(30),
                    semaphore.acquire_many(workers as u32),
                )
                .await;
                drop(journal_file);
                if let Err(e) = std::fs::remove_dir_all(&target_dir) {
                    warn!(
                        "Could not remove the cancelled install at {}: {}",
                        target_dir.display(),
                        e
                    );
                }
            }
            return Err(AppError::Download("Download cancelled by user".to_string()));
        }

        // Crucial: verify that ALL chunks were downloaded
        if chunks_completed < total_needed_chunks {
            error!(
                "Download did not complete all chunks for {}: only {} of {} chunks finished",
                app_name, chunks_completed, total_needed_chunks
            );
            let mut p = progress.lock().await;
            p.status = DownloadStatus::Failed;
            p.speed_bytes_per_sec = 0;
            p.error_message = Some(format!(
                "Incomplete download: {} of {} chunks downloaded",
                chunks_completed, total_needed_chunks
            ));
            let _ = app_handle.emit("download-progress", p.clone());
            return Err(AppError::Download(
                "Not all chunks were downloaded successfully".to_string(),
            ));
        }

        // Final verification and journal update ONLY when 100% of chunks completed!
        info!("All chunks downloaded in {:?}", start_time.elapsed());
        {
            let mut p = progress.lock().await;
            p.status = DownloadStatus::Verifying;
            let _ = app_handle.emit("download-progress", p.clone());
        }

        // Files the previous build had and this one dropped are deleted, so an
        // update leaves exactly this build behind, not a mix of both.
        let previous = storage
            .get_installed_game(&app_name)
            .ok()
            .flatten()
            .filter(|g| same_dir(std::path::Path::new(&g.install_path), &target_dir))
            .and_then(|_| crate::manifest_cache::read_installed(&app_name))
            .and_then(|bytes| Manifest::parse(&bytes).ok());
        if let Some(previous) = previous {
            let removed = remove_stale_files(&target_dir, &previous, &manifest.file_manifest_list);
            if removed > 0 {
                info!("Removed {} files the new build no longer ships", removed);
            }
        }

        // Compact the journal to exactly this build
        drop(journal_file);
        if let Ok(mut rf) = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&resume_path)
        {
            for fm in &manifest.file_manifest_list {
                let _ = writeln!(rf, "{}:{}", hex::encode(fm.hash), fm.filename);
            }
        }

        // The exact manifest this install came from: verify, repair, updates and
        // uninstall work from it instead of whatever is published today.
        if let Err(e) = crate::manifest_cache::write_installed(&app_name, &manifest_bytes) {
            warn!("Could not keep the manifest for {}: {}", app_name, e);
        }

        // SAVE TO SQLITE DATABASE NOW THAT DOWNLOAD IS ACTUALLY COMPLETE!
        if let Some(ref inst) = installed_game {
            if let Err(e) = storage.save_installed_game(inst) {
                error!("Failed to save installed game record to SQLite: {}", e);
            } else {
                info!("Saved installed game record to SQLite for {}", app_name);
            }
        }

        {
            let mut p = progress.lock().await;
            p.status = DownloadStatus::Completed;
            p.progress_percent = 100.0;
            p.speed_bytes_per_sec = 0;
            p.eta_seconds = 0;
            let _ = app_handle.emit("download-progress", p.clone());
        }

        info!("Installation completed successfully for {}", app_name);
        Ok(())
    }
}

/// Tries per chunk (fetch + decode + hash check) before the download fails.
const CHUNK_ATTEMPTS: u32 = 6;

/// Starting guess for how long one chunk takes, before any have finished.
const INITIAL_TYPICAL_CHUNK_MS: u64 = 2_000;

/// How one fetch of a chunk failed.
enum FetchFailure {
    /// 404/403: this CDN does not have the chunk at all
    Missing(String),
    /// Anything worth retrying: network, stall, truncated body, bad data
    Other(String),
}

/// Downloads one chunk and decodes + hash-checks it on the blocking pool.
async fn fetch_chunk(
    client: &Client,
    url: &str,
    auth_token: Option<&str>,
    expected_sha: [u8; 20],
) -> std::result::Result<Vec<u8>, FetchFailure> {
    let mut request = client.get(url);
    if let Some(token) = auth_token {
        // Never leak an Epic token to a third-party mirror
        if crate::api::is_epic_host(url) {
            request = request.header("authorization", format!("bearer {}", token));
        }
    }

    let resp = request
        .send()
        .await
        .map_err(|e| FetchFailure::Other(format!("network error: {}", e)))?;
    let status = resp.status();
    if !status.is_success() {
        let message = format!("HTTP {}", status);
        return Err(
            if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::FORBIDDEN {
                FetchFailure::Missing(message)
            } else {
                FetchFailure::Other(message)
            },
        );
    }
    let raw = resp
        .bytes()
        .await
        .map_err(|e| FetchFailure::Other(format!("body read error: {}", e)))?;
    if raw.is_empty() {
        return Err(FetchFailure::Other("empty response body".to_string()));
    }

    tokio::task::spawn_blocking(move || decode_verified(&raw, &expected_sha))
        .await
        .map_err(|e| FetchFailure::Other(format!("decoder did not finish: {}", e)))?
        .map_err(FetchFailure::Other)
}

/// Fetches a chunk; if it runs past `hedge_after`, a second request on a fresh
/// connection races it and whichever succeeds first wins (the loser is
/// dropped, which aborts it). Returns whether the race was needed.
///
/// Epic's CDN serves most chunks of an old build in about a second but leaves
/// some hanging for 15 s to several minutes; measured on the Dauntless build,
/// racing those took 16 workers from 0.5–6 MiB/s to 6–9 MiB/s, worst chunk
/// from 505 s to 14 s.
async fn fetch_chunk_hedged(
    client: &Client,
    url: &str,
    auth_token: Option<&str>,
    expected_sha: [u8; 20],
    hedge_after: Duration,
) -> (std::result::Result<Vec<u8>, FetchFailure>, bool) {
    let first = fetch_chunk(client, url, auth_token, expected_sha);
    tokio::pin!(first);
    tokio::select! {
        result = &mut first => return (result, false),
        _ = tokio::time::sleep(hedge_after) => {}
    }

    let second = fetch_chunk(client, url, auth_token, expected_sha);
    tokio::pin!(second);
    let result = tokio::select! {
        result = &mut first => match result {
            Ok(data) => Ok(data),
            Err(_) => second.await,
        },
        result = &mut second => match result {
            Ok(data) => Ok(data),
            Err(_) => first.await,
        },
    };
    (result, true)
}

/// When to race a second request: well past what a chunk normally takes right
/// now. Relative, so a slow connection (where every chunk is slow) does not
/// get its traffic doubled; floored and capped so it stays useful either way.
fn hedge_delay(typical_ms: u64) -> Duration {
    Duration::from_millis((typical_ms.saturating_mul(3)).clamp(3_000, 60_000))
}

/// Folds one finished chunk's duration into the moving average.
fn record_chunk_time(typical_ms: &AtomicU64, took: Duration) {
    let sample = took.as_millis().min(u64::MAX as u128) as u64;
    let previous = typical_ms.load(Ordering::Relaxed);
    typical_ms.store((previous * 9 + sample) / 10, Ordering::Relaxed);
}

/// Download speed averaged over a sliding window, so the figure (and the ETA
/// built on it) does not jump with every burst of finished chunks.
struct SpeedMeter {
    window: Duration,
    samples: std::collections::VecDeque<(Instant, u64)>,
}

impl SpeedMeter {
    fn new(window: Duration) -> Self {
        Self {
            window,
            samples: std::collections::VecDeque::new(),
        }
    }

    /// Records the running byte total at `now` and returns bytes per second.
    fn record(&mut self, now: Instant, total_bytes: u64) -> u64 {
        self.samples.push_back((now, total_bytes));
        while self.samples.len() > 2
            && now.duration_since(self.samples[1].0) >= self.window
        {
            self.samples.pop_front();
        }
        let (first_at, first_bytes) = self.samples[0];
        let elapsed = now.duration_since(first_at).as_secs_f64();
        if elapsed <= 0.0 {
            return 0;
        }
        (total_bytes.saturating_sub(first_bytes) as f64 / elapsed) as u64
    }
}

/// Exponential backoff between chunk attempts: 0.5 s, 1 s, 2 s … capped at 8 s.
fn retry_delay(attempt: u32) -> Duration {
    Duration::from_millis((500u64 << attempt.saturating_sub(1).min(4)).min(8_000))
}

/// Inflates/decrypts a downloaded chunk and checks it against the manifest.
///
/// The manifest's chunk SHA-1 is the hash of the uncompressed window, so a
/// mislabelled, truncated or bit-flipped chunk is caught here before a single
/// byte reaches the game files. zlib's own checksum only covers compressed
/// chunks; stored (uncompressed) ones had no check at all. An all-zero hash
/// means the manifest predates chunk hashes and cannot be checked.
fn decode_verified(raw: &[u8], expected_sha: &[u8; 20]) -> std::result::Result<Vec<u8>, String> {
    let chunk = Chunk::parse(raw, None).map_err(|e| e.to_string())?;
    if *expected_sha != [0u8; 20] {
        let actual: [u8; 20] = Sha1::digest(&chunk.data).into();
        if actual != *expected_sha {
            return Err(format!(
                "SHA-1 mismatch: got {}, manifest expects {}",
                hex::encode(actual),
                hex::encode(expected_sha)
            ));
        }
    }
    Ok(chunk.data)
}

/// `filename -> sha1 hex` from a resume journal (lower-cased names; the last
/// line for a file wins, since completed files are appended as they finish).
fn read_journal(path: &std::path::Path) -> HashMap<String, String> {
    let mut entries = HashMap::new();
    if let Ok(content) = std::fs::read_to_string(path) {
        for line in content.lines() {
            if let Some((hash, name)) = line.split_once(':') {
                let name = name.trim();
                if !name.is_empty() {
                    entries.insert(name.to_ascii_lowercase(), hash.trim().to_ascii_lowercase());
                }
            }
        }
    }
    entries
}

/// Deletes files `previous` installed that `current` no longer has, plus any
/// folders that leaves empty. Returns how many files were removed.
fn remove_stale_files(
    install_dir: &std::path::Path,
    previous: &Manifest,
    current: &[FileManifest],
) -> usize {
    let key = |name: &str| name.replace('\\', "/").to_ascii_lowercase();
    let keep: HashSet<String> = current
        .iter()
        .map(|f| key(&f.filename))
        .collect();

    let mut removed = 0;
    for old in &previous.file_manifest_list {
        if keep.contains(&key(&old.filename)) {
            continue;
        }
        let Ok(path) = safe_join(install_dir, &old.filename) else {
            continue;
        };
        if std::fs::remove_file(&path).is_ok() {
            removed += 1;
            let mut dir = path.parent();
            while let Some(d) = dir {
                if same_dir(d, install_dir) || std::fs::remove_dir(d).is_err() {
                    break;
                }
                dir = d.parent();
            }
        }
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str) -> FileManifest {
        FileManifest {
            filename: name.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn chunks_must_match_the_manifest_sha1() {
        let payload = b"chunk payload bytes".repeat(64);
        let raw = Chunk::create_from_data([1, 2, 3, 4], &payload).unwrap();
        let good: [u8; 20] = Sha1::digest(&payload).into();

        assert_eq!(decode_verified(&raw, &good).unwrap(), payload);
        // Manifests without chunk hashes cannot be checked, but still decode
        assert_eq!(decode_verified(&raw, &[0u8; 20]).unwrap(), payload);

        let mut wrong = good;
        wrong[0] ^= 0xff;
        assert!(decode_verified(&raw, &wrong).unwrap_err().contains("SHA-1 mismatch"));
        assert!(decode_verified(&raw[..raw.len() - 5], &good).is_err());
    }

    /// Local chunk server: the connections listed in `stall` hang for 30 s
    /// before answering, every other one answers at once.
    async fn chunk_server(raw: Vec<u8>, stall: &'static [u64]) -> (String, Arc<AtomicU64>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/chunk", listener.local_addr().unwrap());
        let served = Arc::new(AtomicU64::new(0));
        let counter = served.clone();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let n = counter.fetch_add(1, Ordering::SeqCst);
                let raw = raw.clone();
                tokio::spawn(async move {
                    let mut request = [0u8; 2048];
                    let _ = socket.read(&mut request).await;
                    if stall.contains(&n) {
                        tokio::time::sleep(Duration::from_secs(30)).await;
                    }
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        raw.len()
                    );
                    let _ = socket.write_all(head.as_bytes()).await;
                    let _ = socket.write_all(&raw).await;
                });
            }
        });
        (url, served)
    }

    fn test_chunk() -> (Vec<u8>, Vec<u8>, [u8; 20]) {
        let payload = b"stalled chunk payload ".repeat(512);
        let raw = Chunk::create_from_data([9, 8, 7, 6], &payload).unwrap();
        let sha: [u8; 20] = Sha1::digest(&payload).into();
        (payload, raw, sha)
    }

    #[tokio::test]
    async fn a_stalled_chunk_is_raced_and_the_fast_copy_wins() {
        let (payload, raw, sha) = test_chunk();
        let (url, served) = chunk_server(raw, &[0]).await;
        let client = Client::new();

        let started = Instant::now();
        let (result, hedged) =
            fetch_chunk_hedged(&client, &url, None, sha, Duration::from_millis(200)).await;

        assert!(hedged);
        assert_eq!(result.ok().unwrap(), payload);
        assert!(started.elapsed() < Duration::from_secs(5), "waited on the stalled request");
        assert_eq!(served.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_prompt_chunk_is_fetched_once() {
        let (payload, raw, sha) = test_chunk();
        let (url, served) = chunk_server(raw, &[]).await;
        let client = Client::new();

        let (result, hedged) =
            fetch_chunk_hedged(&client, &url, None, sha, Duration::from_secs(5)).await;

        assert!(!hedged);
        assert_eq!(result.ok().unwrap(), payload);
        assert_eq!(served.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_chunk_fails_when_neither_copy_is_good() {
        let (_, raw, mut sha) = test_chunk();
        let (url, _) = chunk_server(raw, &[0]).await;
        // The stall timeout cuts the hung first copy short, as in the app
        let client = Client::builder()
            .read_timeout(Duration::from_secs(1))
            .build()
            .unwrap();

        // Both copies are the same bytes, so a wrong expected hash fails both
        sha[0] ^= 0xff;
        let started = Instant::now();
        let (result, hedged) =
            fetch_chunk_hedged(&client, &url, None, sha, Duration::from_millis(200)).await;
        assert!(hedged);
        assert!(result.is_err());
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn hedging_waits_well_past_the_typical_chunk() {
        assert_eq!(hedge_delay(500), Duration::from_secs(3)); // floor
        assert_eq!(hedge_delay(2_000), Duration::from_secs(6));
        assert_eq!(hedge_delay(40_000), Duration::from_secs(60)); // cap

        let typical = AtomicU64::new(2_000);
        record_chunk_time(&typical, Duration::from_millis(12_000));
        assert_eq!(typical.load(Ordering::Relaxed), 3_000);
    }

    #[test]
    fn speed_is_averaged_over_the_window() {
        let mut meter = SpeedMeter::new(Duration::from_secs(5));
        let t0 = Instant::now();
        assert_eq!(meter.record(t0, 0), 0);
        // A burst followed by silence: the average decays instead of flipping to 0
        assert_eq!(meter.record(t0 + Duration::from_secs(1), 10_000_000), 10_000_000);
        assert_eq!(meter.record(t0 + Duration::from_secs(2), 10_000_000), 5_000_000);
        // Old samples leave the window
        let speed = meter.record(t0 + Duration::from_secs(12), 10_000_000);
        assert_eq!(speed, 0);
    }

    #[test]
    fn retries_back_off_exponentially_with_a_cap() {
        assert_eq!(retry_delay(1), Duration::from_millis(500));
        assert_eq!(retry_delay(2), Duration::from_millis(1_000));
        assert_eq!(retry_delay(3), Duration::from_millis(2_000));
        assert_eq!(retry_delay(10), Duration::from_millis(8_000));
    }

    #[test]
    fn journal_keeps_the_latest_hash_per_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(RESUME_JOURNAL);
        std::fs::write(&path, "aa:Bin/Game.exe\nbb:Content/a.pak\ncc:bin/game.exe\nnot a line\n").unwrap();

        let journal = read_journal(&path);
        assert_eq!(journal.get("bin/game.exe").map(String::as_str), Some("cc"));
        assert_eq!(journal.get("content/a.pak").map(String::as_str), Some("bb"));
        assert_eq!(journal.len(), 2);
        assert!(read_journal(&dir.path().join("missing.txt")).is_empty());
    }

    #[test]
    fn update_removes_only_files_the_new_build_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("Old")).unwrap();
        std::fs::create_dir_all(root.join("Bin")).unwrap();
        std::fs::write(root.join("Old/gone.pak"), b"x").unwrap();
        std::fs::write(root.join("Bin/game.exe"), b"x").unwrap();
        std::fs::write(root.join("Bin/user.ini"), b"mine").unwrap();

        let mut previous = Manifest::default();
        previous.file_manifest_list = vec![file("Old/gone.pak"), file("Bin/game.exe"), file("../outside.txt")];
        let current = vec![file("bin\\GAME.exe")];

        assert_eq!(remove_stale_files(root, &previous, &current), 1);
        assert!(!root.join("Old").exists());
        assert!(root.join("Bin/game.exe").exists());
        assert!(root.join("Bin/user.ini").exists());
    }
}

/// Writes `data` at an absolute offset in an already-sized file.
///
/// Every worker opens its own handle, so each has a private cursor and parallel
/// chunk writers never fight over a shared file position. Failures are returned
/// rather than swallowed: a silently dropped slice used to be counted as a
/// completed chunk and left a corrupt install that still reported success.
fn write_at(path: &std::path::Path, data: &[u8], offset: u64) -> std::io::Result<()> {
    let mut file = OpenOptions::new().write(true).open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    file.write_all(data)
}
