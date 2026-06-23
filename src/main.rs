mod analyze;
mod cli;
mod git;
mod model;
mod report;
mod store;

use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    let result = run(&cli);
    match result {
        Ok(()) => {}
        Err(e) => {
            let code = e.exit_code();
            if cli.is_json() {
                let err_json = serde_json::json!({
                    "ok": false,
                    "error": {
                        "code": e.error_code(),
                        "message": e.to_string(),
                    }
                });
                eprintln!("{}", serde_json::to_string_pretty(&err_json).unwrap_or_else(|_| format!("{{\"ok\":false,\"error\":{{\"message\":\"{e}\"}}}}")));
            } else {
                eprintln!("error: {e}");
            }
            std::process::exit(code);
        }
    }
}

fn run(cli: &Cli) -> Result<(), SentinelError> {
    let repo = cli.resolve_repo()?;

    match &cli.command {
        Command::Scan { limit, force } => {
            if !force && store::has_matrix(&repo) {
                let matrix = store::load(&repo)?
                    .ok_or_else(|| SentinelError::NotFound("stored matrix".into()))?;
                report::print_scan(&matrix, cli.is_json())?;
                return Ok(());
            }

            let matrix = analyze::build_matrix(&repo, *limit)?;
            store::save(&repo, &matrix)?;
            report::print_scan(&matrix, cli.is_json())
        }
        Command::Risk {
            file,
            changed,
            limit,
        } => {
            let matrix = load_or_scan(&repo, *limit)?;
            let files = if !file.is_empty() {
                file.clone()
            } else if *changed || file.is_empty() {
                git::changed_files(&repo)?
            } else {
                Vec::new()
            };

            report::print_risk(&matrix, &files, cli.is_json())
        }
        Command::Matrix { top, limit } => {
            let matrix = load_or_scan(&repo, *limit)?;
            report::print_matrix(&matrix, *top, cli.is_json())
        }
        Command::Tests { file, limit } => {
            let matrix = load_or_scan(&repo, *limit)?;
            report::print_tests(&matrix, file, cli.is_json())
        }
        Command::Status => {
            let status = store::status(&repo);
            report::print_status(&status, cli.is_json())
        }
    }
}

fn load_or_scan(
    repo: &std::path::Path,
    limit: usize,
) -> Result<model::FragilityMatrix, SentinelError> {
    if let Some(matrix) = store::load(repo)? {
        Ok(matrix)
    } else {
        let matrix = analyze::build_matrix(repo, limit)?;
        store::save(repo, &matrix)?;
        Ok(matrix)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SentinelError {
    #[error("{0}")]
    Validation(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl SentinelError {
    pub fn exit_code(&self) -> i32 {
        match self {
            SentinelError::Validation(_) => 1,
            SentinelError::NotFound(_) => 3,
            SentinelError::Io(_) => 2,
            SentinelError::Json(_) => 1,
        }
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            SentinelError::Validation(_) => "validation_error",
            SentinelError::NotFound(_) => "not_found",
            SentinelError::Io(_) => "io_error",
            SentinelError::Json(_) => "json_error",
        }
    }
}
