use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::model::FragilityMatrix;
use crate::SentinelError;

const STORE_DIR: &str = ".agent-sentinel";
const MATRIX_FILE: &str = "matrix.json";

#[derive(Debug, Serialize)]
pub struct StoreStatus {
    pub store_dir: String,
    pub matrix_path: String,
    pub matrix_exists: bool,
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
    let dir = store_dir(repo);
    fs::create_dir_all(&dir)?;
    let text = serde_json::to_string_pretty(matrix)?;
    fs::write(matrix_path(repo), text)?;
    Ok(())
}

pub fn status(repo: &Path) -> StoreStatus {
    StoreStatus {
        store_dir: store_dir(repo).display().to_string(),
        matrix_path: matrix_path(repo).display().to_string(),
        matrix_exists: has_matrix(repo),
    }
}

fn store_dir(repo: &Path) -> PathBuf {
    repo.join(STORE_DIR)
}

fn matrix_path(repo: &Path) -> PathBuf {
    store_dir(repo).join(MATRIX_FILE)
}
