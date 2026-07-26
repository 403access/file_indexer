use crate::modules::file_entry::_types::FileEntry;
use crate::modules::file_entry::sort::{sort_dir_entries, SortOrder};

fn make_entry(name: &str) -> FileEntry {
    FileEntry {
        path: None,
        name: name.to_string(),
        size: 0,
        created: None,
        modified: None,
        accessed: None,
        hash: None,
        is_directory: false,
        is_file: true,
        is_symlink: false,
        parent_path: None,
    }
}

#[test]
fn sort_default_orders_by_ord() {
    let mut entries = vec![make_entry("c"), make_entry("a"), make_entry("b")];
    sort_dir_entries(SortOrder::Default, &mut entries);
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["a", "b", "c"]);
}

#[test]
fn sort_abc_abc_is_case_sensitive() {
    let mut entries = vec![make_entry("B"), make_entry("a"), make_entry("A"), make_entry("b")];
    sort_dir_entries(SortOrder::ABCabc, &mut entries);
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["A", "B", "a", "b"]);
}

#[test]
fn sort_aabbcc_is_case_insensitive() {
    let mut entries = vec![make_entry("B"), make_entry("a"), make_entry("A"), make_entry("b")];
    sort_dir_entries(SortOrder::AaBbCc, &mut entries);
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["a", "A", "B", "b"]);
}

#[test]
fn sort_empty_list_stays_empty() {
    let mut entries: Vec<FileEntry> = vec![];
    sort_dir_entries(SortOrder::Default, &mut entries);
    assert!(entries.is_empty());
}

#[test]
fn sort_single_element_stays_same() {
    let mut entries = vec![make_entry("only")];
    sort_dir_entries(SortOrder::Default, &mut entries);
    assert_eq!(entries[0].name, "only");
}

#[test]
fn sort_preserves_entry_data() {
    let mut entries = vec![make_entry("b"), make_entry("a")];
    sort_dir_entries(SortOrder::Default, &mut entries);
    assert_eq!(entries[0].name, "a");
    assert!(entries[0].is_file);
}
