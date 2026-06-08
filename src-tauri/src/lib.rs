pub mod adapters;
pub mod commands;
pub mod db;
pub mod indexer;
pub mod models;
pub mod shell_quote;
pub mod watcher;

use std::sync::Arc;
use tokio::sync::Mutex;

pub fn open_db() -> Result<rusqlite::Connection, Box<dyn std::error::Error>> {
    let app_dir = dirs::data_dir()
        .ok_or("Cannot determine app data directory")?
        .join("orbit");
    std::fs::create_dir_all(&app_dir)?;
    let db_path = app_dir.join("orbit.db");
    let conn = rusqlite::Connection::open(&db_path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    db::schema::init_schema(&conn)?;
    Ok(conn)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let conn = open_db().expect("Failed to open database");
    let db = Arc::new(Mutex::new(conn));

    let registry = Arc::new(adapters::AdapterRegistry::new());
    let indexer = Arc::new(indexer::Indexer::new(db.clone(), registry.clone()));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(commands::AppState {
            db,
            registry,
            indexer,
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_sessions,
            commands::get_session_messages,
            commands::search_sessions,
            commands::get_platform,
            commands::get_resume_command,
            commands::launch_resume,
            commands::get_active_sessions,
            commands::reindex_all,
            commands::get_sync_status,
            commands::detect_terminals,
            commands::get_app_settings,
            commands::save_app_setting,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
