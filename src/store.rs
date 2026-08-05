use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::model::{FragilityMatrix, MatrixHealth};
use crate::SentinelError;

const STORE_DIR: &str = ".agent-sentinel";
const MATRIX_FILE: &str = "matrix.json";
const LOCAL_EXCLUDE_ENTRY: &str = ".agent-sentinel/";

#[derive(Debug, Serialize)]
pub struct StoreStatus {
    pub store_dir: String,
    pub matrix_path: String,
    pub matrix_exists: bool,
    pub matrix_health: Option<MatrixHealth>,
    pub matrix_error: Option<String>,
}

pub fn has_matrix(repo: &Path) -> bool {
    matrix_path(repo).exists()
}

pub fn load(repo: &Path) -> Result<Option<FragilityMatrix>, SentinelError> {
    let path = matrix_path(repo);
    if !path.exists() {
        return Ok(None);
    }

    let text = fs::read_to_string(path)?;
    Ok(Some(serde_json::from_str(&text)?))
}

pub fn save(repo: &Path, matrix: &FragilityMatrix) -> Result<(), SentinelError> {
    ensure_local_cache_excluded(repo)?;

    let dir = store_dir(repo);
    fs::create_dir_all(&dir)?;
    let text = serde_json::to_string_pretty(matrix)?;
    fs::write(matrix_path(repo), text)?;
    Ok(())
}

pub fn ensure_local_cache_excluded(repo: &Path) -> Result<(), SentinelError> {
    let Some(exclude_path) = git_exclude_path(repo)? else {
        return Ok(());
    };

    if let Some(parent) = exclude_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let contents = match fs::read_to_string(&exclude_path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err.into()),
    };
    if contents
        .lines()
        .any(|line| line.trim() == LOCAL_EXCLUDE_ENTRY)
    {
        return Ok(());
    }

    let mut updated = contents;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(LOCAL_EXCLUDE_ENTRY);
    updated.push('\n');
    fs::write(exclude_path, updated)?;
    Ok(())
}

pub fn status(repo: &Path) -> StoreStatus {
    let (matrix_health, matrix_error) = match load(repo) {
        Ok(Some(matrix)) => match crate::analyze::matrix_health(&matrix, repo) {
            Ok(health) => (Some(health), None),
            Err(err) => (None, Some(err.to_string())),
        },
        Ok(None) => (None, None),
        Err(err) => (None, Some(err.to_string())),
    };

    StoreStatus {
        store_dir: store_dir(repo).display().to_string(),
        matrix_path: matrix_path(repo).display().to_string(),
        matrix_exists: has_matrix(repo),
        matrix_health,
        matrix_error,
    }
}

fn store_dir(repo: &Path) -> PathBuf {
    repo.join(STORE_DIR)
}

fn matrix_path(repo: &Path) -> PathBuf {
    store_dir(repo).join(MATRIX_FILE)
}

fn git_exclude_path(repo: &Path) -> Result<Option<PathBuf>, SentinelError> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-path", "info/exclude"])
        .current_dir(repo)
        .output()?;

    if !output.status.success() {
        return Ok(None);
    }

    let raw_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if raw_path.is_empty() {
        return Ok(None);
    }

    let path = PathBuf::from(raw_path);
    if path.is_absolute() {
        Ok(Some(path))
    } else {
        Ok(Some(repo.join(path)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MatrixSummary;

    #[test]
    fn saving_matrix_excludes_local_cache_directory() {
        let workspace = tempfile::tempdir().unwrap();
        init_git_repo(workspace.path());

        save(workspace.path(), &empty_matrix(workspace.path())).unwrap();

        let exclude = fs::read_to_string(workspace.path().join(".git/info/exclude")).unwrap();
        assert!(exclude
            .lines()
            .any(|line| line.trim() == LOCAL_EXCLUDE_ENTRY));
    }

    #[test]
    fn saving_matrix_does_not_duplicate_local_exclude_entry() {
        let workspace = tempfile::tempdir().unwrap();
        init_git_repo(workspace.path());

        save(workspace.path(), &empty_matrix(workspace.path())).unwrap();
        save(workspace.path(), &empty_matrix(workspace.path())).unwrap();

        let exclude = fs::read_to_string(workspace.path().join(".git/info/exclude")).unwrap();
        let entries = exclude
            .lines()
            .filter(|line| line.trim() == LOCAL_EXCLUDE_ENTRY)
            .count();
        assert_eq!(entries, 1);
    }

    fn init_git_repo(repo: &Path) {
        let output = Command::new("git")
            .args(["init"])
            .current_dir(repo)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git init failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn empty_matrix(repo: &Path) -> FragilityMatrix {
        FragilityMatrix {
            generated_at_unix: 0,
            repo: repo.display().to_string(),
            history_limit: 100,
            commits_scanned: 0,
            head_sha: None,
            dirty_at_scan: false,
            files: Vec::new(),
            summary: MatrixSummary::default(),
        }
    }
}
