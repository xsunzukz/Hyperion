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

