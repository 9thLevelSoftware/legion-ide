use legion_protocol::{
    CanonicalPath, LanguageToolchainSettingsRecord, PythonToolchainSettings,
    TypeScriptToolchainSettings, WorkspaceSessionRecord,
};

#[test]
fn language_toolchain_settings_round_trip_metadata_only() {
    let settings = LanguageToolchainSettingsRecord {
        schema_version: 1,
        typescript: Some(TypeScriptToolchainSettings {
            server_archive: CanonicalPath("C:/bundles/typescript-language-server.tgz".into()),
            compiler_archive: CanonicalPath("C:/bundles/typescript.tgz".into()),
            node_executable: CanonicalPath("C:/node/node.exe".into()),
        }),
        python: None,
    };
    let value = serde_json::to_value(&settings).expect("serialize settings");
    assert_eq!(value["schema_version"], 1);
    assert_eq!(
        value["typescript"]["server_archive"],
        "C:/bundles/typescript-language-server.tgz"
    );
    assert!(value.get("cache_path").is_none());
    assert!(value.get("hash").is_none());
    assert!(value.get("grant").is_none());
    assert_eq!(
        serde_json::from_value::<LanguageToolchainSettingsRecord>(value).expect("round trip"),
        settings
    );
}

#[test]
fn legacy_session_without_toolchain_settings_defaults_to_schema_one_none() {
    let legacy = serde_json::json!({
        "session_id": "legacy",
        "last_workspace": null,
        "last_workspace_path": null,
        "open_tabs": [],
        "active_tab": null,
        "active_buffer": null,
        "tab_groups": [],
        "layout_splits": [],
        "explorer_expansion": [],
        "panel_state": {
            "bottom_visible": false,
            "side_visible": true,
            "active_panel": null,
            "bottom_height_px": null,
            "side_width_px": null
        },
        "dirty_indicators": [],
        "saved_at": 0,
        "schema_version": 1
    });
    let session: WorkspaceSessionRecord = serde_json::from_value(legacy).expect("legacy session");
    assert_eq!(
        session.language_toolchain_settings,
        LanguageToolchainSettingsRecord::default()
    );
}

#[test]
fn python_toolchain_settings_round_trip_metadata_only() {
    let settings = LanguageToolchainSettingsRecord {
        schema_version: 1,
        typescript: None,
        python: Some(PythonToolchainSettings {
            interpreter_executable: CanonicalPath("C:/python/3.12/python.exe".into()),
            formatter_executable: CanonicalPath("C:/python/3.12/Scripts/black.exe".into()),
        }),
    };
    let value = serde_json::to_value(&settings).expect("serialize settings");
    assert_eq!(value["schema_version"], 1);
    assert_eq!(
        value["python"]["interpreter_executable"],
        "C:/python/3.12/python.exe"
    );
    assert_eq!(
        value["python"]["formatter_executable"],
        "C:/python/3.12/Scripts/black.exe"
    );

    // Metadata only: no cache location, no artifact hash, no grant, at either
    // level of the record.
    for absent in ["cache_path", "hash", "grant"] {
        assert!(
            value.get(absent).is_none(),
            "record must not carry {absent}"
        );
        assert!(
            value["python"].get(absent).is_none(),
            "python section must not carry {absent}"
        );
    }

    assert_eq!(
        serde_json::from_value::<LanguageToolchainSettingsRecord>(value).expect("round trip"),
        settings
    );

    // A record written before the Python section existed has no `python` key
    // at all and must still load, with no Python configuration.
    let legacy = serde_json::json!({
        "schema_version": 1,
        "typescript": null
    });
    let restored: LanguageToolchainSettingsRecord =
        serde_json::from_value(legacy).expect("legacy record without a python key");
    assert_eq!(restored.python, None);
    assert_eq!(restored, LanguageToolchainSettingsRecord::default());

    // An unconfigured record serializes without the key at all, so a session
    // written after this field existed stays byte-identical to one written
    // before it.
    let unconfigured = serde_json::to_value(LanguageToolchainSettingsRecord::default())
        .expect("serialize the default record");
    assert!(unconfigured.get("python").is_none());
}
