//! Behavioral tests for multi_project.
//!
//! Tests verify observable outcomes: events emitted, payload shapes, registry state,
//! directory creation, and failure modes.
//! All assertions reference event names from events/multi_project_schema.md exactly.

use serde_json::{json, Value};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn binary_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_multi_project"))
}

fn setup() -> TempDir {
    tempfile::tempdir().unwrap()
}

fn run_init(registry: &str, name: &str, dir: &str) -> std::process::Output {
    Command::new(binary_path())
        .args(["--registry", registry, "init", name, "--dir", dir])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run multi_project init")
}

fn run_list(registry: &str) -> std::process::Output {
    Command::new(binary_path())
        .args(["--registry", registry, "list"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run multi_project list")
}

fn run_open(registry: &str, name: &str) -> std::process::Output {
    Command::new(binary_path())
        .args(["--registry", registry, "open", name])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run multi_project open")
}

fn read_events(registry: &str) -> Vec<Value> {
    let path = std::path::Path::new(registry).join("events.jsonl");
    if !path.exists() { return vec![]; }
    fs::read_to_string(path).unwrap()
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .collect()
}

// ── Happy Path 1: Successful Project Init ────────────────────────────────────

#[test]
fn test_init_emits_requested_then_initialized() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    let project_dir = tmp.path().join("myproject").to_string_lossy().into_owned();

    run_init(&registry, "MyProject", &project_dir);

    let events = read_events(&registry);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();

    assert!(types.contains(&"ProjectInitRequested"),  "ProjectInitRequested must be emitted");
    assert!(types.contains(&"ProjectInitialized"),    "ProjectInitialized must be emitted");

    let req_pos = types.iter().position(|&t| t == "ProjectInitRequested").unwrap();
    let ini_pos = types.iter().position(|&t| t == "ProjectInitialized").unwrap();
    assert!(req_pos < ini_pos, "ProjectInitRequested must precede ProjectInitialized");
}

#[test]
fn test_init_creates_project_directory() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    let project_dir = tmp.path().join("myproject");

    assert!(!project_dir.exists(), "Directory must not exist before init");
    run_init(&registry, "MyProject", project_dir.to_str().unwrap());
    assert!(project_dir.exists(), "Project directory must be created by init");
    assert!(project_dir.is_dir(), "Created path must be a directory");
}

#[test]
fn test_init_creates_events_subdirectory() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    let project_dir = tmp.path().join("myproject");

    run_init(&registry, "MyProject", project_dir.to_str().unwrap());

    let events_dir = project_dir.join("events");
    assert!(events_dir.exists(), "events/ subdirectory must be created so other binaries work immediately");
}

#[test]
fn test_init_initialized_payload_shape() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    let project_dir = tmp.path().join("myproject").to_string_lossy().into_owned();

    run_init(&registry, "MyProject", &project_dir);

    let events = read_events(&registry);
    let initialized = events.iter().find(|e| e["event_type"] == "ProjectInitialized")
        .expect("ProjectInitialized not found");

    let p = &initialized["payload"];
    assert_eq!(p["project_name"].as_str().unwrap(), "MyProject");
    assert!(p["project_dir"].as_str().is_some(), "project_dir must be present");
    assert!(!p["project_dir"].as_str().unwrap().is_empty(), "project_dir must not be empty");
}

#[test]
fn test_init_project_dir_in_payload_is_absolute() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    let project_dir = tmp.path().join("myproject").to_string_lossy().into_owned();

    run_init(&registry, "MyProject", &project_dir);

    let events = read_events(&registry);
    let initialized = events.iter().find(|e| e["event_type"] == "ProjectInitialized").unwrap();
    let stored_dir = initialized["payload"]["project_dir"].as_str().unwrap();

    assert!(
        std::path::Path::new(stored_dir).is_absolute(),
        "project_dir in ProjectInitialized must be an absolute path; got: {stored_dir}"
    );
}

#[test]
fn test_init_project_appears_in_subsequent_list() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    let project_dir = tmp.path().join("myproject").to_string_lossy().into_owned();

    run_init(&registry, "MyProject", &project_dir);
    let output = run_list(&registry);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("MyProject"), "Registered project must appear in list output");
}

// ── Happy Path 2: List Non-empty ─────────────────────────────────────────────

#[test]
fn test_list_emits_requested_then_returned() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    let project_dir = tmp.path().join("proj").to_string_lossy().into_owned();
    run_init(&registry, "ProjA", &project_dir);

    // Clear events so we only see list events
    let tmp2 = setup();
    let registry2 = tmp2.path().join("registry").to_string_lossy().into_owned();
    // Use a fresh registry with one project seeded via init
    run_init(&registry2, "ProjA", &tmp2.path().join("projA").to_string_lossy().into_owned());
    // Truncate events so far by using a separate list-only registry test
    let tmp3 = setup();
    let reg3 = tmp3.path().join("reg").to_string_lossy().into_owned();
    run_init(&reg3, "ProjA", &tmp3.path().join("a").to_string_lossy().into_owned());

    run_list(&reg3);

    let events = read_events(&reg3);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    let req_pos = types.iter().position(|&t| t == "ProjectListRequested").unwrap();
    let ret_pos = types.iter().position(|&t| t == "ProjectListReturned").unwrap();
    assert!(req_pos < ret_pos, "ProjectListRequested must precede ProjectListReturned");
}

#[test]
fn test_list_returned_payload_contains_registered_project() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    let project_dir = tmp.path().join("myproject").to_string_lossy().into_owned();
    run_init(&registry, "MyProject", &project_dir);
    run_list(&registry);

    let events = read_events(&registry);
    let returned = events.iter().rev().find(|e| e["event_type"] == "ProjectListReturned")
        .expect("ProjectListReturned not found");

    let projects = returned["payload"]["projects"].as_array().unwrap();
    assert!(
        projects.iter().any(|p| p["name"].as_str() == Some("MyProject")),
        "Registered project must appear in ProjectListReturned payload"
    );
}

#[test]
fn test_list_returned_project_count_matches_array_length() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    run_init(&registry, "A", &tmp.path().join("a").to_string_lossy().into_owned());
    run_init(&registry, "B", &tmp.path().join("b").to_string_lossy().into_owned());
    run_list(&registry);

    let events = read_events(&registry);
    let returned = events.iter().rev().find(|e| e["event_type"] == "ProjectListReturned").unwrap();
    let count = returned["payload"]["project_count"].as_u64().unwrap() as usize;
    let arr_len = returned["payload"]["projects"].as_array().unwrap().len();
    assert_eq!(count, arr_len, "project_count must equal the length of the projects array");
}

// ── Happy Path 3: List Empty ──────────────────────────────────────────────────

#[test]
fn test_list_empty_registry_emits_returned_with_zero_count() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();

    run_list(&registry);

    let events = read_events(&registry);
    let returned = events.iter().find(|e| e["event_type"] == "ProjectListReturned")
        .expect("ProjectListReturned must be emitted even for empty registry");
    assert_eq!(returned["payload"]["project_count"].as_u64().unwrap(), 0);
    assert_eq!(returned["payload"]["projects"].as_array().unwrap().len(), 0);
}

#[test]
fn test_list_empty_registry_no_failure_event() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();

    run_list(&registry);

    let events = read_events(&registry);
    assert!(
        !events.iter().any(|e| e["event_type"].as_str().unwrap_or("").contains("Failed")),
        "Empty registry list must not emit any failure event"
    );
}

// ── Happy Path 4: Open Registered Project ────────────────────────────────────

#[test]
fn test_open_emits_requested_then_path_returned() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    let project_dir = tmp.path().join("myproject").to_string_lossy().into_owned();
    run_init(&registry, "MyProject", &project_dir);

    run_open(&registry, "MyProject");

    let events = read_events(&registry);
    let types: Vec<&str> = events.iter().map(|e| e["event_type"].as_str().unwrap()).collect();
    assert!(types.contains(&"ProjectOpenRequested"), "ProjectOpenRequested must be emitted");
    assert!(types.contains(&"ProjectPathReturned"),  "ProjectPathReturned must be emitted");

    let req_pos = types.iter().position(|&t| t == "ProjectOpenRequested").unwrap();
    let ret_pos = types.iter().position(|&t| t == "ProjectPathReturned").unwrap();
    assert!(req_pos < ret_pos, "ProjectOpenRequested must precede ProjectPathReturned");
}

#[test]
fn test_open_prints_directory_path_to_stdout() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    let project_dir = tmp.path().join("myproject").to_string_lossy().into_owned();
    run_init(&registry, "MyProject", &project_dir);

    let output = run_open(&registry, "MyProject");
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    assert!(!stdout.is_empty(), "open must print the project directory path to stdout");
    assert!(
        std::path::Path::new(&stdout).exists(),
        "The path printed by open must exist on disk; got: {stdout}"
    );
}

#[test]
fn test_open_path_returned_payload_shape() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    let project_dir = tmp.path().join("myproject").to_string_lossy().into_owned();
    run_init(&registry, "MyProject", &project_dir);
    run_open(&registry, "MyProject");

    let events = read_events(&registry);
    let returned = events.iter().find(|e| e["event_type"] == "ProjectPathReturned")
        .expect("ProjectPathReturned not found");

    let p = &returned["payload"];
    assert_eq!(p["project_name"].as_str().unwrap(), "MyProject");
    assert!(p["project_dir"].as_str().is_some(), "project_dir must be present in payload");
}

// ── Failure Path 1: ProjectNameAlreadyExists ──────────────────────────────────

#[test]
fn test_init_duplicate_name_emits_failed_duplicate() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    run_init(&registry, "MyProject", &tmp.path().join("first").to_string_lossy().into_owned());
    run_init(&registry, "MyProject", &tmp.path().join("second").to_string_lossy().into_owned());

    let events = read_events(&registry);
    assert!(
        events.iter().any(|e| e["event_type"] == "ProjectInitFailedDuplicate"),
        "ProjectInitFailedDuplicate must be emitted on duplicate name"
    );
}

#[test]
fn test_init_duplicate_name_failure_reason() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    run_init(&registry, "MyProject", &tmp.path().join("first").to_string_lossy().into_owned());
    run_init(&registry, "MyProject", &tmp.path().join("second").to_string_lossy().into_owned());

    let events = read_events(&registry);
    let failure = events.iter().find(|e| e["event_type"] == "ProjectInitFailedDuplicate").unwrap();
    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "project_name_already_exists");
    assert_eq!(failure["payload"]["project_name"].as_str().unwrap(), "MyProject");
}

#[test]
fn test_init_duplicate_name_does_not_create_second_directory() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    run_init(&registry, "MyProject", &tmp.path().join("first").to_string_lossy().into_owned());

    let second_dir = tmp.path().join("second");
    run_init(&registry, "MyProject", &second_dir.to_string_lossy().into_owned());

    assert!(!second_dir.exists(), "Second directory must NOT be created on duplicate name");
}

#[test]
fn test_init_duplicate_name_registry_has_only_one_entry() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    run_init(&registry, "MyProject", &tmp.path().join("first").to_string_lossy().into_owned());
    run_init(&registry, "MyProject", &tmp.path().join("second").to_string_lossy().into_owned());
    run_list(&registry);

    let events = read_events(&registry);
    let returned = events.iter().rev().find(|e| e["event_type"] == "ProjectListReturned").unwrap();
    assert_eq!(
        returned["payload"]["project_count"].as_u64().unwrap(), 1,
        "Registry must contain only one entry after duplicate init attempt"
    );
}

// ── Failure Path 2: DirectoryNotAccessible ────────────────────────────────────

#[test]
fn test_init_unwritable_dir_emits_failed_directory_not_accessible() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();

    // Create a read-only parent directory so subdirectory creation fails
    let ro_parent = tmp.path().join("readonly_parent");
    fs::create_dir(&ro_parent).unwrap();
    fs::set_permissions(&ro_parent, fs::Permissions::from_mode(0o444)).unwrap();
    let blocked_dir = ro_parent.join("project");

    run_init(&registry, "Blocked", &blocked_dir.to_string_lossy().into_owned());

    fs::set_permissions(&ro_parent, fs::Permissions::from_mode(0o755)).unwrap(); // cleanup

    let events = read_events(&registry);
    assert!(
        events.iter().any(|e| e["event_type"] == "ProjectInitFailedDirectoryNotAccessible"),
        "ProjectInitFailedDirectoryNotAccessible must be emitted when directory cannot be created"
    );
}

#[test]
fn test_init_unwritable_dir_failure_payload() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();

    let ro_parent = tmp.path().join("ro");
    fs::create_dir(&ro_parent).unwrap();
    fs::set_permissions(&ro_parent, fs::Permissions::from_mode(0o444)).unwrap();
    let blocked_dir = ro_parent.join("proj");

    run_init(&registry, "Blocked", &blocked_dir.to_string_lossy().into_owned());

    fs::set_permissions(&ro_parent, fs::Permissions::from_mode(0o755)).unwrap();

    let events = read_events(&registry);
    let failure = events.iter()
        .find(|e| e["event_type"] == "ProjectInitFailedDirectoryNotAccessible")
        .unwrap();

    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "directory_not_accessible");
    assert_eq!(failure["payload"]["project_name"].as_str().unwrap(), "Blocked");
    assert!(failure["payload"]["project_dir"].as_str().is_some());
}

#[test]
fn test_init_unwritable_dir_does_not_modify_registry() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    run_init(&registry, "Good", &tmp.path().join("good").to_string_lossy().into_owned());

    let ro_parent = tmp.path().join("ro");
    fs::create_dir(&ro_parent).unwrap();
    fs::set_permissions(&ro_parent, fs::Permissions::from_mode(0o444)).unwrap();
    let blocked_dir = ro_parent.join("bad");

    run_init(&registry, "Bad", &blocked_dir.to_string_lossy().into_owned());
    fs::set_permissions(&ro_parent, fs::Permissions::from_mode(0o755)).unwrap();

    run_list(&registry);
    let events = read_events(&registry);
    let returned = events.iter().rev().find(|e| e["event_type"] == "ProjectListReturned").unwrap();
    assert_eq!(
        returned["payload"]["project_count"].as_u64().unwrap(), 1,
        "Registry must contain only the successful project after a directory-not-accessible failure"
    );
}

// ── Failure Path 3: ProjectNotFound ──────────────────────────────────────────

#[test]
fn test_open_unknown_project_emits_failed_not_found() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();

    run_open(&registry, "Nonexistent");

    let events = read_events(&registry);
    assert!(
        events.iter().any(|e| e["event_type"] == "ProjectOpenFailedNotFound"),
        "ProjectOpenFailedNotFound must be emitted for unknown project"
    );
}

#[test]
fn test_open_unknown_project_failure_payload() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();

    run_open(&registry, "Nonexistent");

    let events = read_events(&registry);
    let failure = events.iter().find(|e| e["event_type"] == "ProjectOpenFailedNotFound").unwrap();
    assert_eq!(failure["payload"]["failure_reason"].as_str().unwrap(), "project_not_found");
    assert_eq!(failure["payload"]["project_name"].as_str().unwrap(), "Nonexistent");
}

#[test]
fn test_open_unknown_project_no_path_returned() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();

    run_open(&registry, "Nonexistent");

    let events = read_events(&registry);
    assert!(
        !events.iter().any(|e| e["event_type"] == "ProjectPathReturned"),
        "ProjectPathReturned must NOT be emitted for unknown project"
    );
}

// ── Invariant: project isolation ─────────────────────────────────────────────

#[test]
fn test_two_projects_are_registered_independently() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    run_init(&registry, "Alpha", &tmp.path().join("alpha").to_string_lossy().into_owned());
    run_init(&registry, "Beta",  &tmp.path().join("beta").to_string_lossy().into_owned());

    run_list(&registry);
    let events = read_events(&registry);
    let returned = events.iter().rev().find(|e| e["event_type"] == "ProjectListReturned").unwrap();
    let count = returned["payload"]["project_count"].as_u64().unwrap();
    assert_eq!(count, 2, "Both projects must be independently registered");

    let projects = returned["payload"]["projects"].as_array().unwrap();
    let names: Vec<&str> = projects.iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"Alpha"), "Alpha must be in registry");
    assert!(names.contains(&"Beta"),  "Beta must be in registry");
}

// ── Telemetry ─────────────────────────────────────────────────────────────────

#[test]
fn test_all_events_have_required_base_fields() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    run_list(&registry); // simplest invocation

    let events = read_events(&registry);
    assert!(!events.is_empty());

    for event in &events {
        let t = event["event_type"].as_str().unwrap_or("unknown");
        assert!(!event["event_id"].is_null(),       "{t}: event_id must be present");
        assert!(!event["event_type"].is_null(),     "{t}: event_type must be present");
        assert!(!event["timestamp"].is_null(),      "{t}: timestamp must be present");
        assert!(!event["correlation_id"].is_null(), "{t}: correlation_id must be present");
        assert!(!event["source_module"].is_null(),  "{t}: source_module must be present");
        assert!(!event["payload"].is_null(),        "{t}: payload must be present");
        assert_eq!(
            event["source_module"].as_str().unwrap(), "multi_project",
            "{t}: source_module must be 'multi_project'"
        );
        assert!(event["timestamp"].as_u64().unwrap() > 0, "{t}: timestamp must be positive");
    }
}

#[test]
fn test_correlation_id_consistent_within_one_invocation() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    run_list(&registry);

    let events = read_events(&registry);
    assert!(events.len() >= 2);
    let first_cid = events[0]["correlation_id"].as_str().unwrap();
    for event in &events {
        assert_eq!(
            event["correlation_id"].as_str().unwrap(), first_cid,
            "All events from one invocation must share the same correlation_id"
        );
    }
}

#[test]
fn test_separate_invocations_have_different_correlation_ids() {
    let tmp = setup();
    let registry = tmp.path().join("registry").to_string_lossy().into_owned();
    run_list(&registry);
    run_list(&registry);

    let events = read_events(&registry);
    let cids: Vec<&str> = events.iter()
        .filter(|e| e["event_type"] == "ProjectListRequested")
        .map(|e| e["correlation_id"].as_str().unwrap())
        .collect();

    assert_eq!(cids.len(), 2);
    assert_ne!(cids[0], cids[1], "Different invocations must produce different correlation_ids");
}
