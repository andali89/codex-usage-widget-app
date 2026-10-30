fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_state",
            "diagnostics_status",
            "diagnostics_report",
            "open_diagnostics_folder",
            "refresh",
            "retry_autostart",
            "save_settings",
            "begin_display_preview",
            "update_display_preview",
            "end_display_preview",
            "inspect_storage_location",
            "choose_storage_folder",
            "view_ready",
            "hide_dashboard",
            "quit",
            "surface_action",
            "reminder_action",
        ]),
    ))
    .expect("failed to build native permissions");
}
