use crate::file_entry::_types::FileEntry;

pub enum SortOrder {
    Default,
    ABCabc,
    AaBbCc,
}

pub fn sort_dir_entries(sort_order: SortOrder, dir_entries: &mut Vec<FileEntry>) {
    match sort_order {
        SortOrder::Default => {
            dir_entries.sort();
        }
        SortOrder::ABCabc => {
            dir_entries.sort_by(|a, b| a.name.cmp(&b.name));
        }
        SortOrder::AaBbCc => {
            dir_entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        }
    }
}
