pub mod arrange;

#[test]
pub fn actual_test() {
    if arrange::database_exists() {
        panic!("Test database already exists. Deleting it before running the test.");
    }

    arrange::create_database();

    let temp_folder_path = arrange::create_test_data();
    arrange::delete_test_data(&temp_folder_path);

    arrange::delete_database();
}
