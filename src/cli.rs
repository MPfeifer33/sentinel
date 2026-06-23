use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::SentinelError;

#[derive(Parser, Debug)]
#[command(
    name = "sentinel",
    version,
    about = "Regression risk watcher for agents"
)]
pub struct Cli {
    /// Project root override
    #[arg(long, global = true)]
    pub repo: Option<PathBuf>,

    /// Output format
    #[arg(long, global = true, default_value = "text")]
    pub format: OutputFormat,

    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn resolve_repo(&self) -> Result<PathBuf, SentinelError> {
        if let Some(ref repo) = self.repo {
            return Ok(repo.clone());
        }
        if let Ok(output) = std::process::Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                return Ok(PathBuf::from(path));
            }
        }
        std::env::current_dir().map_err(SentinelError::Io)
    }

    pub fn is_json(&self) -> bool {
        matches!(self.format, OutputFormat::Json)
    }
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum OutputFormat {
    Json,
    Text,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Build or refresh the fragility matrix from git history
    Scan {
        /// Number of commits to inspect
        #[arg(long, default_value = "200")]
        limit: usize,
        /// Rebuild even if a stored matrix already exists
        #[arg(long)]
        force: bool,
    },
    /// Report risk for explicit files or current changed files
    Risk {
        /// File path to inspect; repeat for multiple paths
        #[arg(long)]
        file: Vec<String>,
        /// Inspect files changed relative to HEAD
        #[arg(long)]
        changed: bool,
        /// Number of commits to inspect if no matrix exists
        #[arg(long, default_value = "200")]
        limit: usize,
    },
    /// Show the highest-risk files in the matrix
    Matrix {
        /// Number of rows to print
        #[arg(long, default_value = "20")]
        top: usize,
        /// Number of commits to inspect if no matrix exists
        #[arg(long, default_value = "200")]
        limit: usize,
    },
    /// Show tests historically co-changed with a file
    Tests {
        /// File path to inspect
        file: String,
        /// Number of commits to inspect if no matrix exists
        #[arg(long, default_value = "200")]
        limit: usize,
    },
    /// Show stored matrix status and data sources
    Status,
}
