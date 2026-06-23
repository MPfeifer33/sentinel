use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::git;
use crate::model::{FileRisk, FileStats, FragilityMatrix, MatrixSummary, RelatedTest, RiskLevel};
use crate::SentinelError;

const RECENT_WINDOW: usize = 25;

pub fn build_matrix(repo: &Path, limit: usize) -> Result<FragilityMatrix, SentinelError> {
    let commits = git::history(repo, limit)?;
    let mut stats: BTreeMap<String, FileStats> = BTreeMap::new();

    for (index, commit) in commits.iter().enumerate() {
        let deltas = git::file_deltas(repo, &commit.sha)?;
        let churn_by_file: BTreeMap<String, usize> = deltas
            .into_iter()
            .map(|delta| (delta.path, delta.additions + delta.deletions))
            .collect();

        let files: Vec<String> = commit
            .files
            .iter()
            .filter(|path| should_track(path))
            .cloned()
            .collect();
        let tests: Vec<String> = files
            .iter()
            .filter(|path| is_test_file(path))
            .cloned()
            .collect();

        for path in &files {
            let entry = stats
                .entry(path.clone())
                .or_insert_with(|| FileStats::new(path.clone()));
            entry.commits += 1;
            if index < RECENT_WINDOW {
                entry.recent_commits += 1;
            }
            if entry.last_touched.is_none() {
                entry.last_touched = Some(commit.date.clone());
            }
            if looks_like_failure_work(&commit.subject) {
                entry.bugfix_commits += 1;
            }
            if looks_like_revert(&commit.subject) {
                entry.revert_commits += 1;
            }
            entry.total_churn += churn_by_file.get(path).copied().unwrap_or(0);

            if !is_test_file(path) && !tests.is_empty() {
                entry.test_cochanges += 1;
                for test in &tests {
                    *entry.related_tests.entry(test.clone()).or_insert(0) += 1;
                }
            }
        }
    }

    let mut files: Vec<FileRisk> = stats.into_values().map(score_file).collect();
    files.sort_by(|a, b| {
        b.risk_score
            .cmp(&a.risk_score)
            .then_with(|| b.commits.cmp(&a.commits))
            .then_with(|| a.path.cmp(&b.path))
    });

    let summary = summarize(&files);
    Ok(FragilityMatrix {
        generated_at_unix: now_unix(),
        repo: repo.display().to_string(),
        history_limit: limit,
        commits_scanned: commits.len(),
        files,
        summary,
    })
}

pub fn find_file<'a>(matrix: &'a FragilityMatrix, path: &str) -> Option<&'a FileRisk> {
    matrix.files.iter().find(|risk| risk.path == path)
}

pub fn synthetic_quiet_file(path: &str) -> FileRisk {
    FileRisk {
        path: path.to_string(),
        risk_score: 0,
        level: RiskLevel::Quiet,
        commits: 0,
        recent_commits: 0,
        bugfix_commits: 0,
        revert_commits: 0,
        test_cochanges: 0,
        total_churn: 0,
        last_touched: None,
        related_tests: Vec::new(),
        reasons: vec!["No historical signal in scanned commits".into()],
    }
}

fn score_file(stats: FileStats) -> FileRisk {
    let mut raw_score = 0usize;
    raw_score += stats.commits * 5;
    raw_score += stats.recent_commits * 10;
    raw_score += stats.bugfix_commits * 24;
    raw_score += stats.revert_commits * 32;
    raw_score += stats.test_cochanges * 14;
    raw_score += (stats.total_churn / 12).min(80);

    let risk_score = raw_score.min(100) as u32;
    let level = risk_level(risk_score);
    let reasons = reasons_for(&stats, risk_score);
    let related_tests = top_related_tests(stats.related_tests);

    FileRisk {
        path: stats.path,
        risk_score,
        level,
        commits: stats.commits,
        recent_commits: stats.recent_commits,
        bugfix_commits: stats.bugfix_commits,
        revert_commits: stats.revert_commits,
        test_cochanges: stats.test_cochanges,
        total_churn: stats.total_churn,
        last_touched: stats.last_touched,
        related_tests,
        reasons,
    }
}

fn top_related_tests(related_tests: BTreeMap<String, usize>) -> Vec<RelatedTest> {
    let mut tests: Vec<_> = related_tests
        .into_iter()
        .map(|(path, cochanges)| RelatedTest { path, cochanges })
        .collect();
    tests.sort_by(|a, b| {
        b.cochanges
            .cmp(&a.cochanges)
            .then_with(|| a.path.cmp(&b.path))
    });
    tests.truncate(8);
    tests
}

fn reasons_for(stats: &FileStats, risk_score: u32) -> Vec<String> {
    let mut reasons = Vec::new();

    if stats.bugfix_commits > 0 {
        reasons.push(format!(
            "{} failure-flavored commit(s) touched this file",
            stats.bugfix_commits
        ));
    }
    if stats.revert_commits > 0 {
        reasons.push(format!(
            "{} revert/rollback commit(s)",
            stats.revert_commits
        ));
    }
    if stats.test_cochanges > 0 {
        reasons.push(format!(
            "{} commit(s) co-changed tests with this file",
            stats.test_cochanges
        ));
    }
    if stats.recent_commits > 2 {
        reasons.push(format!(
            "{} recent touch(es) in the latest history window",
            stats.recent_commits
        ));
    }
    if stats.total_churn >= 200 {
        reasons.push(format!(
            "high churn: {} added/deleted lines",
            stats.total_churn
        ));
    }
    if reasons.is_empty() {
        if risk_score == 0 {
            reasons.push("No historical fragility signal in scanned commits".into());
        } else {
            reasons.push(format!("{} historical commit touch(es)", stats.commits));
        }
    }

    reasons
}

fn summarize(files: &[FileRisk]) -> MatrixSummary {
    let mut summary = MatrixSummary {
        tracked_files: files.len(),
        ..MatrixSummary::default()
    };

    for file in files {
        match file.level {
            RiskLevel::High => summary.high_risk += 1,
            RiskLevel::Medium => summary.medium_risk += 1,
            RiskLevel::Low => summary.low_risk += 1,
            RiskLevel::Quiet => summary.quiet += 1,
        }
    }

    summary
}

fn risk_level(score: u32) -> RiskLevel {
    match score {
        70..=100 => RiskLevel::High,
        40..=69 => RiskLevel::Medium,
        15..=39 => RiskLevel::Low,
        _ => RiskLevel::Quiet,
    }
}

fn should_track(path: &str) -> bool {
    if path.starts_with(".agent-") || path.starts_with("target/") || path.contains("/target/") {
        return false;
    }
    !path.trim().is_empty()
}

fn is_test_file(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.starts_with("tests/")
        || lower.contains("/tests/")
        || lower.contains("_test.")
        || lower.contains(".test.")
        || lower.contains("_spec.")
        || lower.contains(".spec.")
}

fn looks_like_failure_work(subject: &str) -> bool {
    let lower = subject.to_ascii_lowercase();
    let repair_or_failure = [
        "fix", "bug", "fail", "failure", "broken", "panic", "flake", "crash", "hotfix",
    ]
    .iter()
    .any(|needle| lower.contains(needle));

    repair_or_failure
        || (lower.contains("regression")
            && (lower.contains("repair")
                || lower.contains("prevent")
                || lower.contains("avoid")
                || lower.contains("caused")
                || lower.contains("in ")))
}

fn looks_like_revert(subject: &str) -> bool {
    let lower = subject.to_ascii_lowercase();
    ["revert", "rollback", "back out", "backout"]
        .iter()
        .any(|needle| lower.contains(needle))
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir as scratch_dir;

    #[test]
    fn scores_failure_and_test_cochange_highly() {
        let mut stats = FileStats::new("src/lib.rs".into());
        stats.commits = 3;
        stats.recent_commits = 2;
        stats.bugfix_commits = 1;
        stats.test_cochanges = 2;
        stats.total_churn = 120;
        stats.related_tests.insert("tests/lib.rs".into(), 2);

        let risk = score_file(stats);

        assert!(risk.risk_score >= 70);
        assert_eq!(risk.level, RiskLevel::High);
        assert_eq!(risk.related_tests[0].path, "tests/lib.rs");
    }

    #[test]
    fn quiet_file_gets_explanation() {
        let risk = synthetic_quiet_file("src/new.rs");

        assert_eq!(risk.level, RiskLevel::Quiet);
        assert_eq!(risk.risk_score, 0);
        assert!(!risk.reasons.is_empty());
    }

    #[test]
    fn failure_keywords_are_detected() {
        assert!(looks_like_failure_work("Fix regression in claims parser"));
        assert!(looks_like_revert("Rollback flaky change"));
        assert!(!looks_like_failure_work("Add happy path"));
        assert!(!looks_like_failure_work(
            "Add sentinel regression watcher MVP"
        ));
    }

    #[test]
    fn tracks_common_test_paths() {
        assert!(is_test_file("tests/cli.rs"));
        assert!(is_test_file("src/foo_test.rs"));
        assert!(is_test_file("web/button.spec.ts"));
        assert!(!is_test_file("src/main.rs"));
    }

    #[test]
    fn related_tests_are_ranked() {
        let mut tests = BTreeMap::new();
        tests.insert("tests/b.rs".into(), 1);
        tests.insert("tests/a.rs".into(), 3);

        let ranked = top_related_tests(tests);

        assert_eq!(ranked[0].path, "tests/a.rs");
        assert_eq!(ranked[0].cochanges, 3);
    }

    #[test]
    fn builds_matrix_from_git_history() {
        let workspace = scratch_dir().unwrap();
        init_repo(workspace.path());

        write_file(
            workspace.path().join("src/lib.rs"),
            "pub fn value() -> u8 { 1 }\n",
        );
        write_file(
            workspace.path().join("tests/lib.rs"),
            "#[test]\nfn value() {}\n",
        );
        git(workspace.path(), &["add", "."]);
        git(workspace.path(), &["commit", "-m", "initial"]);

        write_file(
            workspace.path().join("src/lib.rs"),
            "pub fn value() -> u8 { 2 }\n",
        );
        write_file(
            workspace.path().join("tests/lib.rs"),
            "#[test]\nfn value_regression() {}\n",
        );
        git(workspace.path(), &["add", "."]);
        git(
            workspace.path(),
            &["commit", "-m", "fix regression in value"],
        );

        let matrix = build_matrix(workspace.path(), 20).unwrap();
        let risk = find_file(&matrix, "src/lib.rs").unwrap();

        assert!(risk.bugfix_commits >= 1);
        assert!(risk.test_cochanges >= 1);
        assert_eq!(risk.related_tests[0].path, "tests/lib.rs");
    }

    #[test]
    fn empty_repo_builds_empty_matrix() {
        let workspace = scratch_dir().unwrap();
        init_repo(workspace.path());

        let matrix = build_matrix(workspace.path(), 20).unwrap();

        assert_eq!(matrix.commits_scanned, 0);
        assert_eq!(matrix.files.len(), 0);
    }

    fn init_repo(path: &std::path::Path) {
        git(path, &["init"]);
        git(path, &["config", "user.email", "sentinel@example.test"]);
        git(path, &["config", "user.name", "Sentinel Test"]);
    }

    fn git(path: &std::path::Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn write_file(path: std::path::PathBuf, contents: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }
}
