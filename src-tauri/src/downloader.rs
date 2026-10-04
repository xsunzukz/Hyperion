use crate::chunk::Chunk;
use crate::error::{AppError, Result};
use crate::manifest::{ChunkInfo, Manifest};
use crate::models::{DownloadProgress, DownloadStatus, InstalledGame};
use crate::storage::Storage;
use reqwest::Client;
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
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
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
        base_url: String,
        // Epic OAuth token, attached only to Epic CDN requests
        auth_token: Option<String>,
        target_dir: PathBuf,
        max_workers: usize,
        installed_game: Option<InstalledGame>,
        storage: Arc<Storage>,
    ) -> Result<()> {
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
        // 1. Create installation directory
        std::fs::create_dir_all(&target_dir)?;

        // 2. Read resume file if exists
        let resume_path = target_dir.join(".hyperion_resume.txt");
        let completed_files: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

        if resume_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&resume_path) {
                let mut set = completed_files.lock().await;
                for line in content.lines() {
                    let parts: Vec<&str> = line.splitn(2, ':').collect();
                    if parts.len() == 2 {
                        let rel_path = parts[1].trim();
                        let full = target_dir.join(rel_path);
                        if full.exists() {
                            set.insert(rel_path.to_string());
                        }
                    }
                }
                info!(
                    "Found {} previously completed files from resume journal",
                    set.len()
                );
            }
        }

        // 3. Filter files and build chunk requirements
        #[derive(Clone)]
        struct ChunkRef {
            file_path: PathBuf,
            chunk_offset: u32,
            size: u32,
            file_offset: u64,
        }

        let mut chunk_refs: HashMap<[u32; 4], Vec<ChunkRef>> = HashMap::new();
        let mut files_to_create = Vec::new();

        {
            let done = completed_files.lock().await;
            for fm in &manifest.file_manifest_list {
                if done.contains(&fm.filename) {
                    continue;
                }

                let full_path = target_dir.join(&fm.filename);
                files_to_create.push((full_path.clone(), fm.file_size));

                for part in &fm.chunk_parts {
                    chunk_refs.entry(part.guid).or_default().push(ChunkRef {
                        file_path: full_path.clone(),
                        chunk_offset: part.offset,
                        size: part.size,
                        file_offset: part.file_offset,
                    });
                }
            }
        }

        // 4. Pre-create directories and target files with exact sizes
        for (fp, size) in &files_to_create {
            if let Some(parent) = fp.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let f = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(fp)?;
            let _ = f.set_len(*size);
        }

        // 5. Build list of needed chunks
        let needed_chunks: Vec<ChunkInfo> = manifest
            .chunk_data_list
            .into_iter()
            .filter(|c| chunk_refs.contains_key(&c.guid))
            .collect();

        let total_needed_chunks = needed_chunks.len() as u64;
        let total_needed_bytes: u64 = needed_chunks.iter().map(|c| c.window_size as u64).sum();

        {
            let mut p = progress.lock().await;
            p.status = DownloadStatus::Downloading;
            p.total_chunks = total_needed_chunks;
            p.total_bytes = total_needed_bytes;
            let _ = app_handle.emit("download-progress", p.clone());
        }

        let semaphore = Arc::new(Semaphore::new(max_workers.clamp(1, 32)));
        let downloaded_bytes = Arc::new(AtomicU64::new(0));
        let downloaded_chunks = Arc::new(AtomicU64::new(0));

        let start_time = Instant::now();
        let mut last_speed_time = Instant::now();
        let mut last_speed_bytes = 0u64;

        let chunk_refs = Arc::new(chunk_refs);
        let base_url = Arc::new(base_url);
        let manifest_version = manifest.version;

        // Channel for task completions
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Result<()>>(max_workers * 2);

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
            let cancel = cancel_flag.clone();
            let pause = pause_flag.clone();

            tokio::spawn(async move {
                // If paused, wait before acquiring permit
                while pause.load(Ordering::SeqCst) {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    if cancel.load(Ordering::SeqCst) {
                        return;
                    }
                }

                let _permit = match sem.acquire().await {
                    Ok(p) => p,
                    Err(_) => return,
                };

                if cancel.load(Ordering::SeqCst) {
                    return;
                }

                // If paused while waiting for permit, wait again
                while pause.load(Ordering::SeqCst) {
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    if cancel.load(Ordering::SeqCst) {
                        return;
                    }
                }

                let chunk_rel_path = chunk_info.path(manifest_version);
                let chunk_url = format!("{}/{}", base_url.trim_end_matches('/'), chunk_rel_path);

                // Fetch chunk with retry
                let mut chunk_data = None;
                for retry in 0..3 {
                    if cancel.load(Ordering::SeqCst) {
                        return;
                    }

                    let mut request = client.get(&chunk_url);
                    if let Some(token) = auth_token.as_deref() {
                        // Never leak an Epic token to a third-party mirror
                        if chunk_url.contains("epicgames") {
                            request = request.header("authorization", format!("bearer {}", token));
                        }
                    }

                    match request.send().await {
                        Ok(resp) if resp.status().is_success() => {
                            if let Ok(bytes) = resp.bytes().await {
                                match Chunk::parse(&bytes, None) {
                                    Ok(parsed) => {
                                        chunk_data = Some(parsed.data);
                                        break;
                                    }
                                    Err(e) => {
                                        warn!("Chunk parse error on attempt {}: {}", retry + 1, e);
                                    }
                                }
                            }
                        }
                        Ok(resp) => {
                            warn!(
                                "Chunk download HTTP {} on attempt {}",
                                resp.status(),
                                retry + 1
                            );
                        }
                        Err(e) => {
                            warn!("Chunk network error on attempt {}: {}", retry + 1, e);
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(200 * (retry + 1))).await;
                }

                if cancel.load(Ordering::SeqCst) {
                    return;
                }

                let uncompressed = match chunk_data {
                    Some(d) => d,
                    None => {
                        // A 404/403 here almost always means the base URL points at a
                        // host that only mirrors the manifest, not the chunk files.
                        let _ = tx
                            .send(Err(AppError::Download(format!(
                                "Chunk CDN '{}' does not serve this build (missing: {}). Sign in to Epic so the game data can be pulled from Epic's CDN, or set the chunk base URL in Options & Manifest.",
                                base_url, chunk_url
                            ))))
                            .await;
                        return;
                    }
                };

                // Write chunk slices into respective files
                if let Some(refs) = chunk_refs.get(&chunk_info.guid) {
                    for r in refs {
                        let slice_start = r.chunk_offset as usize;
                        let slice_end = slice_start + r.size as usize;
                        if slice_end <= uncompressed.len() {
                            let slice = &uncompressed[slice_start..slice_end];
                            if let Ok(mut f) = OpenOptions::new().write(true).open(&r.file_path) {
                                let _ = f.seek(SeekFrom::Start(r.file_offset));
                                let _ = f.write_all(slice);
                            }
                        }
                    }
                }

                downloaded_bytes.fetch_add(chunk_info.window_size as u64, Ordering::Relaxed);
                downloaded_chunks.fetch_add(1, Ordering::Relaxed);

                let _ = tx.send(Ok(())).await;
            });
        }

        drop(tx); // drop original sender so rx closes when all tasks finish

        // Monitor progress and updates
        let mut chunks_completed = 0u64;
        while let Some(res) = rx.recv().await {
            if cancel_flag.load(Ordering::SeqCst) {
                break;
            }

            res?;
            chunks_completed += 1;

            let now = Instant::now();
            let elapsed_sec = now.duration_since(last_speed_time).as_secs_f64();

            if elapsed_sec >= 0.4 || chunks_completed == total_needed_chunks {
                let cur_bytes = downloaded_bytes.load(Ordering::Relaxed);
                let delta_bytes = cur_bytes.saturating_sub(last_speed_bytes);
                let speed = if elapsed_sec > 0.0 {
                    (delta_bytes as f64 / elapsed_sec) as u64
                } else {
                    0
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

                last_speed_time = now;
                last_speed_bytes = cur_bytes;
            }
        }

        // Check if download was cancelled
        if cancel_flag.load(Ordering::SeqCst) {
            info!("Download cancelled for {}", app_name);
            let mut p = progress.lock().await;
            p.status = DownloadStatus::Cancelled;
            p.speed_bytes_per_sec = 0;
            p.eta_seconds = 0;
            let _ = app_handle.emit("download-progress", p.clone());
            let _ = std::fs::remove_file(&resume_path);
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

        // Save resume journal
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
