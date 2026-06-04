/// Helper to quote/escape strings for safe inclusion in shell commands.
/// Uses single-quote style on Unix and double-quote style on Windows (cmd/powershell).
pub fn shell_quote(s: &str) -> String {
    if cfg!(windows) {
        if s.is_empty() {
            "\"\"".to_string()
        } else {
            // Double up quotes for Windows quoting
            let escaped = s.replace('"', "\"\"");
            format!("\"{}\"", escaped)
        }
    } else {
        if s.is_empty() {
            "''".to_string()
        } else {
            // Replace single quote with the sequence: '\''  (close, escaped quote, reopen)
            let escaped = s.replace("'", "'\\''");
            format!("'{}'", escaped)
        }
    }
}
