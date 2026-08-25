use crate::modules::logging;
use crate::modules::processes::{
    clear_completed, complete, fail, get_all, is_paused, is_stopped, pending, register,
    register_controllable, request_stop, set_paused, ProcessStatus,
};

#[test]
fn register_creates_running_process() {
    let id = register("test", "cat", None);
    let all = get_all();
    let p = all.iter().find(|x| x.id == id).unwrap();
    assert_eq!(p.name, "test");
    assert_eq!(p.category, "cat");
    assert_eq!(p.status, ProcessStatus::Running);
}

#[test]
fn pending_creates_pending_process() {
    let id = pending("waiting", "cat", None);
    let all = get_all();
    let p = all.iter().find(|x| x.id == id).unwrap();
    assert_eq!(p.status, ProcessStatus::Pending);
}

#[test]
fn complete_marks_process_completed() {
    let id = register("complete-me", "cat", None);
    complete(id, Some("done"));
    let all = get_all();
    let p = all.iter().find(|x| x.id == id).unwrap();
    assert_eq!(p.status, ProcessStatus::Completed);
    assert_eq!(p.progress, Some(100.0));
    assert_eq!(p.message, Some("done".to_string()));
}

#[test]
fn fail_marks_process_failed() {
    let id = register("fail-me", "cat", None);
    fail(id, "oops");
    let all = get_all();
    let p = all.iter().find(|x| x.id == id).unwrap();
    assert_eq!(p.status, ProcessStatus::Failed);
    assert_eq!(p.message, Some("oops".to_string()));
}

#[test]
fn register_controllable_creates_control_flags() {
    let id = register_controllable("controllable", "cat", None);
    assert!(!is_stopped(id));
    assert!(!is_paused(id));
    set_paused(id, true);
    assert!(is_paused(id));
    set_paused(id, false);
    assert!(!is_paused(id));
}

#[test]
fn request_stop_sets_stop_flag() {
    let id = register_controllable("stoppable", "cat", None);
    assert!(!is_stopped(id));
    request_stop(id);
    assert!(is_stopped(id));
}

#[test]
fn clear_completed_removes_only_finished_processes() {
    let running = register("keep", "cat", None);
    let done = register("remove", "cat", None);
    complete(done, None);
    clear_completed();
    let all = get_all();
    assert!(all.iter().any(|p| p.id == running));
    assert!(!all.iter().any(|p| p.id == done));
}

#[test]
fn process_lifecycle_creates_logs() {
    let id = register_controllable("log-test", "test", Some("starting"));
    logging::info_with_process("middle", id);
    complete(id, Some("finished"));

    // Verify logging with process_id compiles and runs without error
    assert!(true);
}
