use crate::models::*;
use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionFileRow {
    pub path: String,
    pub operation: String,
    pub touch_count: u32,
    pub first_touched_sequence: u32,
}

pub struct DbQueries<'a> {
    conn: &'a Connection,
}

impl<'a> DbQueries<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM settings WHERE key = ?1")?;
        let result = stmt
            .query_row(params![key], |row| row.get::<_, String>(0))
            .ok();
        Ok(result)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_source_hash(&self, file_path: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT source_hash FROM sessions WHERE file_path = ?1")?;
        let result = stmt
            .query_row(params![file_path], |row| row.get::<_, String>(0))
            .ok();
        Ok(result)
    }

    pub fn upsert_session(&self, session: &Session) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sessions (id, parent_session_id, agent, title, project_path, created_at, updated_at, file_path, is_active, message_count, source_hash, model, git_branch, input_tokens, output_tokens, cached_tokens, reasoning_tokens, file_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)
             ON CONFLICT(id) DO UPDATE SET
                parent_session_id = excluded.parent_session_id,
                title = excluded.title,
                project_path = excluded.project_path,
                updated_at = excluded.updated_at,
                is_active = excluded.is_active,
                message_count = excluded.message_count,
                source_hash = excluded.source_hash,
                model = excluded.model,
                git_branch = excluded.git_branch,
                input_tokens = excluded.input_tokens,
                output_tokens = excluded.output_tokens,
                cached_tokens = excluded.cached_tokens,
                reasoning_tokens = excluded.reasoning_tokens,
                file_count = excluded.file_count",
            params![
                session.id,
                session.parent_session_id,
                session.agent.as_str(),
                session.title,
                session.project_path,
                session.created_at.to_rfc3339(),
                session.updated_at.to_rfc3339(),
                session.file_path,
                session.is_active as i32,
                session.message_count,
                String::new(),
                session.model,
                session.git_branch,
                session.input_tokens as i64,
                session.output_tokens as i64,
                session.cached_tokens as i64,
                session.reasoning_tokens as i64,
                session.file_count as i64,
            ],
        )?;
        Ok(())
    }

    pub fn set_source_hash(&self, session_id: &str, hash: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET source_hash = ?1 WHERE id = ?2",
            params![hash, session_id],
        )?;
        Ok(())
    }

    pub fn delete_sessions_by_file_path_except(
        &self,
        file_path: &str,
        keep_id: &str,
    ) -> Result<u64> {
        self.conn
            .execute(
                "DELETE FROM sessions WHERE file_path = ?1 AND id != ?2",
                params![file_path, keep_id],
            )
            .map(|n| n as u64)
    }

    pub fn delete_session_messages(&self, session_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM messages WHERE session_id = ?1",
            params![session_id],
        )?;
        Ok(())
    }

    pub fn insert_message(&self, msg: &Message) -> Result<()> {
        self.conn.execute(
            "INSERT INTO messages (id, session_id, role, content, timestamp, sequence, tool_name, tool_input, tool_output)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                msg.id,
                msg.session_id,
                msg.role.as_str(),
                msg.content,
                msg.timestamp.map(|t| t.to_rfc3339()),
                msg.sequence,
                msg.tool_name,
                msg.tool_input,
                msg.tool_output,
            ],
        )?;
        Ok(())
    }

    pub fn replace_session_files(
        &self,
        session_id: &str,
        touches: &[(String, String, u32)],
    ) -> Result<()> {
        self.conn.execute(
            "DELETE FROM session_files WHERE session_id = ?1",
            params![session_id],
        )?;

        for (path, operation, sequence) in touches {
            self.conn.execute(
                "INSERT INTO session_files (session_id, file_path, operation, touch_count, first_touched_sequence)
                 VALUES (?1, ?2, ?3, 1, ?4)
                 ON CONFLICT(session_id, file_path, operation) DO UPDATE SET
                    touch_count = touch_count + 1,
                    first_touched_sequence = MIN(first_touched_sequence, excluded.first_touched_sequence)",
                params![session_id, path, operation, sequence],
            )?;
        }
        Ok(())
    }

    pub fn get_session_files(&self, session_id: &str) -> Result<Vec<SessionFileRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT file_path, operation, touch_count, first_touched_sequence
             FROM session_files
             WHERE session_id = ?1
             ORDER BY first_touched_sequence ASC",
        )?;
        let rows = stmt
            .query_map(params![session_id], |row| {
                Ok(SessionFileRow {
                    path: row.get(0)?,
                    operation: row.get(1)?,
                    touch_count: row.get(2)?,
                    first_touched_sequence: row.get(3)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    pub fn get_sessions(
        &self,
        filters: &SessionFilters,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<Session>> {
        let mut sql = String::from(
            "SELECT id, parent_session_id, agent, title, project_path, created_at, updated_at, file_path, is_active, message_count, model, git_branch, input_tokens, output_tokens, cached_tokens, reasoning_tokens, file_count FROM sessions WHERE 1=1",
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut param_idx = 1;

        if let Some(ref agent) = filters.agent {
            sql.push_str(&format!(" AND agent = ?{}", param_idx));
            param_values.push(Box::new(agent.clone()));
            param_idx += 1;
        }

        if let Some(ref agents) = filters.agents {
            if agents.is_empty() {
                sql.push_str(" AND 1=0");
            } else {
                let mut placeholders = Vec::new();
                for a in agents {
                    placeholders.push(format!("?{}", param_idx));
                    param_values.push(Box::new(a.clone()));
                    param_idx += 1;
                }
                sql.push_str(&format!(" AND agent IN ({})", placeholders.join(",")));
            }
        }

        if let Some(ref title) = filters.title {
            if !title.is_empty() {
                let like_pattern = format!("%{}%", title.replace('%', "\\%").replace('_', "\\_"));
                sql.push_str(&format!(" AND title LIKE ?{} ESCAPE '\\'", param_idx));
                param_values.push(Box::new(like_pattern));
                param_idx += 1;
            }
        }

        if let Some(ref project) = filters.project_path {
            if !project.is_empty() {
                let like_pattern = format!("%{}%", project.replace('%', "\\%").replace('_', "\\_"));
                sql.push_str(&format!(
                    " AND project_path LIKE ?{} ESCAPE '\\'",
                    param_idx
                ));
                param_values.push(Box::new(like_pattern));
                param_idx += 1;
            }
        }

        if let Some(ref model) = filters.model {
            if !model.is_empty() {
                let like_pattern = format!("%{}%", model.replace('%', "\\%").replace('_', "\\_"));
                sql.push_str(&format!(" AND model LIKE ?{} ESCAPE '\\'", param_idx));
                param_values.push(Box::new(like_pattern));
                param_idx += 1;
            }
        }

        if let Some(active) = filters.is_active {
            sql.push_str(&format!(" AND is_active = ?{}", param_idx));
            param_values.push(Box::new(active as i32));
            param_idx += 1;
        }

        if let Some(ref branch) = filters.git_branch {
            sql.push_str(&format!(" AND git_branch = ?{}", param_idx));
            param_values.push(Box::new(branch.clone()));
            param_idx += 1;
        }

        if let Some(ref query) = filters.query {
            if !query.is_empty() {
                let like_pattern = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
                sql.push_str(&format!(
                    " AND (title LIKE ?{} ESCAPE '\\' OR id IN (SELECT session_id FROM messages WHERE content LIKE ?{} ESCAPE '\\' OR tool_input LIKE ?{} ESCAPE '\\' OR tool_output LIKE ?{} ESCAPE '\\'))",
                    param_idx, param_idx, param_idx, param_idx
                ));
                param_values.push(Box::new(like_pattern));
            }
        }

        sql.push_str(" ORDER BY updated_at DESC");
        sql.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));

        let mut stmt = self.conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let sessions = stmt
            .query_map(params_refs.as_slice(), |row| {
                Ok(Session {
                    id: row.get(0)?,
                    parent_session_id: row.get(1)?,
                    agent: AgentType::from_str(&row.get::<_, String>(2)?)
                        .unwrap_or(AgentType::Claude),
                    title: row.get(3)?,
                    project_path: row.get(4)?,
                    created_at: row
                        .get::<_, String>(5)
                        .ok()
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_default(),
                    updated_at: row
                        .get::<_, String>(6)
                        .ok()
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_default(),
                    file_path: row.get(7)?,
                    is_active: row.get::<_, i32>(8)? != 0,
                    message_count: row.get(9)?,
                    model: row.get(10)?,
                    git_branch: row.get(11)?,
                    input_tokens: row.get::<_, i64>(12)? as u64,
                    output_tokens: row.get::<_, i64>(13)? as u64,
                    cached_tokens: row.get::<_, i64>(14)? as u64,
                    reasoning_tokens: row.get::<_, i64>(15)? as u64,
                    file_count: row.get::<_, i64>(16)? as u32,
                })
            })?
            .filter_map(|s| s.ok())
            .collect();

        Ok(sessions)
    }

    pub fn get_messages(&self, session_id: &str, offset: u32, limit: u32) -> Result<Vec<Message>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, role, content, timestamp, sequence, tool_name, tool_input, tool_output
             FROM messages WHERE session_id = ?1 ORDER BY sequence ASC LIMIT ?2 OFFSET ?3",
        )?;

        let messages = stmt
            .query_map(params![session_id, limit, offset], |row| {
                let ts_str: Option<String> = row.get(4)?;
                let timestamp = ts_str
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc));
                Ok(Message {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    role: MessageRole::from_str(&row.get::<_, String>(2)?)
                        .unwrap_or(MessageRole::User),
                    content: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    timestamp,
                    sequence: row.get(5)?,
                    tool_name: row.get(6)?,
                    tool_input: row.get(7)?,
                    tool_output: row.get(8)?,
                })
            })?
            .filter_map(|m| m.ok())
            .collect();

        Ok(messages)
    }

    pub fn mark_stale_sessions(&self, active_paths: &[String]) -> Result<u64> {
        if active_paths.is_empty() {
            return self
                .conn
                .execute("DELETE FROM sessions WHERE 1=1", [])
                .map(|n| n as u64);
        }

        self.conn
            .execute_batch("CREATE TEMP TABLE IF NOT EXISTS active_paths (path TEXT PRIMARY KEY); DELETE FROM active_paths;")?;

        {
            let mut insert_stmt = self
                .conn
                .prepare("INSERT OR IGNORE INTO active_paths (path) VALUES (?1)")?;
            for path in active_paths {
                insert_stmt.execute(params![path])?;
            }
        }

        let deleted = self.conn.execute(
            "DELETE FROM sessions WHERE file_path NOT IN (SELECT path FROM active_paths)",
            [],
        )?;
        Ok(deleted as u64)
    }

    pub fn search_messages(&self, query: &str, limit: u32) -> Result<Vec<Message>> {
        let mut stmt = self.conn.prepare(
            "SELECT m.id, m.session_id, m.role, m.content, m.timestamp, m.sequence, m.tool_name, m.tool_input, m.tool_output
             FROM messages m
             INNER JOIN messages_fts fts ON m.rowid = fts.rowid
             WHERE messages_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;

        let messages = stmt
            .query_map(params![query, limit], |row| {
                let ts_str: Option<String> = row.get(4)?;
                let timestamp = ts_str
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc));
                Ok(Message {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    role: MessageRole::from_str(&row.get::<_, String>(2)?)
                        .unwrap_or(MessageRole::User),
                    content: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    timestamp,
                    sequence: row.get(5)?,
                    tool_name: row.get(6)?,
                    tool_input: row.get(7)?,
                    tool_output: row.get(8)?,
                })
            })?
            .filter_map(|m| m.ok())
            .collect();

        Ok(messages)
    }

    pub fn get_active_session_ids(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM sessions WHERE is_active = 1")?;
        let ids = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|id| id.ok())
            .collect();
        Ok(ids)
    }

    pub fn set_session_active(&self, session_id: &str, active: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE sessions SET is_active = ?1 WHERE id = ?2",
            params![active as i32, session_id],
        )?;
        Ok(())
    }

    pub fn rebuild_fts(&self) -> Result<()> {
        self.conn
            .execute_batch("INSERT INTO messages_fts(messages_fts) VALUES('rebuild');")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema;
    use chrono::Utc;
    use rusqlite::Connection;

    fn fresh() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        schema::init_schema(&conn).unwrap();
        conn
    }

    fn make_session(id: &str) -> Session {
        Session {
            id: id.to_string(),
            parent_session_id: None,
            agent: AgentType::Claude,
            title: "Test".to_string(),
            project_path: "/tmp/proj".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            file_path: format!("/tmp/{}.jsonl", id),
            is_active: false,
            message_count: 0,
            model: Some("claude-sonnet-4-5".to_string()),
            git_branch: Some("feat/auth".to_string()),
            input_tokens: 12345,
            output_tokens: 678,
            cached_tokens: 9000,
            reasoning_tokens: 50,
            file_count: 2,
        }
    }

    #[test]
    fn session_roundtrips_model_branch_and_tokens() {
        let conn = fresh();
        let q = DbQueries::new(&conn);
        let s = make_session("s1");
        q.upsert_session(&s).unwrap();

        let filters = SessionFilters {
            agent: None,
            agents: None,
            title: None,
            project_path: None,
            model: None,
            date_from: None,
            date_to: None,
            is_active: None,
            query: None,
            git_branch: None,
        };
        let rows = q.get_sessions(&filters, 0, 10).unwrap();
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!(r.model.as_deref(), Some("claude-sonnet-4-5"));
        assert_eq!(r.git_branch.as_deref(), Some("feat/auth"));
        assert_eq!(r.input_tokens, 12345);
        assert_eq!(r.output_tokens, 678);
        assert_eq!(r.cached_tokens, 9000);
        assert_eq!(r.reasoning_tokens, 50);
        assert_eq!(r.file_count, 2);
    }

    #[test]
    fn session_filters_by_git_branch() {
        let conn = fresh();
        let q = DbQueries::new(&conn);

        let mut s_a = make_session("a");
        s_a.git_branch = Some("feat/a".to_string());
        let mut s_b = make_session("b");
        s_b.git_branch = Some("feat/b".to_string());
        q.upsert_session(&s_a).unwrap();
        q.upsert_session(&s_b).unwrap();

        let filters = SessionFilters {
            agent: None,
            agents: None,
            title: None,
            project_path: None,
            model: None,
            date_from: None,
            date_to: None,
            is_active: None,
            query: None,
            git_branch: Some("feat/a".to_string()),
        };
        let rows = q.get_sessions(&filters, 0, 10).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "a");
    }

    #[test]
    fn session_file_touches_roundtrip() {
        let conn = fresh();
        let q = DbQueries::new(&conn);
        q.upsert_session(&make_session("s1")).unwrap();

        q.replace_session_files(
            "s1",
            &[
                ("/src/foo.rs".to_string(), "edit".to_string(), 0),
                ("/src/bar.ts".to_string(), "read".to_string(), 5),
                ("/src/foo.rs".to_string(), "edit".to_string(), 8),
            ],
        )
        .unwrap();

        let files = q.get_session_files("s1").unwrap();
        assert_eq!(files.len(), 2);

        let foo = files
            .iter()
            .find(|f| f.path == "/src/foo.rs")
            .expect("foo.rs should be present");
        assert_eq!(foo.operation, "edit");
        assert_eq!(foo.touch_count, 2);
        assert_eq!(foo.first_touched_sequence, 0);

        let bar = files
            .iter()
            .find(|f| f.path == "/src/bar.ts")
            .expect("bar.ts should be present");
        assert_eq!(bar.operation, "read");
        assert_eq!(bar.touch_count, 1);
        assert_eq!(bar.first_touched_sequence, 5);
    }
}
