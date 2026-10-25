use crate::api::EpicApiClient;
use crate::error::{AppError, Result};
use crate::models::{InstalledGame, Session};
use std::path::Path;
use std::process::Command;
use tracing::{debug, info, warn};

pub struct Launcher;

impl Launcher {
    pub async fn launch_game(
        game: &InstalledGame,
        namespace: &str,
        session: Option<&Session>,
        api_client: &EpicApiClient,
        offline: bool,
        extra_args: &[String],
        language: Option<&str>,
    ) -> Result<u32> {
        let install_path = Path::new(&game.install_path);
        let exe_rel = game.executable.trim_start_matches(['/', '\\']);
        // The executable name comes from the manifest: never run anything
        // outside the install folder.
        let exe_path = crate::installer::safe_join(install_path, exe_rel)?;

        if !exe_path.exists() {
            return Err(AppError::Launch(format!(
                "Executable not found at: {}",
                exe_path.display()
            )));
        }

        let working_dir = exe_path.parent().unwrap_or(install_path);

        // Fetch online exchange token if not offline
        let exchange_token = if !offline && session.is_some() {
            match api_client.get_exchange_code_for_launcher().await {
                Ok(code) => code,
                Err(e) => {
                    warn!("Failed to fetch exchange code ({}). Launching offline.", e);
                    "unused".to_string()
                }
            }
        } else {
            "unused".to_string()
        };

        let (display_name, account_id) = if let Some(s) = session {
            (
                s.display_name
                    .clone()
                    .unwrap_or_else(|| "EpicUser".to_string()),
                s.account_id.clone(),
            )
        } else {
            (
                "OfflineUser".to_string(),
                "00000000000000000000000000000000".to_string(),
            )
        };

        let lang = language.unwrap_or("en");

        let mut args: Vec<String> = Vec::new();

        // 1. Built-in game launch parameters from manifest
        if !game.launch_parameters.is_empty() {
            for part in game.launch_parameters.split_whitespace() {
                args.push(part.to_string());
            }
        }

        // 2. Epic Portal parameters
        args.push("-AUTH_LOGIN=unused".to_string());
        args.push(format!("-AUTH_PASSWORD={}", exchange_token));
        args.push("-AUTH_TYPE=exchangecode".to_string());
        args.push(format!("-epicapp={}", game.app_name));
        args.push("-epicenv=Prod".to_string());
        args.push("-EpicPortal".to_string());
        args.push(format!("-epicusername={}", display_name));
        args.push(format!("-epicuserid={}", account_id));
        args.push(format!("-epiclocale={}", lang));
        args.push(format!("-epicsandboxid={}", namespace));

        // 3. User extra parameters
        for extra in extra_args {
            args.push(extra.clone());
        }

        info!("Spawning game process: {}", exe_path.display());
        info!("Working directory: {}", working_dir.display());
        // The exchange code is a live credential: never write it to a log
        let shown: Vec<String> = args
            .iter()
            .map(|arg| match arg.split_once('=') {
                Some((key, _)) if key.eq_ignore_ascii_case("-AUTH_PASSWORD") => format!("{}=<redacted>", key),
                _ => arg.clone(),
            })
            .collect();
        debug!("Launch arguments: {:?}", shown);

        let mut cmd = Command::new(&exe_path);
        cmd.args(&args).current_dir(working_dir);

        #[cfg(target_os = "windows")]
        {
            // Windows process flags (don't block parent console)
            use std::os::windows::process::CommandExt;
            const DETACHED_PROCESS: u32 = 0x00000008;
            cmd.creation_flags(DETACHED_PROCESS);
        }

        let child = cmd.spawn().map_err(|e| {
            AppError::Launch(format!("Failed to execute {}: {}", exe_path.display(), e))
        })?;

        let pid = child.id();
        info!("Game launched successfully with PID: {}", pid);
        Ok(pid)
    }
}
