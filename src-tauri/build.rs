fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_state",
            "refresh",
            "retry_autostart",
            "save_settings",
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
