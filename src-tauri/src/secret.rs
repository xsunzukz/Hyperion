//! At-rest protection for stored credentials: the saved Epic session and the
//! remembered Steam password.
//!
//! Both are encrypted with Windows DPAPI (`CryptProtectData`) scoped to the
//! current Windows user, then base64-encoded into the local SQLite database
//! (the Epic session by `storage.rs`). A copy of `hyperion.db` on another
//! machine or Windows account cannot be decrypted, so staying signed in does
//! not turn into a plaintext credential leak.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use tracing::{debug, warn};

use crate::error::{AppError, Result};
use crate::models::SteamCredentials;
use crate::storage::Storage;

/// CRYPTPROTECT_UI_FORBIDDEN — never show a DPAPI prompt from a background app
const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;

#[cfg(target_os = "windows")]
mod sys {
    use std::ffi::c_void;

    #[repr(C)]
    pub struct DataBlob {
        pub cb_data: u32,
        pub pb_data: *mut u8,
    }

    #[link(name = "crypt32")]
    extern "system" {
        pub fn CryptProtectData(
            data_in: *const DataBlob,
            data_descr: *const u16,
            optional_entropy: *const DataBlob,
            reserved: *const c_void,
            prompt_struct: *const c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;

        pub fn CryptUnprotectData(
            data_in: *const DataBlob,
            data_descr: *mut *mut u16,
            optional_entropy: *const DataBlob,
            reserved: *const c_void,
            prompt_struct: *const c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        pub fn LocalFree(mem: *mut c_void) -> *mut c_void;
    }
}

#[cfg(target_os = "windows")]
fn dpapi_protect(data: &[u8]) -> Result<Vec<u8>> {
    use sys::*;

    let input = DataBlob {
        cb_data: data.len() as u32,
        pb_data: data.as_ptr() as *mut u8,
    };
    let mut output = DataBlob {
        cb_data: 0,
        pb_data: std::ptr::null_mut(),
    };

    let ok = unsafe {
        CryptProtectData(
            &input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };

    if ok == 0 {
        return Err(AppError::General(format!(
            "Windows DPAPI could not protect the credential: {}",
            std::io::Error::last_os_error()
        )));
    }

    let bytes = unsafe {
        std::slice::from_raw_parts(output.pb_data, output.cb_data as usize).to_vec()
    };
    unsafe {
        LocalFree(output.pb_data as *mut std::ffi::c_void);
    }

    Ok(bytes)
}

#[cfg(target_os = "windows")]
fn dpapi_unprotect(data: &[u8]) -> Result<Vec<u8>> {
    use sys::*;

    let input = DataBlob {
        cb_data: data.len() as u32,
        pb_data: data.as_ptr() as *mut u8,
    };
    let mut output = DataBlob {
        cb_data: 0,
        pb_data: std::ptr::null_mut(),
    };

    let ok = unsafe {
        CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };

    if ok == 0 {
        return Err(AppError::General(format!(
            "Windows DPAPI could not read the stored credential: {}",
            std::io::Error::last_os_error()
        )));
    }

    let bytes = unsafe {
        std::slice::from_raw_parts(output.pb_data, output.cb_data as usize).to_vec()
    };
    unsafe {
        LocalFree(output.pb_data as *mut std::ffi::c_void);
    }

    Ok(bytes)
}

/// Encrypts a secret and returns it base64-encoded, ready for the settings table.
pub fn encrypt(plaintext: &str) -> Result<String> {
    #[cfg(target_os = "windows")]
    {
        Ok(B64.encode(dpapi_protect(plaintext.as_bytes())?))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = plaintext;
        Err(AppError::General(
            "Remembering the Steam password is only supported on Windows".to_string(),
        ))
    }
}

/// Reverses [`encrypt`].
pub fn decrypt(encoded: &str) -> Result<String> {
    #[cfg(target_os = "windows")]
    {
        let raw = B64
            .decode(encoded.trim())
            .map_err(|e| AppError::General(format!("Stored credential is not valid base64: {}", e)))?;
        let plain = dpapi_unprotect(&raw)?;
        String::from_utf8(plain)
            .map_err(|e| AppError::General(format!("Stored credential is not valid UTF-8: {}", e)))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = encoded;
        Err(AppError::General(
            "Remembering the Steam password is only supported on Windows".to_string(),
        ))
    }
}

/// Saves the Steam password (encrypted) together with the account name.
pub fn remember(storage: &Storage, username: &str, password: &str) -> Result<()> {
    let mut settings = storage.get_settings()?;
    settings.steam_account = Some(username.to_string());
    settings.steam_password_enc = Some(encrypt(password)?);
    storage.save_settings(&settings)?;
    debug!("Stored DPAPI-protected Steam credential for '{}'", username);
    Ok(())
}

/// Removes any remembered credential from the settings table.
pub fn forget(storage: &Storage) -> Result<()> {
    let mut settings = storage.get_settings()?;
    if settings.steam_password_enc.is_none() {
        return Ok(());
    }
    settings.steam_password_enc = None;
    storage.save_settings(&settings)?;
    Ok(())
}

/// Loads the remembered credential, if present and decryptable.
///
/// A corrupt or foreign blob is cleared so the app falls back to asking for the
/// password instead of failing every launch.
pub fn load_credentials(storage: &Storage) -> Option<SteamCredentials> {
    let settings = storage.get_settings().ok()?;
    let encoded = settings.steam_password_enc?;
    let username = settings.steam_account?;

    match decrypt(&encoded) {
        Ok(password) => {
            debug!("Loaded remembered Steam credential for '{}'", username);
            Some(SteamCredentials {
                username,
                password,
                guard_code: None,
            })
        }
        Err(e) => {
            warn!("Stored Steam credential could not be decrypted ({}), clearing it", e);
            let _ = forget(storage);
            None
        }
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::{decrypt, encrypt};

    #[test]
    fn dpapi_round_trip() {
        let secret = "hunter2-★-round-trip";
        let encrypted = encrypt(secret).expect("encrypt");

        // The stored form must not contain the plaintext
        assert_ne!(encrypted, secret);
        assert!(!encrypted.contains("hunter2"));

        assert_eq!(decrypt(&encrypted).expect("decrypt"), secret);
    }

    #[test]
    fn corrupt_blob_is_rejected() {
        assert!(decrypt("not-a-dpapi-blob").is_err());
        assert!(decrypt("").is_err());
    }

    #[test]
    fn remembered_credential_survives_a_database_reopen() {
        use crate::storage::Storage;

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("hyperion-test.db");

        {
            let storage = Storage::new(db_path.clone()).expect("open db");
            super::remember(&storage, "sunzukz", "p@ssw0rd").expect("remember");

            // The raw settings row must not contain the plaintext
            let settings = storage.get_settings().unwrap();
            let stored = settings.steam_password_enc.expect("stored blob");
            assert!(!stored.contains("p@ssw0rd"));
        }

        let storage = Storage::new(db_path).expect("reopen db");
        let creds = super::load_credentials(&storage).expect("credential restored");
        assert_eq!(creds.username, "sunzukz");
        assert_eq!(creds.password, "p@ssw0rd");

        super::forget(&storage).expect("forget");
        assert!(super::load_credentials(&storage).is_none());
    }
}
