#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::{Manager, AppHandle};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

fn toggle_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, _event| {
                    let toggle_shortcut = Shortcut::new(
                        Some(Modifiers::CONTROL | Modifiers::SHIFT),
                        Code::Space
                    );
                    if shortcut == &toggle_shortcut {
                        toggle_window(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            // Register the global shortcut
            let shortcut = Shortcut::new(
                Some(Modifiers::CONTROL | Modifiers::SHIFT),
                Code::Space
            );
            app.global_shortcut().register(shortcut)?;

            println!("CVM Agent started. Press Ctrl+Shift+Space to toggle.");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
