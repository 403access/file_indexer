#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileEntry {
    pub name: String,
    pub size: u64,
    pub modified: u64,
    pub hash: String,
}