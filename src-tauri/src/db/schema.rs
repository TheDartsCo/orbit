use rusqlite::Connection;

pub fn init_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            parent_session_id TEXT,
            agent TEXT NOT NULL,
            title TEXT,
            project_path TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            file_path TEXT NOT NULL,
            is_active INTEGER NOT NULL DEFAULT 0,
            message_count INTEGER NOT NULL DEFAULT 0,
            source_hash TEXT
        );

        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            role TEXT NOT NULL,
            content TEXT,
            timestamp TEXT,
            sequence INTEGER NOT NULL,
            tool_name TEXT,
            tool_input TEXT,
            tool_output TEXT
        );

        CREATE TABLE IF NOT EXISTS attachments (
            id TEXT PRIMARY KEY,
            message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
            attachment_type TEXT NOT NULL,
            path TEXT NOT NULL,
            mime_type TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_sessions_agent ON sessions(agent);
        CREATE INDEX IF NOT EXISTS idx_sessions_project ON sessions(project_path);
        CREATE INDEX IF NOT EXISTS idx_sessions_updated ON sessions(updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id, sequence);
        ",
    )?;

    let has_parent_session_id = conn
        .prepare("PRAGMA table_info(sessions)")?
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|column| column.ok())
        .any(|column| column == "parent_session_id");

    if !has_parent_session_id {
        conn.execute_batch("ALTER TABLE sessions ADD COLUMN parent_session_id TEXT;")?;
    }

    add_column_if_missing(
        conn,
        "sessions",
        "model",
        "ALTER TABLE sessions ADD COLUMN model TEXT;",
    )?;
    add_column_if_missing(
        conn,
        "sessions",
        "git_branch",
        "ALTER TABLE sessions ADD COLUMN git_branch TEXT;",
    )?;
    add_column_if_missing(
        conn,
        "sessions",
        "input_tokens",
        "ALTER TABLE sessions ADD COLUMN input_tokens INTEGER NOT NULL DEFAULT 0;",
    )?;
    add_column_if_missing(
        conn,
        "sessions",
        "output_tokens",
        "ALTER TABLE sessions ADD COLUMN output_tokens INTEGER NOT NULL DEFAULT 0;",
    )?;
    add_column_if_missing(
        conn,
        "sessions",
        "cached_tokens",
        "ALTER TABLE sessions ADD COLUMN cached_tokens INTEGER NOT NULL DEFAULT 0;",
    )?;
    add_column_if_missing(
        conn,
        "sessions",
        "reasoning_tokens",
        "ALTER TABLE sessions ADD COLUMN reasoning_tokens INTEGER NOT NULL DEFAULT 0;",
    )?;
    add_column_if_missing(
        conn,
        "sessions",
        "file_count",
        "ALTER TABLE sessions ADD COLUMN file_count INTEGER NOT NULL DEFAULT 0;",
    )?;

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS session_files (
            session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            file_path TEXT NOT NULL,
            operation TEXT NOT NULL,
            touch_count INTEGER NOT NULL DEFAULT 1,
            first_touched_sequence INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (session_id, file_path, operation)
        );

        CREATE INDEX IF NOT EXISTS idx_session_files_path ON session_files(file_path);
        CREATE INDEX IF NOT EXISTS idx_session_files_session ON session_files(session_id);

        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT DEFAULT CURRENT_TIMESTAMP
        );
        ",
    )?;

    conn.execute_batch(
        "
        CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
            content,
            tool_input,
            tool_output,
            content=messages,
            content_rowid=rowid
        );
        ",
    )?;

    Ok(())
}

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    ddl: &str,
) -> Result<(), rusqlite::Error> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let columns: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|c| c.ok())
        .collect();

    if !columns.iter().any(|c| c == column) {
        conn.execute_batch(ddl)?;
    }
    Ok(())
}
