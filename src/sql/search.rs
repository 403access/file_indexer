use crate::{
    commands::command_search_file::{OrderKind, PatternKind, TargetKind},
    file_entry::{_types::FileEntry, convert::convert_from_rows},
};

pub fn search_file(
    transaction: &rusqlite::Transaction,
    name: &str,
    target_kind: TargetKind,
    pattern_kind: PatternKind,
    order_kind: OrderKind,
) -> rusqlite::Result<Vec<FileEntry>> {
    // Determine the order direction
    let order = match order_kind {
        OrderKind::Asc => "ASC",
        OrderKind::Desc => "DESC",
    };

    let sql = format!(
        "
    SELECT
        f.path, fn.name, f.size, f.modified, f.hash,
        f.is_directory, f.is_file, f.is_symlink
     FROM
        files f
     JOIN
        file_names fn ON f.file_name_id = fn.id
     WHERE
        fn.name LIKE ?
        AND (
            f.is_directory = ?
            OR f.is_file = ?
        )
     ORDER BY
        fn.name {}",
        order
    );

    let mut stmt = transaction.prepare(&sql)?;

    // Determine if we are searching for directories, files, or both
    let is_directory = matches!(target_kind, TargetKind::Folders | TargetKind::Both);
    let is_file = matches!(target_kind, TargetKind::Files | TargetKind::Both);

    // Prepare the pattern based on the pattern kind
    let pattern = match pattern_kind {
        PatternKind::Exact => format!("{}", name),
        PatternKind::StartsWith => format!("{}%", name),
        PatternKind::EndsWith => format!("%{}", name),
        PatternKind::Contains => format!("%{}%", name),
    };

    let mut rows = stmt.query(rusqlite::params![
        pattern,
        if is_directory { 1 } else { 0 },
        if is_file { 1 } else { 0 },
    ])?;

    convert_from_rows(&mut rows)
}
