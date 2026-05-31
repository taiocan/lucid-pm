use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const SOURCE_MODULE: &str = "multi_project";

#[derive(Parser)]
#[command(about = "Manage multiple LucidPM projects")]
struct Cli {
    /// Registry directory (defaults to ~/.lucidpm)
    #[arg(long, global = true)]
    registry: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create and register a new named project
    Init {
        /// Project name
        name: String,
        /// Directory path for the new project
        #[arg(long)]
        dir: String,
    },
    /// List all registered projects
    List,
    /// Print the directory path of a registered project
    Open {
        /// Project name
        name: String,
    },
}

fn default_registry_dir() -> PathBuf {
    std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(".lucidpm"))
        .unwrap_or_else(|_| PathBuf::from(".lucidpm"))
}

fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn emit_event(events_file: &Path, event_type: &str, correlation_id: &str, payload: Value) {
    let event = json!({
        "event_id": Uuid::new_v4().to_string(),
        "event_type": event_type,
        "timestamp": timestamp_ms(),
        "correlation_id": correlation_id,
        "source_module": SOURCE_MODULE,
        "payload": payload,
    });
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(events_file)
        .expect("Failed to open events file");
    writeln!(file, "{}", event).expect("Failed to write event");
}

fn registry_path(registry_dir: &Path) -> PathBuf {
    registry_dir.join("projects.json")
}

fn events_path(registry_dir: &Path) -> PathBuf {
    registry_dir.join("events.jsonl")
}

fn ensure_registry_dir(registry_dir: &Path) -> Result<()> {
    fs::create_dir_all(registry_dir)
        .with_context(|| format!("creating registry directory '{}'", registry_dir.display()))
}

fn read_registry(registry_dir: &Path) -> Result<Vec<(String, String)>> {
    let path = registry_path(registry_dir);
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = fs::read_to_string(&path)
        .with_context(|| format!("reading registry '{}'", path.display()))?;
    let val: Value = serde_json::from_str(&content)
        .context("parsing registry file")?;
    let projects = val["projects"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|p| {
            let name = p["name"].as_str()?.to_string();
            let dir  = p["dir"].as_str()?.to_string();
            Some((name, dir))
        })
        .collect();
    Ok(projects)
}

fn write_registry(registry_dir: &Path, projects: &[(String, String)]) -> Result<()> {
    let path = registry_path(registry_dir);
    let arr: Vec<Value> = projects.iter()
        .map(|(n, d)| json!({"name": n, "dir": d}))
        .collect();
    let content = serde_json::to_string_pretty(&json!({"projects": arr}))?;
    fs::write(&path, content)
        .with_context(|| format!("writing registry '{}'", path.display()))
}

fn check_dir_writable(dir: &Path) -> bool {
    let test = dir.join(".write_check");
    match fs::write(&test, b"") {
        Ok(_) => { let _ = fs::remove_file(&test); true }
        Err(_) => false,
    }
}

fn cmd_init(registry_dir: &Path, name: &str, dir: &str) -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();
    ensure_registry_dir(registry_dir)?;
    let events = events_path(registry_dir);

    emit_event(&events, "ProjectInitRequested", &correlation_id, json!({
        "project_name": name,
        "project_dir": dir,
    }));

    // Contract failure: ProjectNameAlreadyExists
    let mut projects = read_registry(registry_dir)?;
    if projects.iter().any(|(n, _)| n == name) {
        eprintln!("Project '{}' already exists in registry.", name);
        emit_event(&events, "ProjectInitFailedDuplicate", &correlation_id, json!({
            "failure_reason": "project_name_already_exists",
            "project_name": name,
        }));
        return Ok(());
    }

    // Contract failure: DirectoryNotAccessible
    let project_path = Path::new(dir);
    if fs::create_dir_all(project_path).is_err() || !check_dir_writable(project_path) {
        eprintln!("Cannot create or write to project directory '{}'.", dir);
        emit_event(&events, "ProjectInitFailedDirectoryNotAccessible", &correlation_id, json!({
            "failure_reason": "directory_not_accessible",
            "project_name": name,
            "project_dir": dir,
        }));
        return Ok(());
    }

    // Create events subdirectory so other module binaries work immediately
    let _ = fs::create_dir_all(project_path.join("events"));

    let abs_dir = fs::canonicalize(project_path)
        .unwrap_or_else(|_| project_path.to_path_buf())
        .to_string_lossy()
        .into_owned();

    projects.push((name.to_string(), abs_dir.clone()));
    write_registry(registry_dir, &projects)?;

    emit_event(&events, "ProjectInitialized", &correlation_id, json!({
        "project_name": name,
        "project_dir": abs_dir,
    }));

    println!("Project '{}' created at '{}'.", name, abs_dir);
    Ok(())
}

fn cmd_list(registry_dir: &Path) -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();
    ensure_registry_dir(registry_dir)?;
    let events = events_path(registry_dir);

    emit_event(&events, "ProjectListRequested", &correlation_id, json!({}));

    let projects = read_registry(registry_dir)?;
    let count = projects.len() as u32;
    let projects_json: Vec<Value> = projects.iter()
        .map(|(n, d)| json!({"name": n, "dir": d}))
        .collect();

    if projects.is_empty() {
        println!("No projects registered.");
    } else {
        for (name, dir) in &projects {
            println!("{}: {}", name, dir);
        }
    }

    emit_event(&events, "ProjectListReturned", &correlation_id, json!({
        "project_count": count,
        "projects": projects_json,
    }));

    Ok(())
}

fn cmd_open(registry_dir: &Path, name: &str) -> Result<()> {
    let correlation_id = Uuid::new_v4().to_string();
    ensure_registry_dir(registry_dir)?;
    let events = events_path(registry_dir);

    emit_event(&events, "ProjectOpenRequested", &correlation_id, json!({
        "project_name": name,
    }));

    let projects = read_registry(registry_dir)?;

    // Contract failure: ProjectNotFound
    match projects.iter().find(|(n, _)| n == name) {
        None => {
            eprintln!("Project '{}' not found in registry.", name);
            emit_event(&events, "ProjectOpenFailedNotFound", &correlation_id, json!({
                "failure_reason": "project_not_found",
                "project_name": name,
            }));
        }
        Some((_, dir)) => {
            println!("{}", dir);
            emit_event(&events, "ProjectPathReturned", &correlation_id, json!({
                "project_name": name,
                "project_dir": dir,
            }));
        }
    }

    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let registry_dir = cli.registry
        .map(PathBuf::from)
        .unwrap_or_else(default_registry_dir);

    let result = match &cli.command {
        Commands::Init { name, dir } => cmd_init(&registry_dir, name, dir),
        Commands::List               => cmd_list(&registry_dir),
        Commands::Open { name }      => cmd_open(&registry_dir, name),
    };

    if let Err(e) = result {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}
