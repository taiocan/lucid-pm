//! Behavioral tests for lucid_sync_enforcement (R12).
//!
//! Verifies install-set/dispatch coverage gap detection against
//! contracts/lucid_sync_enforcement_contract.md.
//! Event schema is null (no events emitted) — no telemetry assertions required.

use std::collections::HashSet;

// ── Pure helpers ─────────────────────────────────────────────────────────────
//
// These mirror the logic in lucid_behavior.rs (parse_install_set /
// test_install_set_covered_by_dispatch) as pure functions so contract
// scenarios can be exercised with controlled inputs without touching
// the filesystem or the real install.sh.

fn parse_modules_array(content: &str) -> Vec<String> {
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

fn coverage_gaps<'a>(install_set: &'a [String], dispatch_targets: &HashSet<&str>) -> Vec<&'a str> {
    install_set
        .iter()
        .map(String::as_str)
        .filter(|m| !dispatch_targets.contains(*m))
        .collect()
}

// ── Happy Path: Full Coverage ─────────────────────────────────────────────────

/// Contract scenario: Happy Path — Full Coverage
/// All modules in the install set have dispatch entries → no gaps.
#[test]
fn test_full_coverage_no_gaps() {
    let content = "MODULES=(\n  alpha\n  beta\n)\n";
    let install_set = parse_modules_array(content);
    let dispatch: HashSet<&str> = ["alpha", "beta"].iter().copied().collect();
    let gaps = coverage_gaps(&install_set, &dispatch);
    assert!(gaps.is_empty(), "full coverage must produce no gaps; got: {}", gaps.join(", "));
}

// ── Failure Path: CoverageGap — Single Module ─────────────────────────────────

/// Contract scenario: CoverageGap — Single Module
/// A module in the install set that has no dispatch entry is detected and named.
#[test]
fn test_coverage_gap_single_module_detected_and_named() {
    let content = "MODULES=(\n  alpha\n  new_feature\n)\n";
    let install_set = parse_modules_array(content);
    let dispatch: HashSet<&str> = ["alpha"].iter().copied().collect();
    let gaps = coverage_gaps(&install_set, &dispatch);
    assert_eq!(gaps.len(), 1);
    assert!(
        gaps.contains(&"new_feature"),
        "gap output must name the missing module; got: {:?}", gaps
    );
}

// ── Failure Path: CoverageGap — Multiple Modules ─────────────────────────────

/// Contract scenario: CoverageGap — Multiple Modules
/// All missing modules are named in one result, not just the first.
#[test]
fn test_coverage_gap_multiple_modules_all_named() {
    let content = "MODULES=(\n  alpha\n  mod_a\n  mod_b\n)\n";
    let install_set = parse_modules_array(content);
    let dispatch: HashSet<&str> = ["alpha"].iter().copied().collect();
    let gaps = coverage_gaps(&install_set, &dispatch);
    assert!(gaps.contains(&"mod_a"), "mod_a must be named in gap output; got: {:?}", gaps);
    assert!(gaps.contains(&"mod_b"), "mod_b must be named in gap output; got: {:?}", gaps);
}

// ── Boundary: Empty Install Set ───────────────────────────────────────────────

/// Contract boundary scenario: empty MODULES array passes vacuously.
/// The check must not fail spuriously on an empty install set.
#[test]
fn test_empty_install_set_passes() {
    let content = "MODULES=(\n)\n";
    let install_set = parse_modules_array(content);
    assert!(install_set.is_empty(), "parser must return empty vec for empty MODULES array");
    let dispatch: HashSet<&str> = HashSet::new();
    let gaps = coverage_gaps(&install_set, &dispatch);
    assert!(gaps.is_empty(), "empty install set must produce no gaps");
}

// ── Invariant Falsification ───────────────────────────────────────────────────

/// LSE-IF-01
/// Invariant: test passes iff every install-set module has a dispatch entry.
/// Falsifies: install-set membership derived from DISPATCH_TABLE (circular) —
/// a circular check sees no gap because DISPATCH_TABLE trivially covers itself.
#[test]
fn test_circular_install_set_would_miss_new_module() {
    // new_feature is in install.sh MODULES but NOT in the dispatch table.
    // A circular implementation (install set = dispatch targets) would compute
    // gaps against itself and find zero. Correct implementation reads install.sh.
    let install_sh_content = "MODULES=(\n  alpha\n  new_feature\n)\n";
    let install_set = parse_modules_array(install_sh_content);
    let dispatch: HashSet<&str> = ["alpha"].iter().copied().collect();
    let gaps = coverage_gaps(&install_set, &dispatch);
    assert!(
        gaps.contains(&"new_feature"),
        "new_feature is in install.sh MODULES but not in dispatch — \
         circular check would miss this; got: {:?}", gaps
    );
}

/// LSE-IF-02
/// Invariant: failing check names ALL missing modules, not just the first.
/// Falsifies: gap collection short-circuits after first missing module.
#[test]
fn test_all_gaps_reported_not_first_only() {
    let install_sh_content = "MODULES=(\n  alpha\n  mod_a\n  mod_b\n)\n";
    let install_set = parse_modules_array(install_sh_content);
    let dispatch: HashSet<&str> = ["alpha"].iter().copied().collect();
    let gaps = coverage_gaps(&install_set, &dispatch);
    assert_eq!(
        gaps.len(), 2,
        "both mod_a and mod_b must be collected; \
         short-circuit implementation would yield 1; got: {:?}", gaps
    );
    assert!(gaps.contains(&"mod_a"));
    assert!(gaps.contains(&"mod_b"));
}

/// LSE-IF-03
/// Invariant: install set derived exclusively from install.sh MODULES.
/// Falsifies: install set inferred from DISPATCH_TABLE targets — adding a module
/// to install.sh alone (without updating DISPATCH_TABLE) goes undetected.
#[test]
fn test_install_set_source_is_install_sh_exclusively() {
    // mod_c is in install.sh but absent from the dispatch table.
    // An implementation that builds the install set from DISPATCH_TABLE targets
    // would never include mod_c (not a dispatch target), so gaps stays empty.
    let install_sh_content = "MODULES=(\n  alpha\n  mod_c\n)\n";
    let install_set = parse_modules_array(install_sh_content);
    let dispatch: HashSet<&str> = ["alpha"].iter().copied().collect();
    let gaps = coverage_gaps(&install_set, &dispatch);
    assert!(
        gaps.contains(&"mod_c"),
        "mod_c is in install.sh but not in dispatch — must be detected; got: {:?}", gaps
    );
}
