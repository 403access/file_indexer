pub mod arrange;

#[test]
pub fn actual_test() {
    if arrange::database_exists() {
        panic!("Test database already exists. Deleting it before running the test.");
    }

    let temp_folder_path = arrange::create_test_data();
    let _database_file_path = temp_folder_path.join("file_index.db");
    let database_file_path = arrange::create_database(&_database_file_path);

    arrange::delete_database(&database_file_path);
    arrange::delete_test_data(&temp_folder_path);
}
