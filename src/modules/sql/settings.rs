use rusqlite::{named_params, Connection, OptionalExtension};

pub fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        [key],
        |row| row.get(0),
    )
    .optional()
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (:key, :value)",
        named_params! { ":key": key, ":value": value },
    )?;
    Ok(())
}

pub struct IgnoreRule {
    pub name: String,
    pub condition: Option<String>,
}

impl IgnoreRule {
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if raw.is_empty() {
            return None;
        }
        if let Some((name, condition)) = raw.split_once(':') {
            let name = name.trim().to_string();
            let condition = condition.trim().to_string();
            if name.is_empty() || condition.is_empty() {
                return Some(Self { name, condition: None });
            }
            Some(Self { name, condition: Some(condition) })
        } else {
            Some(Self { name: raw.to_string(), condition: None })
        }
    }

    pub fn should_skip(&self, parent_path: &std::path::Path) -> bool {
        match &self.condition {
            None => true,
            Some(cond) => {
                let sibling = parent_path.join(cond);
                sibling.exists()
            }
        }
    }

    pub fn to_raw(&self) -> String {
        match &self.condition {
            Some(cond) => format!("{}:{}", self.name, cond),
            None => self.name.clone(),
        }
    }
}

pub fn get_ignore_rules(conn: &Connection) -> Vec<IgnoreRule> {
    get_setting(conn, "ignore_folders")
        .ok()
        .flatten()
        .map(|v| {
            v.split('\n')
                .filter_map(|s| IgnoreRule::parse(s))
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_ignore_rules(conn: &Connection, rules: &[IgnoreRule]) -> rusqlite::Result<()> {
    let value: String = rules
        .iter()
        .map(|r| r.to_raw())
        .collect::<Vec<_>>()
        .join("\n");
    set_setting(conn, "ignore_folders", &value)
}

pub fn get_ignore_list(conn: &Connection) -> Vec<String> {
    get_setting(conn, "ignore_folders")
        .ok()
        .flatten()
        .map(|v| {
            v.split('\n')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

pub fn set_ignore_list(conn: &Connection, folders: &[String]) -> rusqlite::Result<()> {
    let value = folders.join("\n");
    set_setting(conn, "ignore_folders", &value)
}

/// Persist a process's "stopped by user" state so it survives restarts.
/// The key is a stable process identifier (name), not a runtime id.
pub fn set_process_stopped(conn: &Connection, key: &str, stopped: bool) -> rusqlite::Result<()> {
    let value = if stopped { "1" } else { "0" };
    set_setting(conn, &format!("process_{}_stopped", key), value)
}

/// Whether a process with the given stable key was stopped by the user
/// (persisted from a previous run).
pub fn is_process_stopped(conn: &Connection, key: &str) -> bool {
    get_setting(conn, &format!("process_{}_stopped", key))
        .ok()
        .flatten()
        .map(|v| v == "1")
        .unwrap_or(false)
}
