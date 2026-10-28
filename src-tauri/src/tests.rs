#[cfg(test)]
mod tests {
    use crate::chunk::Chunk;
    use crate::cloud_save::CloudSaveManager;
    use crate::manifest::{read_fstring, write_fstring, ChunkInfo, Manifest, ManifestMeta};
    use crate::models::{AppSettings, InstalledGame, Session};
    use crate::storage::Storage;
    use std::collections::HashMap;
    use std::io::Cursor;

    #[test]
    fn test_fstring_ascii_and_utf16() {
        // Test ASCII fstring
        let mut buf = Vec::new();
        write_fstring(&mut buf, "Cyberpunk 2077").unwrap();
        let mut reader = Cursor::new(&buf);
        let decoded = read_fstring(&mut reader).unwrap();
        assert_eq!(decoded, "Cyberpunk 2077");

        // Test empty fstring
        let mut buf2 = Vec::new();
        write_fstring(&mut buf2, "").unwrap();
        let mut reader2 = Cursor::new(&buf2);
        let decoded2 = read_fstring(&mut reader2).unwrap();
        assert_eq!(decoded2, "");

        // Test Unicode / non-ASCII fstring
        let mut buf3 = Vec::new();
        write_fstring(&mut buf3, "ゲーム (Game)").unwrap();
        let mut reader3 = Cursor::new(&buf3);
        let decoded3 = read_fstring(&mut reader3).unwrap();
        assert_eq!(decoded3, "ゲーム (Game)");
    }

    #[test]
    fn test_manifest_serialization_and_parsing() {
        let manifest = Manifest {
            header_size: 41,
            size_uncompressed: 0,
            size_compressed: 0,
            sha_hash: [0u8; 20],
            stored_as: 0x01,
            version: 18,
            meta: ManifestMeta {
                meta_size: 0,
                data_version: 0,
                feature_level: 18,
                is_file_data: false,
                app_id: 12345,
                app_name: "TestGame".to_string(),
                build_version: "1.0.0".to_string(),
                launch_exe: "Binaries/Win64/TestGame.exe".to_string(),
                launch_command: "-windowed".to_string(),
                prereq_ids: vec!["VCRedist".to_string()],
                prereq_name: "Visual C++".to_string(),
                prereq_path: "prereq.exe".to_string(),
                prereq_args: "/quiet".to_string(),
                build_id: "".to_string(),
                uninstall_action_path: "".to_string(),
                uninstall_action_args: "".to_string(),
            },
            chunk_data_list: vec![ChunkInfo {
                guid: [0x11111111, 0x22222222, 0x33333333, 0x44444444],
                hash: 9876543210,
                sha_hash: [7u8; 20],
                group_num: 42,
                window_size: 1048576,
                file_size: 524288,
                secret_guid: None,
                window_size_compressed: None,
                encryption_tag: None,
            }],
            file_manifest_list: vec![],
            custom_fields: HashMap::from([(
                "CloudSaveFolder".to_string(),
                "{appdata}/Test".to_string(),
            )]),
        };

        let binary_bytes = manifest.write_to_vec().expect("Serialization failed");
        assert!(binary_bytes.len() > 41);

        // Parse serialized manifest
        let parsed = Manifest::parse(&binary_bytes).expect("Parsing failed");
        assert_eq!(parsed.meta.app_name, "TestGame");
        assert_eq!(parsed.meta.launch_exe, "Binaries/Win64/TestGame.exe");
        assert_eq!(parsed.chunk_data_list.len(), 1);
        assert_eq!(parsed.chunk_data_list[0].group_num, 42);
        assert_eq!(
            parsed.custom_fields.get("CloudSaveFolder").unwrap(),
            "{appdata}/Test"
        );
    }

    #[test]
    fn test_chunk_creation_and_parsing() {
        let guid = [0xAAAAAAAA, 0xBBBBBBBB, 0xCCCCCCCC, 0xDDDDDDDD];
        let original_data = b"Hello, this is a test chunk payload for Hyperion in Rust!";

        let chunk_bytes =
            Chunk::create_from_data(guid, original_data).expect("Chunk creation failed");
        assert!(chunk_bytes.len() > 66);

        let parsed = Chunk::parse(&chunk_bytes, None).expect("Chunk parsing failed");
        assert_eq!(parsed.guid, guid);
        assert_eq!(&parsed.data, original_data);
    }

    #[test]
    fn test_chunk_path_generation() {
        let chunk = ChunkInfo {
            guid: [0x12345678, 0x9ABCDEF0, 0x11223344, 0x55667788],
            hash: 0x0123456789ABCDEF,
            sha_hash: [0u8; 20],
            group_num: 7,
            window_size: 1048576,
            file_size: 100000,
            secret_guid: None,
            window_size_compressed: None,
            encryption_tag: None,
        };

        let path_v18 = chunk.path(18);
        assert!(path_v18.starts_with("ChunksV4/07/"));
        assert!(path_v18.ends_with(".chunk"));
    }

    #[test]
    fn epic_catalog_prune_keeps_steam_rows_and_current_titles() {
        let temp_dir = tempfile::tempdir().unwrap();
        let storage = Storage::new(temp_dir.path().join("prune.db")).unwrap();
        for (app, ns) in [
            ("Jackal", "jackal"),
            ("Gone", "oldns"),
            ("steam_331370", "steam"),
            ("ignored", "steam_ignored"),
            ("stepping", "ns"),
        ] {
            storage.cache_catalog_item(app, app, ns, "id", "{}").unwrap();
        }

        let removed = storage
            .prune_epic_catalog(&["Jackal".to_string(), "stepping".to_string()])
            .unwrap();
        assert_eq!(removed, 1);

        let mut left: Vec<String> = storage
            .get_cached_catalog_items()
            .unwrap()
            .into_iter()
            .map(|i| i.app_name)
            .collect();
        left.sort();
        assert_eq!(left, ["Jackal", "ignored", "steam_331370", "stepping"]);
    }

    #[test]
    fn old_default_worker_count_moves_to_32_once() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("workers.db");

        // A database written before the migration existed, on the old default
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO settings (key, value) VALUES ('max_workers', '16');",
            )
            .unwrap();
        }
        let storage = Storage::new(&db_path).unwrap();
        assert_eq!(storage.get_settings().unwrap().max_workers, 32);

        // Afterwards 16 is a deliberate choice and survives restarts
        let mut settings = storage.get_settings().unwrap();
        settings.max_workers = 16;
        storage.save_settings(&settings).unwrap();
        drop(storage);
        let storage = Storage::new(&db_path).unwrap();
        assert_eq!(storage.get_settings().unwrap().max_workers, 16);
    }

    #[test]
    fn test_storage_operations() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test_storage.db");
        let storage = Storage::new(&db_path).expect("Failed to initialize SQLite storage");

        // Test Settings
        let mut settings = AppSettings::default();
        assert!(
            !settings.steam_library_scan,
            "reading Steam's account files must be opt-in"
        );
        settings.max_workers = 12;
        settings.language = "de".to_string();
        settings.steam_library_scan = true;
        storage.save_settings(&settings).unwrap();

        let loaded_settings = storage.get_settings().unwrap();
        assert_eq!(loaded_settings.max_workers, 12);
        assert_eq!(loaded_settings.language, "de");
        assert!(loaded_settings.steam_library_scan);

        // Test Installed Game
        let game = InstalledGame {
            app_name: "TestGame".to_string(),
            title: "Test Game Title".to_string(),
            version: "1.0.0".to_string(),
            install_path: "C:\\Games\\TestGame".to_string(),
            executable: "test.exe".to_string(),
            launch_parameters: "-dx12".to_string(),
            install_size: 1024000,
            can_run_offline: true,
            requires_ot: false,
            platform: "Windows".to_string(),
            is_dlc: false,
            base_urls: vec!["https://cdn.example.com/build".to_string()],
            installed_at: Some("2026-10-20T00:00:00Z".to_string()),
        };

        storage.save_installed_game(&game).unwrap();
        let retrieved = storage
            .get_installed_game("TestGame")
            .unwrap()
            .expect("Game not found");
        assert_eq!(retrieved.title, "Test Game Title");
        assert_eq!(retrieved.executable, "test.exe");
        assert_eq!(retrieved.base_urls, vec!["https://cdn.example.com/build"]);

        // Test Session
        let session = Session {
            access_token: "test_access_token".to_string(),
            expires_in: 3600,
            expires_at: Some("2026-10-20T12:00:00Z".to_string()),
            token_type: "bearer".to_string(),
            refresh_token: Some("test_refresh_token".to_string()),
            refresh_expires: Some(7200),
            refresh_expires_at: None,
            account_id: "acc_12345".to_string(),
            client_id: Some("client_id".to_string()),
            internal_client: Some(false),
            client_service: None,
            display_name: Some("TestGamer".to_string()),
            app: None,
            in_sub: None,
        };

        storage.save_session(&session).unwrap();
        let loaded_session = storage
            .get_saved_session()
            .unwrap()
            .expect("Session not found");
        assert_eq!(loaded_session.account_id, "acc_12345");
        assert_eq!(loaded_session.display_name.unwrap(), "TestGamer");

        storage.clear_session().unwrap();
        assert!(storage.get_saved_session().unwrap().is_none());
    }

    fn sample_session() -> Session {
        Session {
            access_token: "sample_access_token".to_string(),
            expires_in: 3600,
            expires_at: Some("2026-10-20T12:00:00Z".to_string()),
            token_type: "bearer".to_string(),
            refresh_token: Some("sample_refresh_token".to_string()),
            refresh_expires: Some(7200),
            refresh_expires_at: None,
            account_id: "acc_12345".to_string(),
            client_id: None,
            internal_client: None,
            client_service: None,
            display_name: Some("TestGamer".to_string()),
            app: None,
            in_sub: None,
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn epic_session_is_sealed_on_disk() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("sealed.db");
        let storage = Storage::new(&db_path).unwrap();
        storage.save_session(&sample_session()).unwrap();

        // Neither column holds a readable token
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            let (refresh, json): (String, String) = conn
                .query_row(
                    "SELECT refresh_token, session_json FROM auth_state WHERE id = 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert!(refresh.is_empty());
            assert!(json.starts_with("dpapi:"));
            assert!(!json.contains("sample_access_token"));
            assert!(!json.contains("sample_refresh_token"));
        }

        let loaded = storage.get_saved_session().unwrap().expect("session restored");
        assert_eq!(loaded.access_token, "sample_access_token");
        assert_eq!(loaded.refresh_token.as_deref(), Some("sample_refresh_token"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn plaintext_epic_session_from_older_builds_is_sealed_and_scrubbed() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("legacy.db");
        drop(Storage::new(&db_path).unwrap());

        // Written the way builds before sealing did, renewed a few times. Real
        // tokens are long enough to spill into overflow pages, and a replaced
        // row's overflow pages go to the free list with the old token intact.
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.pragma_update(None, "secure_delete", false).unwrap();
            for n in 0..3 {
                let mut session = sample_session();
                session.access_token = if n == 0 {
                    "legacy_access_token_0 ".repeat(500)
                } else {
                    format!("legacy_access_token_{}", n)
                };
                session.refresh_token = Some(format!("legacy_refresh_token_{}", n));
                conn.execute(
                    "INSERT OR REPLACE INTO auth_state (id, account_id, display_name, refresh_token, session_json)
                     VALUES (1, ?1, 'TestGamer', ?2, ?3)",
                    rusqlite::params![
                        session.account_id,
                        session.refresh_token,
                        serde_json::to_string(&session).unwrap()
                    ],
                )
                .unwrap();
            }
        }
        let before = std::fs::read(&db_path).unwrap();
        assert!(
            String::from_utf8_lossy(&before).contains("legacy_access_token_0"),
            "the old file should still hold a replaced token"
        );

        // Still signed in after the update
        let storage = Storage::new(&db_path).unwrap();
        let loaded = storage.get_saved_session().unwrap().expect("legacy session kept");
        assert_eq!(loaded.refresh_token.as_deref(), Some("legacy_refresh_token_2"));
        drop(storage);

        // No copy of any token, current or replaced, is left in the file
        let bytes = std::fs::read(&db_path).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(!text.contains("legacy_access_token"));
        assert!(!text.contains("legacy_refresh_token"));

        // And the sealed copy opens on the next start
        let storage = Storage::new(&db_path).unwrap();
        let reopened = storage.get_saved_session().unwrap().expect("sealed session restored");
        assert_eq!(reopened.access_token, "legacy_access_token_2");
    }

    #[test]
    fn epic_session_sealed_for_another_user_is_cleared() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("foreign.db");
        let storage = Storage::new(&db_path).unwrap();

        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute(
            "INSERT INTO auth_state (id, account_id, display_name, refresh_token, session_json)
             VALUES (1, 'acc_12345', 'TestGamer', '', 'dpapi:bm90LWEtZHBhcGktYmxvYg==')",
            [],
        )
        .unwrap();

        // A sign-in prompt, not an error on every start
        assert!(storage.get_saved_session().unwrap().is_none());
        let rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM auth_state", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 0);
    }

    #[test]
    fn test_cloud_save_path_resolution() {
        let game = InstalledGame {
            app_name: "MyGame".to_string(),
            title: "My Game".to_string(),
            version: "1.0".to_string(),
            install_path: "D:/Games/MyGame".to_string(),
            executable: "game.exe".to_string(),
            launch_parameters: "".to_string(),
            install_size: 500,
            can_run_offline: true,
            requires_ot: false,
            platform: "Windows".to_string(),
            is_dlc: false,
            base_urls: vec![],
            installed_at: None,
        };

        let session = Session {
            access_token: "".to_string(),
            expires_in: 0,
            expires_at: None,
            token_type: "".to_string(),
            refresh_token: None,
            refresh_expires: None,
            refresh_expires_at: None,
            account_id: "epic_user_42".to_string(),
            client_id: None,
            internal_client: None,
            client_service: None,
            display_name: None,
            app: None,
            in_sub: None,
        };

        let resolved = CloudSaveManager::resolve_save_path(
            "{installdir}/Saves/{epicid}",
            &game,
            Some(&session),
        )
        .unwrap();

        let resolved_str = resolved.to_string_lossy().replace('\\', "/");
        assert_eq!(resolved_str, "D:/Games/MyGame/Saves/epic_user_42");
    }

    #[test]
    fn test_json_manifest_parsing() {
        let json_manifest = r#"{
            "ManifestFileVersion": "013000000000",
            "bIsFileData": false,
            "AppID": "000000000000",
            "AppNameString": "TestJsonGame",
            "BuildVersionString": "1.0.0",
            "LaunchExeString": "Binaries/Win64/TestJsonGame.exe",
            "LaunchCommand": "",
            "PrereqIds": [],
            "PrereqName": "",
            "PrereqPath": "",
            "PrereqArgs": "",
            "ChunkFilesizeList": {
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA": "100000000000"
            },
            "ChunkHashList": {
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA": "042000000000000000000000"
            },
            "ChunkShaList": {
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA": "0102030405060708090a0b0c0d0e0f1011121314"
            },
            "DataGroupList": {
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA": "005"
            },
            "FileManifestList": [
                {
                    "Filename": "Binaries/Win64/TestJsonGame.exe",
                    "FileHash": "001002003004005006007008009010011012013014015016017018019020",
                    "bIsReadOnly": false,
                    "bIsCompressed": false,
                    "bIsUnixExecutable": true,
                    "InstallTags": [],
                    "FileChunkParts": [
                        {
                            "Guid": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
                            "Offset": "000000000000",
                            "Size": "100000000000"
                        }
                    ]
                }
            ],
            "CustomFields": {
                "CloudSaveFolder": "{appdata}/TestJsonGame"
            }
        }"#;

        let parsed = Manifest::parse(json_manifest.as_bytes()).expect("JSON manifest parse failed");
        assert_eq!(parsed.meta.app_name, "TestJsonGame");
        assert_eq!(parsed.meta.build_version, "1.0.0");
        assert_eq!(parsed.meta.launch_exe, "Binaries/Win64/TestJsonGame.exe");
        assert_eq!(parsed.chunk_data_list.len(), 1);
        assert_eq!(parsed.chunk_data_list[0].group_num, 5);
        assert_eq!(parsed.chunk_data_list[0].hash, 42);
        assert_eq!(parsed.chunk_data_list[0].file_size, 100);
        assert_eq!(parsed.file_manifest_list.len(), 1);
        assert_eq!(parsed.file_manifest_list[0].filename, "Binaries/Win64/TestJsonGame.exe");
        assert_eq!(parsed.file_manifest_list[0].file_size, 100);
        assert!(parsed.file_manifest_list[0].is_executable());
        assert_eq!(
            parsed.custom_fields.get("CloudSaveFolder").unwrap(),
            "{appdata}/TestJsonGame"
        );
    }
}

