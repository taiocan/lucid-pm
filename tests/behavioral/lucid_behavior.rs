//! Behavioral tests for lucid.
//!
//! Tests verify observable outcomes by invoking bin/lucid via bash and checking
//! stdout, stderr, and exit codes. No events are expected — lucid has a null
//! event spine by design (see events/lucid_schema.md).
//!
//! All assertions reference contract clauses from contracts/lucid_contract.md.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap() // modules/
        .parent().unwrap() // LucidPM/
        .to_path_buf()
}

fn lucid_script() -> PathBuf {
    repo_root().join("bin/lucid")
}

fn set_exec(p: &Path) {
    fs::set_permissions(p, fs::Permissions::from_mode(0o755)).unwrap();
}

/// Copy bin/lucid into a temp dir so LUCID_BIN_DIR resolves to that dir.
fn install_lucid(temp: &TempDir) -> PathBuf {
    let dest = temp.path().join("lucid");
    fs::copy(lucid_script(), &dest).unwrap();
    set_exec(&dest);
    dest
}

/// Create a mock binary that echoes its own name and all received args, then exits 0.
fn mock_echo(dir: &Path, name: &str) {
    let p = dir.join(name);
    fs::write(&p, format!(
        "#!/usr/bin/env bash\necho \"{name}\"\nfor arg in \"$@\"; do echo \"arg:$arg\"; done\n"
    )).unwrap();
    set_exec(&p);
}

/// Create a mock binary with specific stdout, stderr, and exit code.
fn mock_fixed(dir: &Path, name: &str, stdout: &str, stderr: &str, exit_code: i32) {
    let p = dir.join(name);
    fs::write(&p, format!(
        "#!/usr/bin/env bash\nprintf '%s\\n' '{stdout}'\nprintf '%s\\n' '{stderr}' >&2\nexit {exit_code}\n"
    )).unwrap();
    set_exec(&p);
}

fn run(lucid: &Path, args: &[&str]) -> Output {
    Command::new("bash").arg(lucid).args(args).output().unwrap()
}

fn run_in(lucid: &Path, cwd: &Path, args: &[&str]) -> Output {
    Command::new("bash").arg(lucid).args(args).current_dir(cwd).output().unwrap()
}

// ── Happy Path: Known Command Dispatched ──────────────────────────────────────

#[test]
fn test_known_command_dispatched_to_correct_module() {
    let temp = TempDir::new().unwrap();
    let lucid = install_lucid(&temp);
    mock_echo(temp.path(), "pm_structuring");
    let out = run(&lucid, &["extract"]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("pm_structuring"));
}

#[test]
fn test_args_passed_unchanged() {
    let temp = TempDir::new().unwrap();
    let lucid = install_lucid(&temp);
    mock_echo(temp.path(), "pm_structuring");
    let out = run(&lucid, &["extract", "--folder", "notes/", "--yes"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("arg:--folder"), "expected --folder in args");
    assert!(stdout.contains("arg:notes/"),   "expected notes/ in args");
    assert!(stdout.contains("arg:--yes"),    "expected --yes in args");
}

#[test]
fn test_exit_code_passthrough() {
    let temp = TempDir::new().unwrap();
    let lucid = install_lucid(&temp);
    mock_fixed(temp.path(), "pm_structuring", "", "", 42);
    let out = run(&lucid, &["extract"]);
    assert_eq!(out.status.code(), Some(42), "exit code must pass through unchanged");
}

#[test]
fn test_stdout_passthrough() {
    let temp = TempDir::new().unwrap();
    let lucid = install_lucid(&temp);
    mock_fixed(temp.path(), "pm_structuring", "stdout-sentinel", "", 0);
    let out = run(&lucid, &["extract"]);
    assert!(String::from_utf8_lossy(&out.stdout).contains("stdout-sentinel"));
}

#[test]
fn test_stderr_passthrough() {
    let temp = TempDir::new().unwrap();
    let lucid = install_lucid(&temp);
    mock_fixed(temp.path(), "pm_structuring", "", "stderr-sentinel", 0);
    let out = run(&lucid, &["extract"]);
    assert!(String::from_utf8_lossy(&out.stderr).contains("stderr-sentinel"));
}

// ── Happy Path: Help Output ───────────────────────────────────────────────────

#[test]
fn test_help_lists_all_commands() {
    let out = run(&lucid_script(), &["help"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    for cmd in EXPECTED_COMMANDS {
        assert!(stdout.contains(cmd), "help output missing command: {cmd}");
    }
}

#[test]
fn test_help_includes_usage_examples() {
    let out = run(&lucid_script(), &["help"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Verify dispatcher-owned usage examples are present for a sample of commands
    assert!(stdout.contains("lucid extract"), "missing extract example");
    assert!(stdout.contains("lucid state"),   "missing state example");
    assert!(stdout.contains("lucid task add"), "missing task example");
    assert!(stdout.contains("lucid schema"),  "missing schema example");
}

#[test]
fn test_help_exit_code_zero() {
    let out = run(&lucid_script(), &["help"]);
    assert_eq!(out.status.code(), Some(0));
}

// ── Boundary: No Arguments ────────────────────────────────────────────────────

#[test]
fn test_no_args_equals_help() {
    let help_out  = run(&lucid_script(), &["help"]);
    let noarg_out = run(&lucid_script(), &[]);
    assert_eq!(
        String::from_utf8_lossy(&noarg_out.stdout),
        String::from_utf8_lossy(&help_out.stdout),
        "no-args output must be identical to `lucid help`"
    );
    assert_eq!(noarg_out.status.code(), Some(0));
}

// ── Failure: UnknownCommand ───────────────────────────────────────────────────

#[test]
fn test_unknown_command_nonzero_exit() {
    let out = run(&lucid_script(), &["unknown-xyz"]);
    assert_ne!(out.status.code(), Some(0), "UnknownCommand must exit non-zero");
}

#[test]
fn test_unknown_command_names_command_in_stderr() {
    let out = run(&lucid_script(), &["unknown-xyz"]);
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("unknown-xyz"),
        "stderr must name the unrecognized command"
    );
}

#[test]
fn test_unknown_command_references_lucid_help_in_stderr() {
    let out = run(&lucid_script(), &["unknown-xyz"]);
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("lucid help"),
        "stderr must reference 'lucid help'"
    );
}

#[test]
fn test_unknown_command_no_stdout() {
    let out = run(&lucid_script(), &["unknown-xyz"]);
    assert!(
        String::from_utf8_lossy(&out.stdout).trim().is_empty(),
        "UnknownCommand must not write to stdout"
    );
}

// ── Invariant Falsification ───────────────────────────────────────────────────
// Tests IDs: LUC-IF-01 through LUC-IF-05

/// LUC-IF-01
/// For each (X → M) in the routing table, lucid X reaches module M.
/// Falsifies: dispatch table maps X to a wrong or absent target.
#[test]
fn test_routing_table_entry_reaches_correct_module() {
    for (cmd, module) in DISPATCH_TABLE {
        let temp = TempDir::new().unwrap();
        let lucid = install_lucid(&temp);
        mock_echo(temp.path(), module);
        let out = run(&lucid, &[cmd]);
        assert!(
            String::from_utf8_lossy(&out.stdout).contains(module),
            "lucid {cmd} must dispatch to {module}"
        );
    }
}

/// LUC-IF-02
/// Every routing table entry appears in help output.
/// Falsifies: help block maintained independently — X reachable but unlisted.
#[test]
fn test_dispatch_to_help_parity() {
    let stdout = String::from_utf8_lossy(&run(&lucid_script(), &["help"]).stdout).to_string();
    for (cmd, _) in DISPATCH_TABLE {
        assert!(
            stdout.contains(cmd),
            "command '{cmd}' has a dispatch case but is absent from help output"
        );
    }
}

/// LUC-IF-03
/// Every command in help output has a dispatch case (is invocable without UnknownCommand).
/// Falsifies: help lists commands that have no dispatch case.
#[test]
fn test_help_to_dispatch_parity() {
    for (cmd, module) in DISPATCH_TABLE {
        let temp = TempDir::new().unwrap();
        let lucid = install_lucid(&temp);
        mock_echo(temp.path(), module);
        let out = run(&lucid, &[cmd]);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            !stderr.contains("unknown command"),
            "command '{cmd}' appears in help but produces UnknownCommand on invocation"
        );
    }
    // Meta-commands (version, help) must also not produce UnknownCommand
    for meta in &["version", "help"] {
        let out = run(&lucid_script(), &[meta]);
        assert!(
            !String::from_utf8_lossy(&out.stderr).contains("unknown command"),
            "meta-command '{meta}' appears in help but produces UnknownCommand"
        );
    }
}

/// LUC-IF-04
/// Invoking lucid X [args] produces the same stdout, stderr, and exit code as the module directly.
/// Falsifies: dispatcher buffers/transforms output or normalizes exit codes.
#[test]
fn test_same_stdout_stderr_exit_code_as_direct() {
    let temp = TempDir::new().unwrap();
    let lucid = install_lucid(&temp);
    mock_fixed(temp.path(), "pm_structuring", "expected-stdout", "expected-stderr", 7);
    let out = run(&lucid, &["extract"]);
    assert_eq!(out.status.code(), Some(7), "exit code must pass through unchanged");
    assert!(String::from_utf8_lossy(&out.stdout).contains("expected-stdout"));
    assert!(String::from_utf8_lossy(&out.stderr).contains("expected-stderr"));
}

/// LUC-IF-05
/// UnknownCommand error names the command and references lucid help.
/// Falsifies: generic error text without command name, or exits 0, or writes to stdout.
#[test]
fn test_unknown_command_error_content_complete() {
    let out = run(&lucid_script(), &["xyzzy-nonexistent"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_ne!(out.status.code(), Some(0),      "must exit non-zero");
    assert!(stderr.contains("xyzzy-nonexistent"), "must name the unrecognized command");
    assert!(stderr.contains("lucid help"),        "must reference lucid help");
    assert!(String::from_utf8_lossy(&out.stdout).trim().is_empty(), "error must go to stderr only");
}

// ── Null Event Spine ──────────────────────────────────────────────────────────

/// lucid emits no events on successful dispatch (null event spine).
#[test]
fn test_no_events_emitted_on_dispatch() {
    let temp = TempDir::new().unwrap();
    let lucid = install_lucid(&temp);
    // Mock that does not emit events
    let pm = temp.path().join("pm_structuring");
    fs::write(&pm, "#!/usr/bin/env bash\nexit 0\n").unwrap();
    set_exec(&pm);
    run_in(&lucid, temp.path(), &["extract"]);
    assert_no_lucid_events(temp.path());
}

/// lucid emits no events on UnknownCommand (null event spine).
#[test]
fn test_no_events_emitted_on_unknown_command() {
    let temp = TempDir::new().unwrap();
    let lucid = install_lucid(&temp);
    run_in(&lucid, temp.path(), &["unknown-xyz"]);
    assert_no_lucid_events(temp.path());
}

fn assert_no_lucid_events(dir: &Path) {
    let events_path = dir.join("events/runtime_events.jsonl");
    if !events_path.exists() { return; }
    let content = fs::read_to_string(&events_path).unwrap();
    for line in content.lines().filter(|l| !l.is_empty()) {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_ne!(
            v["source_module"].as_str(), Some("lucid"),
            "null event spine violated — lucid emitted an event: {line}"
        );
    }
}

// ── Routing table (authoritative for parity tests) ───────────────────────────

const DISPATCH_TABLE: &[(&str, &str)] = &[
    ("extract",  "pm_structuring"),
    ("state",    "project_state"),
    ("status",   "item_status"),
    ("link",     "item_links"),
    ("export",   "logseq_export"),
    ("sync",     "logseq_sync"),
    ("project",  "multi_project"),
    ("priority", "priority_view"),
    ("report",   "report_export"),
    ("suggest",  "ontology_suggest"),
    ("journal",  "journal"),
    ("schema",   "project_schema"),
    ("task",     "task_model"),
];

const EXPECTED_COMMANDS: &[&str] = &[
    "extract", "state", "status", "link", "export", "sync", "project",
    "priority", "report", "suggest", "journal", "schema", "task",
    "version", "help",
];

// ── Install-Set / Dispatch Coverage (R12) ─────────────────────────────────────

/// Parse the MODULES=( ... ) array from install.sh, returning one module name per entry.
/// Authoritative source for the install set per contracts/lucid_sync_enforcement_contract.md.
fn parse_install_set() -> Vec<String> {
    let path = repo_root().join("install.sh");
    let content = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read install.sh at {}: {}", path.display(), e));
    let mut modules = Vec::new();
    let mut in_array = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "MODULES=(" {
            in_array = true;
            continue;
        }
        if in_array {
            if trimmed == ")" {
                break;
            }
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                modules.push(trimmed.to_string());
            }
        }
    }
    modules
}

/// R12: every module in the install set (install.sh MODULES) must have a
/// corresponding dispatch entry. Failure names all missing modules.
#[test]
fn test_install_set_covered_by_dispatch() {
    let install_set = parse_install_set();
    let dispatch_targets: std::collections::HashSet<&str> =
        DISPATCH_TABLE.iter().map(|(_, m)| *m).collect();
    let gaps: Vec<&str> = install_set
        .iter()
        .map(String::as_str)
        .filter(|m| !dispatch_targets.contains(*m))
        .collect();
    assert!(
        gaps.is_empty(),
        "install-set modules with no dispatch entry: {}",
        gaps.join(", ")
    );
}
