#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileEntry {

    // File type flags
    pub is_directory: bool,
    pub is_file: bool,
    pub is_symlink: bool,

    // File metadata
    pub path: Option<String>,
    pub name: String,
    pub size: u64,

    // Timestamps
    pub created: Option<u64>,
    pub modified: Option<u64>,
    pub accessed: Option<u64>,

    /// Hash: can be empty if not generated yet.
    pub hash: Option<String>,
}