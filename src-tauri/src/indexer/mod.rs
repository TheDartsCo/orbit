use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use rusqlite::Connection;
use serde_json;
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
        tracing::info!("Detected {} available adapters", available.len());
        let mut stats = IndexStats::default();
        let mut all_indexed_paths = Vec::new();

        for adapter in &available {
            let adapter_id = adapter.id();
            let locations = adapter.scan().await;
            tracing::info!(
                "Adapter {} found {} session files",
                adapter_id,
                locations.len()
            );
            stats.sessions_found += locations.len();

            for loc in &locations {
                let hash = compute_hash(loc, adapter_id);
                let existing = {
                    let db = self.db.lock().await;
                    let queries = DbQueries::new(&db);
                    queries
                        .get_source_hash(&loc.path.to_string_lossy())
                        .ok()
                        .flatten()
                };

                if existing.as_ref() == Some(&hash) && adapter_id != "opencode" {
                    stats.sessions_skipped += 1;
                    all_indexed_paths.push(loc.path.to_string_lossy().to_string());
                    continue;
                }

                match adapter.parse_session(&loc.path).await {
                    Ok(mut normalized) => {
                        let session_id = normalized.session.id.clone();
                        let file_count = count_distinct_files(&normalized.file_touches);
                        normalized.session.file_count = file_count;
                        tracing::info!(
                            "Parsed session {} ({} messages, {} files): {}",
                            session_id,
                            normalized.messages.len(),
                            file_count,
                            normalized
                                .session
                                .title
                                .chars()
                                .take(50)
                                .collect::<String>()
                        );
                        let db = self.db.lock().await;
                        let queries = DbQueries::new(&db);
                        if let Err(e) = queries.upsert_session(&normalized.session) {
                            tracing::error!("Failed to upsert session {}: {}", session_id, e);
                            stats.sessions_errored += 1;
                            continue;
                        }
                        let _ = queries.delete_sessions_by_file_path_except(
                            &loc.path.to_string_lossy(),
                            &session_id,
                        );
                        let _ = queries.delete_session_messages(&session_id);
                        for msg in &normalized.messages {
                            if let Err(e) = queries.insert_message(msg) {
                                tracing::warn!("Failed to insert message: {}", e);
                            }
                        }
                        let touches: Vec<(String, String, u32)> = normalized
                            .file_touches
                            .iter()
                            .map(|t| (t.path.clone(), t.operation.clone(), t.sequence))
                            .collect();
                        if let Err(e) = queries.replace_session_files(&session_id, &touches) {
                            tracing::warn!(
                                "Failed to replace session files for {}: {}",
                                session_id,
                                e
                            );
                        }
                        let _ = queries.set_source_hash(&session_id, &hash);
                        stats.sessions_indexed += 1;

                        // Update provider stats
                        let provider_stat = stats
                            .provider_stats
                            .entry(adapter_id.to_string())
                            .or_default();
                        provider_stat.found += 1;
                        provider_stat.indexed += 1;
                    }
                    Err(e) => {
                        if e.contains("skipping") {
                            tracing::debug!("Skipping subagent session {:?}", loc.path.file_name());
                            stats.sessions_skipped += 1;

                            // Update provider stats for skipped
                            let provider_stat = stats
                                .provider_stats
                                .entry(adapter_id.to_string())
                                .or_default();
                            provider_stat.skipped += 1;
                        } else {
                            tracing::warn!("Failed to parse session {:?}: {}", loc.path, e);
                            stats.sessions_errored += 1;

                            // Update provider stats for errored
                            let provider_stat = stats
                                .provider_stats
                                .entry(adapter_id.to_string())
                                .or_default();
                            provider_stat.errored += 1;
                        }
                    }
                }

                all_indexed_paths.push(loc.path.to_string_lossy().to_string());
            }
        }

        tracing::info!("All adapters done: total_paths={}", all_indexed_paths.len());

        {
            let db = self.db.lock().await;
            let queries = DbQueries::new(&db);
            if let Ok(removed) = queries.mark_stale_sessions(&all_indexed_paths) {
                tracing::info!("Removed {} stale sessions", removed);
                stats.sessions_removed = removed;
            }
            if let Err(e) = queries.rebuild_fts() {
                tracing::warn!("Failed to rebuild FTS index: {}", e);
            }
        }

        // Set sync timestamp and persist stats to settings table
        stats.last_sync_at = Some(Utc::now());

        {
            let db = self.db.lock().await;
            let queries = DbQueries::new(&db);

            // Save last sync timestamp
            if let Some(timestamp) = stats.last_sync_at {
                let _ = queries.set_setting("last_sync_at", &timestamp.to_rfc3339());
            }

            // Save provider stats as JSON
            if let Ok(json) = serde_json::to_string(&stats.provider_stats) {
                let _ = queries.set_setting("last_sync_stats", &json);
            }
        }

        tracing::info!("Index complete: {:?}", stats);
        Ok(stats)
    }

    pub async fn index_session(&self, path: &Path, adapter_id: &str) -> Result<(), String> {
        let adapter = self
            .registry
            .get(adapter_id)
            .ok_or_else(|| format!("Adapter {} not found", adapter_id))?;

        let mut normalized = adapter.parse_session(path).await?;
        normalized.session.file_count = count_distinct_files(&normalized.file_touches);
        let db = self.db.lock().await;
        let queries = DbQueries::new(&db);

        let session_id = normalized.session.id.clone();
        queries
            .upsert_session(&normalized.session)
            .map_err(|e| e.to_string())?;
        let _ = queries.delete_sessions_by_file_path_except(&path.to_string_lossy(), &session_id);
        let _ = queries.delete_session_messages(&session_id);
        for msg in &normalized.messages {
            queries.insert_message(msg).map_err(|e| e.to_string())?;
        }
        let touches: Vec<(String, String, u32)> = normalized
            .file_touches
            .iter()
            .map(|t| (t.path.clone(), t.operation.clone(), t.sequence))
            .collect();
        queries
            .replace_session_files(&session_id, &touches)
            .map_err(|e| e.to_string())?;

        let hash = compute_hash_from_path(path, adapter_id);
        let _ = queries.set_source_hash(&session_id, &hash);
        let _ = queries.rebuild_fts();

        Ok(())
    }

    pub async fn check_active_sessions(&self) -> Result<Vec<String>, String> {
        let available = self.registry.detect_available().await;
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

        {
            let db = self.db.lock().await;
            let queries = DbQueries::new(&db);
            let current_active = queries
                .get_active_session_ids()
                .map_err(|e| e.to_string())?;

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
        }

        Ok(new_active)
    }
}

fn parser_version(adapter_id: &str) -> &'static str {
    match adapter_id {
        "codex" => "7",
        "claude" => "3",
        "cursor" => "6",
        "opencode" => "3",
        "qoder" => "3",
        "warp" => "3",
        "jetbrains" => "3",
        _ => "0",
    }
}

fn compute_hash(loc: &SessionLocation, adapter_id: &str) -> String {
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
            format!("v{}:{}:{}", parser_version(adapter_id), size, modified)
        }
        Err(_) => {
            let ts = loc.last_modified.timestamp();
            format!("v{}:db:{}", parser_version(adapter_id), ts)
        }
    }
}

fn compute_hash_from_path(path: &Path, adapter_id: &str) -> String {
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
            format!("v{}:{}:{}", parser_version(adapter_id), size, modified)
        }
        Err(_) => String::new(),
    }
}

fn count_distinct_files(touches: &[FileTouch]) -> u32 {
    let mut seen = std::collections::HashSet::new();
    for t in touches {
        seen.insert(&t.path);
    }
    seen.len() as u32
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize, Clone)]
pub struct ProviderSyncStats {
    pub found: usize,
    pub indexed: usize,
    pub skipped: usize,
    pub errored: usize,
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct IndexStats {
    pub sessions_found: usize,
    pub sessions_indexed: usize,
    pub sessions_skipped: usize,
    pub sessions_errored: usize,
    pub sessions_removed: u64,
    pub provider_stats: std::collections::HashMap<String, ProviderSyncStats>,
    pub last_sync_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::AdapterRegistry;
    use crate::db::{queries::DbQueries, schema};
    use chrono::Utc;
    use rusqlite::Connection;

    fn fresh_db() -> Arc<Mutex<Connection>> {
        let conn = Connection::open_in_memory().unwrap();
        schema::init_schema(&conn).unwrap();
        Arc::new(Mutex::new(conn))
    }

    async fn sessions_by_file_path(db: &Arc<Mutex<Connection>>, file_path: &str) -> Vec<Session> {
        let conn = db.lock().await;
        let mut stmt = conn
            .prepare("SELECT id, parent_session_id, agent, title, project_path, created_at, updated_at, file_path, is_active, message_count FROM sessions WHERE file_path = ?1")
            .unwrap();
        stmt.query_map([file_path], |row| {
            Ok(Session {
                id: row.get(0)?,
                parent_session_id: row.get(1)?,
                agent: AgentType::from_str(&row.get::<_, String>(2)?).unwrap_or(AgentType::Claude),
                title: row.get(3)?,
                project_path: row.get(4)?,
                created_at: row
                    .get::<_, String>(5)
                    .ok()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_default(),
                updated_at: row
                    .get::<_, String>(6)
                    .ok()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_default(),
                file_path: row.get(7)?,
                is_active: row.get::<_, i32>(8)? != 0,
                message_count: row.get(9)?,
                ..Default::default()
            })
        })
        .unwrap()
        .filter_map(|s| s.ok())
        .collect()
    }

    #[tokio::test]
    async fn reindex_replaces_stale_codex_row_with_uuid_id_and_parent() {
        let temp = tempfile::tempdir().unwrap();
        let session_id = "019da610-77cd-74f1-8d45-90d8610199b5";
        let parent_id = "019da5e7-d63a-7791-9a33-ddb11c729cdb";
        let rollout = temp
            .path()
            .join(format!("rollout-2026-04-19T18-06-30-{}.jsonl", session_id));
        let file_stem = rollout.file_stem().unwrap().to_string_lossy().to_string();
        let file_path_str = rollout.to_string_lossy().to_string();

        std::fs::write(
            &rollout,
            format!(
                concat!(
                    "{{\"timestamp\":\"2026-04-19T18:06:30.000Z\",\"type\":\"session_meta\",\"payload\":{{",
                    "\"id\":\"{sid}\",\"timestamp\":\"2026-04-19T18:06:30.000Z\",",
                    "\"cwd\":\"/tmp/parent-project\",\"originator\":\"Codex Desktop\",\"cli_version\":\"0.1.0\",",
                    "\"source\":{{\"subagent\":{{\"thread_spawn\":{{",
                    "\"parent_thread_id\":\"{pid}\",\"depth\":1,",
                    "\"agent_path\":null,\"agent_nickname\":\"Pauli\",\"agent_role\":\"default\"",
                    "}}}}}}",
                    "}}}}\n",
                    "{{\"timestamp\":\"2026-04-19T18:06:35.000Z\",\"type\":\"event_msg\",\"payload\":{{",
                    "\"type\":\"user_message\",\"message\":\"Investigate the failing test\"",
                    "}}}}\n"
                ),
                sid = session_id,
                pid = parent_id
            ),
        )
        .unwrap();

        let db = fresh_db();
        {
            let conn = db.lock().await;
            let queries = DbQueries::new(&conn);
            let stale = Session {
                id: file_stem.clone(),
                parent_session_id: None,
                agent: AgentType::Codex,
                title: "stale title".to_string(),
                project_path: String::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                file_path: file_path_str.clone(),
                is_active: false,
                message_count: 0,
                ..Default::default()
            };
            queries.upsert_session(&stale).unwrap();
            queries.set_source_hash(&file_stem, "v5:1:1").unwrap();
        }

        let registry = Arc::new(AdapterRegistry::new());
        let indexer = Indexer::new(db.clone(), registry);
        indexer.index_session(&rollout, "codex").await.unwrap();

        let sessions = sessions_by_file_path(&db, &file_path_str).await;
        assert_eq!(
            sessions.len(),
            1,
            "expected exactly one session row for the file, got {}",
            sessions.len()
        );
        let session = &sessions[0];
        assert_eq!(session.id, session_id);
        assert_eq!(session.parent_session_id.as_deref(), Some(parent_id));
        assert_eq!(session.agent, AgentType::Codex);
    }
}
