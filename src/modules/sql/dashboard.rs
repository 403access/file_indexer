use rusqlite::{named_params, Connection};

use crate::modules::sql::settings::get_ignore_rules;
use crate::modules::logging;

/// Refresh the materialized dashboard stats and timeline tables.
pub fn refresh_dashboard_stats(conn: &Connection) {
    logging::info("Refreshing dashboard stats snapshot...");
    let now = chrono::Utc::now().timestamp() as f64;

    let (total_files, total_folders, total_size): (u64, u64, u64) = conn
        .query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN is_file = 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN is_directory = 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN is_file = 1 THEN size ELSE 0 END), 0)
             FROM files",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((0, 0, 0));

    let skipped_paths: u64 = conn
        .query_row("SELECT COUNT(*) FROM skipped_paths", [], |r| r.get(0))
        .unwrap_or(0);

    let ignore_rules_count = get_ignore_rules(conn).len() as u64;

    let duplicate_file_groups: u64 = conn
        .query_row("SELECT COUNT(*) FROM duplicate_hashes", [], |r| r.get(0))
        .unwrap_or(0);

    let (duplicate_files, wasted_file_bytes): (u64, u64) = if duplicate_file_groups > 0 {
        conn.query_row(
            "SELECT COALESCE(SUM(cnt), 0), COALESCE(SUM((cnt - 1) * size), 0)
             FROM (
                SELECT COUNT(*) as cnt, MIN(f.size) as size
                FROM files f
                JOIN duplicate_hashes d ON f.hash = d.hash
                WHERE f.is_file = 1
                GROUP BY f.hash
             )",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((0, 0))
    } else {
        (0, 0)
    };

    let duplicate_folders: u64 = if duplicate_file_groups > 0 {
        conn.query_row(
            "SELECT COALESCE(SUM(cnt), 0)
             FROM (
                SELECT COUNT(*) as cnt
                FROM files f
                JOIN duplicate_hashes d ON f.hash = d.hash
                WHERE f.is_directory = 1
                GROUP BY f.hash
             )",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    } else {
        0
    };

    let duplicate_folder_groups: u64 = if duplicate_folders > 0 {
        conn.query_row(
            "SELECT COUNT(*)
             FROM (
                SELECT f.hash
                FROM files f
                JOIN duplicate_hashes d ON f.hash = d.hash
                WHERE f.is_directory = 1
                GROUP BY f.hash
                HAVING COUNT(*) > 1
             )",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    } else {
        0
    };

    let total_entries = total_files + total_folders;
    let last_entry_id: i64 = conn
        .query_row("SELECT COALESCE(MAX(id), 0) FROM files", [], |r| r.get(0))
        .unwrap_or(0);
    let stats = [
        ("total_files", total_files.to_string()),
        ("total_folders", total_folders.to_string()),
        ("total_size", total_size.to_string()),
        ("skipped_paths", skipped_paths.to_string()),
        ("ignore_rules_count", ignore_rules_count.to_string()),
        ("duplicate_file_groups", duplicate_file_groups.to_string()),
        ("duplicate_files", duplicate_files.to_string()),
        ("wasted_file_bytes", wasted_file_bytes.to_string()),
        ("duplicate_folder_groups", duplicate_folder_groups.to_string()),
        ("duplicate_folders", duplicate_folders.to_string()),
        ("entries_at_refresh", total_entries.to_string()),
        ("last_entry_id", last_entry_id.to_string()),
    ];

    for (key, value) in &stats {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO dashboard_stats (key, value, updated_at) VALUES (:key, :value, :updated_at)",
            named_params! { ":key": key, ":value": value, ":updated_at": now },
        );
    }

    let _ = conn.execute("DELETE FROM dashboard_timeline", []);

    let intervals = ["day", "week", "month", "year"];
    for interval in &intervals {
        let group_sql = match *interval {
            "day" => "strftime('%Y-%m-%d', modified, 'unixepoch')",
            "week" => "strftime('%Y-W%W', modified, 'unixepoch')",
            "year" => "strftime('%Y', modified, 'unixepoch')",
            _ => "strftime('%Y-%m', modified, 'unixepoch')",
        };

        let sql = format!(
            "INSERT INTO dashboard_timeline (interval_type, label, files, folders, size)
             SELECT ?1 as interval_type,
                    {group_sql} as label,
                    SUM(CASE WHEN is_file = 1 THEN 1 ELSE 0 END) as files,
                    SUM(CASE WHEN is_directory = 1 THEN 1 ELSE 0 END) as folders,
                    COALESCE(SUM(CASE WHEN is_file = 1 THEN size ELSE 0 END), 0) as size
             FROM files
             WHERE modified IS NOT NULL
             GROUP BY label
             ORDER BY label ASC"
        );

        let _ = conn.execute(&sql, [interval]);
    }

    let _ = conn.execute(
        "INSERT OR REPLACE INTO dashboard_stats (key, value, updated_at) VALUES ('last_refreshed', :value, :updated_at)",
        named_params! { ":value": now.to_string(), ":updated_at": now },
    );
    logging::info(&format!("Dashboard snapshot refreshed: {} files, {} folders", total_files, total_folders));
}

/// Recompute dashboard stats WITHOUT updating the snapshot timestamp or entries_at_refresh.
/// Used by the periodic timer so the "behind" count keeps growing between manual refreshes.
pub fn recompute_dashboard_stats(conn: &Connection) {
    logging::info("Recomputing dashboard stats...");
    let now = chrono::Utc::now().timestamp() as f64;

    let (total_files, total_folders, total_size): (u64, u64, u64) = conn
        .query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN is_file = 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN is_directory = 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN is_file = 1 THEN size ELSE 0 END), 0)
             FROM files",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap_or((0, 0, 0));

    let skipped_paths: u64 = conn
        .query_row("SELECT COUNT(*) FROM skipped_paths", [], |r| r.get(0))
        .unwrap_or(0);

    let ignore_rules_count = get_ignore_rules(conn).len() as u64;

    let duplicate_file_groups: u64 = conn
        .query_row("SELECT COUNT(*) FROM duplicate_hashes", [], |r| r.get(0))
        .unwrap_or(0);

    let (duplicate_files, wasted_file_bytes): (u64, u64) = if duplicate_file_groups > 0 {
        conn.query_row(
            "SELECT COALESCE(SUM(cnt), 0), COALESCE(SUM((cnt - 1) * size), 0)
             FROM (
                SELECT COUNT(*) as cnt, MIN(f.size) as size
                FROM files f
                JOIN duplicate_hashes d ON f.hash = d.hash
                WHERE f.is_file = 1
                GROUP BY f.hash
             )",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((0, 0))
    } else {
        (0, 0)
    };

    let duplicate_folders: u64 = if duplicate_file_groups > 0 {
        conn.query_row(
            "SELECT COALESCE(SUM(cnt), 0)
             FROM (
                SELECT COUNT(*) as cnt
                FROM files f
                JOIN duplicate_hashes d ON f.hash = d.hash
                WHERE f.is_directory = 1
                GROUP BY f.hash
             )",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    } else {
        0
    };

    let duplicate_folder_groups: u64 = if duplicate_folders > 0 {
        conn.query_row(
            "SELECT COUNT(*)
             FROM (
                SELECT f.hash
                FROM files f
                JOIN duplicate_hashes d ON f.hash = d.hash
                WHERE f.is_directory = 1
                GROUP BY f.hash
                HAVING COUNT(*) > 1
             )",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0)
    } else {
        0
    };

    let stats = [
        ("total_files", total_files.to_string()),
        ("total_folders", total_folders.to_string()),
        ("total_size", total_size.to_string()),
        ("skipped_paths", skipped_paths.to_string()),
        ("ignore_rules_count", ignore_rules_count.to_string()),
        ("duplicate_file_groups", duplicate_file_groups.to_string()),
        ("duplicate_files", duplicate_files.to_string()),
        ("wasted_file_bytes", wasted_file_bytes.to_string()),
        ("duplicate_folder_groups", duplicate_folder_groups.to_string()),
        ("duplicate_folders", duplicate_folders.to_string()),
    ];

    for (key, value) in &stats {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO dashboard_stats (key, value, updated_at) VALUES (:key, :value, :updated_at)",
            named_params! { ":key": key, ":value": value, ":updated_at": now },
        );
    }

    let _ = conn.execute("DELETE FROM dashboard_timeline", []);
    let intervals = ["day", "week", "month", "year"];
    for interval in &intervals {
        let group_sql = match *interval {
            "day" => "strftime('%Y-%m-%d', modified, 'unixepoch')",
            "week" => "strftime('%Y-W%W', modified, 'unixepoch')",
            "year" => "strftime('%Y', modified, 'unixepoch')",
            _ => "strftime('%Y-%m', modified, 'unixepoch')",
        };
        let sql = format!(
            "INSERT INTO dashboard_timeline (interval_type, label, files, folders, size)
             SELECT ?1 as interval_type,
                    {group_sql} as label,
                    SUM(CASE WHEN is_file = 1 THEN 1 ELSE 0 END) as files,
                    SUM(CASE WHEN is_directory = 1 THEN 1 ELSE 0 END) as folders,
                    COALESCE(SUM(CASE WHEN is_file = 1 THEN size ELSE 0 END), 0) as size
             FROM files
             WHERE modified IS NOT NULL
             GROUP BY label
             ORDER BY label ASC"
        );
        let _ = conn.execute(&sql, [interval]);
    }
    logging::info(&format!("Dashboard stats recomputed: {} files, {} folders", total_files, total_folders));
}
