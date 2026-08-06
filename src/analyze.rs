use std::collections::BTreeMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::git;
use crate::model::{
    CoverageStatus, FileRisk, FileStats, FragilityMatrix, MatrixConfidence, MatrixHealth,
    MatrixSummary, RelatedTest, RiskLevel, ScoreBreakdown,
};
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
    let changed_files = git::changed_files(repo)?;
    Ok(FragilityMatrix {
        generated_at_unix: now_unix(),
        repo: repo.display().to_string(),
        history_limit: limit,
        commits_scanned: commits.len(),
        head_sha: git::head_sha(repo)?,
        dirty_at_scan: !changed_files.is_empty(),
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
        known_in_matrix: false,
        coverage_status: CoverageStatus::Unknown,
        risk_score: 0,
        score_breakdown: ScoreBreakdown::default(),
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
    let score_breakdown = score_breakdown_for_stats(&stats);
    let risk_score = score_breakdown.total;
    let level = risk_level(risk_score);
    let reasons = reasons_for(&stats, risk_score);
    let related_tests = top_related_tests(stats.related_tests);

    FileRisk {
        path: stats.path,
        known_in_matrix: true,
        coverage_status: CoverageStatus::Known,
        risk_score,
        score_breakdown,
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

pub fn normalize_agent_fields(mut risk: FileRisk) -> FileRisk {
    risk.coverage_status = if risk.known_in_matrix {
        CoverageStatus::Known
    } else {
        CoverageStatus::Unknown
    };
    risk.score_breakdown = score_breakdown_for_risk(&risk);
    risk
}

fn score_breakdown_for_stats(stats: &FileStats) -> ScoreBreakdown {
    let commits = (stats.commits * 5) as u32;
    let recent = (stats.recent_commits * 10) as u32;
    let failure_like = (stats.bugfix_commits * 24) as u32;
    let revert = (stats.revert_commits * 32) as u32;
    let test_cochange = (stats.test_cochanges * 14) as u32;
    let churn = (stats.total_churn / 12).min(80) as u32;
    let total = (commits + recent + failure_like + revert + test_cochange + churn).min(100);

    ScoreBreakdown {
        commits,
        recent,
        failure_like,
        revert,
        test_cochange,
        churn,
        total,
    }
}

fn score_breakdown_for_risk(risk: &FileRisk) -> ScoreBreakdown {
    let commits = (risk.commits * 5) as u32;
    let recent = (risk.recent_commits * 10) as u32;
    let failure_like = (risk.bugfix_commits * 24) as u32;
    let revert = (risk.revert_commits * 32) as u32;
    let test_cochange = (risk.test_cochanges * 14) as u32;
    let churn = (risk.total_churn / 12).min(80) as u32;
    let total = (commits + recent + failure_like + revert + test_cochange + churn).min(100);

    ScoreBreakdown {
        commits,
        recent,
        failure_like,
        revert,
        test_cochange,
        churn,
        total,
    }
}

pub fn matrix_health(matrix: &FragilityMatrix, repo: &Path) -> Result<MatrixHealth, SentinelError> {
    let current_head_sha = git::head_sha(repo)?;
    let changed_files = git::changed_files(repo)?;
    let dirty_now = !changed_files.is_empty();
    let head_matches = matrix.head_sha == current_head_sha;
    let stale = !head_matches;
    let mut warnings = Vec::new();

    if stale {
        warnings.push(
            "matrix was generated from a different git HEAD; run `sentinel scan --force`".into(),
        );
    }
    if matrix.head_sha.is_none() {
        warnings
            .push("matrix does not record a git HEAD; rescan to capture freshness metadata".into());
    }
    if matrix.commits_scanned < 10 {
        warnings.push(format!(
            "thin history: only {} commit(s) scanned",
            matrix.commits_scanned
        ));
    }
    if matrix.dirty_at_scan {
        warnings.push("matrix was generated while the worktree had changed files".into());
    }
    if dirty_now {
        warnings.push(
            "worktree currently has changed or untracked files; risk rows are historical hints"
                .into(),
        );
    }

    let confidence = if stale || matrix.head_sha.is_none() || matrix.commits_scanned < 10 {
        MatrixConfidence::Low
    } else if matrix.commits_scanned < 50 || matrix.dirty_at_scan {
        MatrixConfidence::Medium
    } else {
        MatrixConfidence::High
    };

    Ok(MatrixHealth {
        repo: matrix.repo.clone(),
        generated_at_unix: matrix.generated_at_unix,
        history_limit: matrix.history_limit,
        commits_scanned: matrix.commits_scanned,
        tracked_files: matrix.summary.tracked_files,
        matrix_head_sha: matrix.head_sha.clone(),
        current_head_sha,
        head_matches,
        dirty_at_scan: matrix.dirty_at_scan,
        dirty_now,
        changed_files_count: changed_files.len(),
        stale,
        confidence,
        warnings,
    })
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
        assert!(!risk.known_in_matrix);
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

    #[test]
    fn matrix_health_detects_stale_head_and_thin_history() {
        let workspace = scratch_dir().unwrap();
        init_repo(workspace.path());

        write_file(workspace.path().join("src/lib.rs"), "pub fn value() {}\n");
        git(workspace.path(), &["add", "."]);
        git(workspace.path(), &["commit", "-m", "initial"]);

        let matrix = build_matrix(workspace.path(), 20).unwrap();
        let fresh = matrix_health(&matrix, workspace.path()).unwrap();
        assert!(!fresh.stale);
        assert!(fresh.head_matches);
        assert_eq!(fresh.confidence, MatrixConfidence::Low);
        assert!(fresh
            .warnings
            .iter()
            .any(|warning| warning.contains("thin history")));

        write_file(
            workspace.path().join("src/lib.rs"),
            "pub fn value() -> u8 { 2 }\n",
        );
        git(workspace.path(), &["add", "."]);
        git(workspace.path(), &["commit", "-m", "change value"]);

        let stale = matrix_health(&matrix, workspace.path()).unwrap();
        assert!(stale.stale);
        assert!(!stale.head_matches);
        assert!(stale
            .warnings
            .iter()
            .any(|warning| warning.contains("different git HEAD")));
    }

    #[test]
    fn matrix_health_lowers_confidence_without_head_metadata() {
        let workspace = scratch_dir().unwrap();
        let matrix = FragilityMatrix {
            generated_at_unix: 1,
            repo: workspace.path().display().to_string(),
            history_limit: 200,
            commits_scanned: 100,
            head_sha: None,
            dirty_at_scan: false,
            files: Vec::new(),
            summary: MatrixSummary::default(),
        };

        let health = matrix_health(&matrix, workspace.path()).unwrap();

        assert_eq!(health.confidence, MatrixConfidence::Low);
        assert!(!health.stale);
        assert!(health
            .warnings
            .iter()
            .any(|warning| warning.contains("does not record a git HEAD")));
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
