use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use crate::SentinelError;

#[derive(Debug, Clone)]
pub struct CommitRecord {
    pub sha: String,
    pub date: String,
    pub subject: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FileDelta {
    pub path: String,
    pub additions: usize,
    pub deletions: usize,
}

pub fn history(repo: &Path, limit: usize) -> Result<Vec<CommitRecord>, SentinelError> {
    let output = Command::new("git")
        .args([
            "log",
            &format!("-{limit}"),
            "--name-only",
            "--format=__SENTINEL_COMMIT__%H%x1f%ai%x1f%s",
        ])
        .current_dir(repo)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("does not have any commits yet")
            || stderr.contains("your current branch")
            || stderr.contains("No commits yet")
        {
            return Ok(Vec::new());
        }
        return Err(SentinelError::Validation(
            "failed to read git history; is this a git repository?".into(),
        ));
    }

    Ok(parse_history(&String::from_utf8_lossy(&output.stdout)))
}

pub fn file_deltas(repo: &Path, sha: &str) -> Result<Vec<FileDelta>, SentinelError> {
    let output = Command::new("git")
        .args(["diff", "--numstat", &format!("{sha}~1..{sha}")])
        .current_dir(repo)
        .output()?;

    if output.status.success() {
        return Ok(parse_numstat(&String::from_utf8_lossy(&output.stdout)));
    }

    let output = Command::new("git")
        .args(["diff", "--numstat", "--root", sha])
        .current_dir(repo)
        .output()?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    Ok(parse_numstat(&String::from_utf8_lossy(&output.stdout)))
}

pub fn changed_files(repo: &Path) -> Result<Vec<String>, SentinelError> {
    let mut files = BTreeSet::new();

    let output = Command::new("git")
        .args(["diff", "--name-only", "HEAD"])
        .current_dir(repo)
        .output()?;

    if output.status.success() {
        files.extend(parse_path_lines(&String::from_utf8_lossy(&output.stdout)));
    }

    let output = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(repo)
        .output()?;

    if output.status.success() {
        files.extend(parse_path_lines(&String::from_utf8_lossy(&output.stdout)));
    }

    Ok(files.into_iter().collect())
}

fn parse_history(text: &str) -> Vec<CommitRecord> {
    let mut commits = Vec::new();
    let mut current: Option<CommitRecord> = None;

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = line.strip_prefix("__SENTINEL_COMMIT__") {
            if let Some(commit) = current.take() {
                commits.push(commit);
            }
            let parts: Vec<&str> = rest.splitn(3, '\x1f').collect();
            if parts.len() == 3 {
                current = Some(CommitRecord {
                    sha: parts[0].to_string(),
                    date: parts[1].to_string(),
                    subject: parts[2].to_string(),
                    files: Vec::new(),
                });
            }
            continue;
        }

        if let Some(commit) = current.as_mut() {
            commit.files.push(line.to_string());
        }
    }

    if let Some(commit) = current {
        commits.push(commit);
    }

    commits
}

fn parse_numstat(text: &str) -> Vec<FileDelta> {
    text.lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 3 {
                return None;
            }

            Some(FileDelta {
                additions: parts[0].parse().unwrap_or(0),
                deletions: parts[1].parse().unwrap_or(0),
                path: parts[2].to_string(),
            })
        })
        .collect()
}

fn parse_path_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter(|line| is_user_changed_path(line))
        .map(ToOwned::to_owned)
        .collect()
}

fn is_user_changed_path(path: &str) -> bool {
    !(path.starts_with(".agent-")
        || path.contains("/.agent-")
        || path == "target"
        || path.starts_with("target/")
        || path.contains("/target/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_history_records() {
        let text = "__SENTINEL_COMMIT__abc\x1f2026-06-22 10:00:00 +0000\x1ffix regression\nsrc/lib.rs\ntests/lib.rs\n\n__SENTINEL_COMMIT__def\x1f2026-06-21 10:00:00 +0000\x1fadd feature\nsrc/main.rs\n";

        let commits = parse_history(text);

        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].sha, "abc");
        assert_eq!(commits[0].subject, "fix regression");
        assert_eq!(commits[0].files, vec!["src/lib.rs", "tests/lib.rs"]);
    }

    #[test]
    fn changed_paths_ignore_agent_and_build_artifacts() {
        let paths = parse_path_lines(
            ".agent-sentinel/matrix.json\nsrc/lib.rs\ntarget/debug/sentinel\nnested/.agent-cache/file\n",
        );

        assert_eq!(paths, vec!["src/lib.rs"]);
    }
}
