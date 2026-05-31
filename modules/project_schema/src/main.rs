use clap::{Parser, Subcommand};
use project_schema::{emit_schema_failure, load_schema, validate};
use std::path::PathBuf;
use uuid::Uuid;

const EVENTS_FILE: &str = "events/runtime_events.jsonl";

#[derive(Parser)]
#[command(about = "LucidPM project vocabulary management")]
struct Cli {
    /// Project directory (defaults to current directory)
    #[arg(long, default_value = ".")]
    project_dir: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate the project vocabulary and report any errors
    Validate,
    /// Print the resolved vocabulary (project + default merged)
    Show,
}

fn main() {
    let cli = Cli::parse();
    let correlation_id = Uuid::new_v4().to_string();
    let events_file = cli.project_dir.join(EVENTS_FILE);

    let schema = match load_schema(&cli.project_dir) {
        Err(e) => {
            emit_schema_failure(&events_file, &e, &correlation_id);
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
        Ok(s) => s,
    };

    if let Err(e) = validate(&schema) {
        emit_schema_failure(&events_file, &e, &correlation_id);
        eprintln!("error: {}", e);
        std::process::exit(1);
    }

    match cli.command {
        Commands::Validate => {
            let mut page_types: Vec<&str> = schema.page_types.keys().map(|s| s.as_str()).collect();
            page_types.sort_unstable();
            let mut block_types: Vec<&str> = schema.block_types.keys().map(|s| s.as_str()).collect();
            block_types.sort_unstable();
            let mut relations: Vec<&str> = schema.relations.keys().map(|s| s.as_str()).collect();
            relations.sort_unstable();
            let mut statuses: Vec<&str> = schema.statuses.keys().map(|s| s.as_str()).collect();
            statuses.sort_unstable();

            println!("Schema OK (schemaVersion: {})", schema.schema_version);
            println!("  Page types:  {}", page_types.join(", "));
            println!("  Block types: {}", block_types.join(", "));
            println!("  Relations:   {}", relations.join(", "));
            println!("  Statuses:    {}", statuses.join(", "));
        }
        Commands::Show => match serde_yaml::to_string(&schema) {
            Ok(yaml) => print!("{}", yaml),
            Err(e) => {
                eprintln!("error: could not serialize schema: {}", e);
                std::process::exit(1);
            }
        },
    }
}
