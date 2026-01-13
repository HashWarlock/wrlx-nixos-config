#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod grpc;

use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

struct AppState {
    grpc_addr: String,
}

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

#[tauri::command]
async fn send_message(
    message: String,
    conversation_id: Option<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<String, String> {
    let addr = {
        let state = state.lock().map_err(|e| e.to_string())?;
        state.grpc_addr.clone()
    };

    let mut client = grpc::AgentClient::connect(&addr)
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;

    client
        .send_message(&message, conversation_id)
        .await
        .map_err(|e| format!("Send failed: {}", e))
}

#[tauri::command]
async fn health_check(state: State<'_, Mutex<AppState>>) -> Result<HealthStatus, String> {
    let addr = {
        let state = state.lock().map_err(|e| e.to_string())?;
        state.grpc_addr.clone()
    };

    match grpc::AgentClient::connect(&addr).await {
        Ok(mut client) => match client.health_check().await {
            Ok((healthy, version)) => Ok(HealthStatus {
                connected: true,
                healthy,
                version: Some(version),
            }),
            Err(e) => Ok(HealthStatus {
                connected: true,
                healthy: false,
                version: Some(format!("Health check failed: {}", e)),
            }),
        },
        Err(_) => Ok(HealthStatus {
            connected: false,
            healthy: false,
            version: None,
        }),
    }
}

#[derive(serde::Serialize)]
struct HealthStatus {
    connected: bool,
    healthy: bool,
    version: Option<String>,
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, _event| {
                    let toggle_shortcut =
                        Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space);
                    if shortcut == &toggle_shortcut {
                        toggle_window(app);
                    }
                })
                .build(),
        )
        .manage(Mutex::new(AppState {
            grpc_addr: std::env::var("AGENT_GRPC_ADDR")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
        }))
        .setup(|app| {
            let shortcut =
                Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space);
            app.global_shortcut().register(shortcut)?;

            println!("CVM Agent started. Press Ctrl+Shift+Space to toggle.");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![send_message, health_check])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
