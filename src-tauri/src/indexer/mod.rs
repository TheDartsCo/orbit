use std::path::Path;
use std::sync::Arc;

use rusqlite::Connection;
use tokio::sync::Mutex;

use crate::adapters::{AdapterRegistry, SessionLocation};
use crate::db::queries::DbQueries;
use crate::models::*;

pub struct Indexer {
    db: Arc<Mutex<Connection>>,
    registry: Arc<AdapterRegistry>,
}

impl Indexer {
    pub fn new(db: Arc<Mutex<Connection>>, registry: Arc<AdapterRegistry>) -> Self {
        Self { db, registry }
    }

    pub async fn index_all(&self) -> Result<IndexStats, String> {
        let available = self.registry.detect_available().await;
        let mut stats = IndexStats::default();

        for adapter in &available {
            let locations = adapter.scan().await;
            stats.sessions_found += locations.len();

            let db = self.db.lock().await;
            let queries = DbQueries::new(&db);

            let mut indexed_paths = Vec::new();

            for loc in &locations {
                let hash = compute_hash(loc);
                let existing = queries.get_source_hash(&loc.path.to_string_lossy()).ok().flatten();

                if existing.as_ref() == Some(&hash) {
                    stats.sessions_skipped += 1;
                    indexed_paths.push(loc.path.to_string_lossy().to_string());
                    continue;
                }

                match adapter.parse_session(&loc.path).await {
                    Ok(normalized) => {
                        let session_id = normalized.session.id.clone();
                        if let Err(e) = queries.upsert_session(&normalized.session) {
                            tracing::warn!("Failed to upsert session {}: {}", session_id, e);
                            stats.sessions_errored += 1;
                            continue;
                        }
                        let _ = queries.delete_session_messages(&session_id);
                        for msg in &normalized.messages {
                            if let Err(e) = queries.insert_message(msg) {
                                tracing::warn!("Failed to insert message: {}", e);
                            }
                        }
                        let _ = queries.set_source_hash(&session_id, &hash);
                        stats.sessions_indexed += 1;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse session {:?}: {}", loc.path, e);
                        stats.sessions_errored += 1;
                    }
                }

                indexed_paths.push(loc.path.to_string_lossy().to_string());
            }

            if let Ok(removed) = queries.mark_stale_sessions(&indexed_paths) {
                stats.sessions_removed = removed;
            }

            if let Err(e) = queries.rebuild_fts() {
                tracing::warn!("Failed to rebuild FTS index: {}", e);
            }
        }

        Ok(stats)
    }

    pub async fn index_session(&self, path: &Path, adapter_id: &str) -> Result<(), String> {
        let adapter = self
            .registry
            .get(adapter_id)
            .ok_or_else(|| format!("Adapter {} not found", adapter_id))?;

        let normalized = adapter.parse_session(path).await?;
        let db = self.db.lock().await;
        let queries = DbQueries::new(&db);

        let session_id = normalized.session.id.clone();
        queries
            .upsert_session(&normalized.session)
            .map_err(|e| e.to_string())?;
        let _ = queries.delete_session_messages(&session_id);
        for msg in &normalized.messages {
            queries.insert_message(msg).map_err(|e| e.to_string())?;
        }

        let hash = compute_hash_from_path(path);
        let _ = queries.set_source_hash(&session_id, &hash);
        let _ = queries.rebuild_fts();

        Ok(())
    }

    pub async fn check_active_sessions(&self) -> Result<Vec<String>, String> {
        let available = self.registry.detect_available().await;
        let db = self.db.lock().await;
        let queries = DbQueries::new(&db);

        let current_active = queries.get_active_session_ids().map_err(|e| e.to_string())?;
        let mut new_active = Vec::new();

        for adapter in &available {
            let locations = adapter.scan().await;
            for loc in &locations {
                if adapter.is_active(&loc.path).await {
                    let file_stem = loc
                        .path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    new_active.push(file_stem);
                }
            }
        }

        for id in &current_active {
            if !new_active.contains(id) {
                let _ = queries.set_session_active(id, false);
            }
        }

        for id in &new_active {
            if !current_active.contains(id) {
                let _ = queries.set_session_active(id, true);
            }
        }

        Ok(new_active)
    }
}

fn compute_hash(loc: &SessionLocation) -> String {
    let metadata = std::fs::metadata(&loc.path);
    match metadata {
        Ok(m) => {
            let size = m.len();
            let modified = m
                .modified()
                .ok()
                .map(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                })
                .unwrap_or(0);
            format!("{}:{}", size, modified)
        }
        Err(_) => String::new(),
    }
}

fn compute_hash_from_path(path: &Path) -> String {
    let metadata = std::fs::metadata(path);
    match metadata {
        Ok(m) => {
            let size = m.len();
            let modified = m
                .modified()
                .ok()
                .map(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                })
                .unwrap_or(0);
            format!("{}:{}", size, modified)
        }
        Err(_) => String::new(),
    }
}

#[derive(Debug, Default, serde::Serialize)]
pub struct IndexStats {
    pub sessions_found: usize,
    pub sessions_indexed: usize,
    pub sessions_skipped: usize,
    pub sessions_errored: usize,
    pub sessions_removed: u64,
}
