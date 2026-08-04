use crate::analyze;
use crate::model::{FileRisk, FragilityMatrix, MatrixHealth, RelatedTest, RiskLevel};
use crate::store::StoreStatus;
use crate::SentinelError;

pub fn print_scan(
    matrix: &FragilityMatrix,
    health: &MatrixHealth,
    is_json: bool,
) -> Result<(), SentinelError> {
    if is_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "matrix": matrix,
                "matrix_health": health,
            }))?
        );
    } else {
        println!(
            "sentinel scan: {} commits, {} tracked file(s)",
            matrix.commits_scanned, matrix.summary.tracked_files
        );
        println!();
        print_health_warnings(health);
        println!();
        print_summary(matrix);
        println!();
        print_top_files(matrix, 10);
        println!();
        println!("  Matrix saved to .agent-sentinel/matrix.json");
    }
    Ok(())
}

pub fn print_matrix(
    matrix: &FragilityMatrix,
    health: &MatrixHealth,
    top: usize,
    is_json: bool,
) -> Result<(), SentinelError> {
    let files: Vec<&FileRisk> = matrix.files.iter().take(top).collect();

    if is_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "summary": matrix.summary,
                "matrix_health": health,
                "files": files,
            }))?
        );
    } else {
        println!(
            "sentinel matrix: top {} of {} tracked file(s)",
            files.len(),
            matrix.summary.tracked_files
        );
        println!();
        print_health_warnings(health);
        println!();
        print_summary(matrix);
        println!();
        print_file_rows(&files);
    }
    Ok(())
}

pub fn print_risk(
    matrix: &FragilityMatrix,
    health: &MatrixHealth,
    files: &[String],
    is_json: bool,
) -> Result<(), SentinelError> {
    let risks = risks_for(matrix, files);

    if is_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "matrix_health": health,
                "files": risks,
                "advice": advice_for(&risks),
            }))?
        );
    } else {
        if files.is_empty() {
            println!("sentinel risk: no changed files detected");
            println!();
            print_health_warnings(health);
            return Ok(());
        }

        println!("sentinel risk: {} file(s)", risks.len());
        println!();
        print_health_warnings(health);
        println!();
        for risk in &risks {
            print_file_detail(risk);
            println!();
        }
        println!("  Advice: {}", advice_for(&risks));
    }
    Ok(())
}

pub fn print_doctor(
    matrix: &FragilityMatrix,
    health: &MatrixHealth,
    changed_files: &[String],
    is_json: bool,
) -> Result<(), SentinelError> {
    let risks = risks_for(matrix, changed_files);
    let recommendations = recommendations_for(health, &risks);

    if is_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "matrix_health": health,
                "changed_files": changed_files,
                "changed_file_risks": risks,
                "advice": advice_for(&risks),
                "recommendations": recommendations,
            }))?
        );
    } else {
        println!("sentinel doctor:");
        println!();
        println!("  Matrix confidence: {}", health.confidence.label());
        println!("  Matrix stale: {}", health.stale);
        println!("  Commits scanned: {}", health.commits_scanned);
        println!("  Tracked files: {}", health.tracked_files);
        println!("  Changed files: {}", changed_files.len());
        println!();
        print_health_warnings(health);
        println!();

        if risks.is_empty() {
            println!("  No changed files detected.");
        } else {
            println!("  Changed-file risk:");
            for risk in &risks {
                println!(
                    "    [{:<6} {:>3}] {}",
                    risk.level.label(),
                    risk.risk_score,
                    risk.path
                );
                if !risk.known_in_matrix {
                    println!("      unknown to matrix");
                }
            }
        }
        println!();
        println!("  Advice: {}", advice_for(&risks));
        println!();
        println!("  Recommended next commands:");
        for recommendation in recommendations {
            println!("    {recommendation}");
        }
    }
    Ok(())
}

pub fn print_tests(
    matrix: &FragilityMatrix,
    health: &MatrixHealth,
    file: &str,
    is_json: bool,
) -> Result<(), SentinelError> {
    let risk = analyze::find_file(matrix, file)
        .cloned()
        .unwrap_or_else(|| analyze::synthetic_quiet_file(file));

    if is_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "file": file,
                "matrix_health": health,
                "related_tests": risk.related_tests,
            }))?
        );
    } else {
        println!("sentinel tests: {file}");
        println!();
        print_health_warnings(health);
        println!();
        print_related_tests(&risk.related_tests);
    }
    Ok(())
}

pub fn print_status(status: &StoreStatus, is_json: bool) -> Result<(), SentinelError> {
    if is_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "status": status,
            }))?
        );
    } else {
        println!("sentinel status:");
        println!();
        println!("  Store: {}", status.store_dir);
        println!("  Matrix: {}", status.matrix_path);
        println!("  Matrix exists: {}", status.matrix_exists);
        if let Some(health) = &status.matrix_health {
            println!("  Matrix confidence: {}", health.confidence.label());
            println!("  Matrix stale: {}", health.stale);
            println!("  Commits scanned: {}", health.commits_scanned);
            println!("  Tracked files: {}", health.tracked_files);
            println!("  Changed files now: {}", health.changed_files_count);
            println!();
            print_health_warnings(health);
        } else if let Some(error) = &status.matrix_error {
            println!("  Matrix error: {error}");
        }
        println!();
        println!("  Sources:");
        println!("    git log       — commit frequency, recency, subjects");
        println!("    git numstat   — churn per file");
        println!("    test cochange — source files touched with tests in same commit");
    }
    Ok(())
}

fn print_summary(matrix: &FragilityMatrix) {
    println!("  Risk bands:");
    println!("    high:   {}", matrix.summary.high_risk);
    println!("    medium: {}", matrix.summary.medium_risk);
    println!("    low:    {}", matrix.summary.low_risk);
    println!("    quiet:  {}", matrix.summary.quiet);
}

fn print_health_warnings(health: &MatrixHealth) {
    if health.warnings.is_empty() {
        println!("  Matrix health: {} confidence", health.confidence.label());
        return;
    }

    println!("  Matrix health: {} confidence", health.confidence.label());
    for warning in &health.warnings {
        println!("    warning: {warning}");
    }
}

fn print_top_files(matrix: &FragilityMatrix, top: usize) {
    let files: Vec<&FileRisk> = matrix.files.iter().take(top).collect();
    print_file_rows(&files);
}

fn print_file_rows(files: &[&FileRisk]) {
    if files.is_empty() {
        println!("  No files in matrix.");
        return;
    }

    println!("  Files:");
    for risk in files {
        println!(
            "    [{:<6} {:>3}] {}",
            risk.level.label(),
            risk.risk_score,
            risk.path
        );
        if let Some(reason) = risk.reasons.first() {
            println!("      {reason}");
        }
    }
}

fn print_file_detail(risk: &FileRisk) {
    println!(
        "  [{:<6} {:>3}] {}",
        risk.level.label(),
        risk.risk_score,
        risk.path
    );
    if !risk.known_in_matrix {
        println!("    coverage: unknown to matrix; treat as new/unsampled file");
    }
    println!(
        "    history: {} commit(s), {} recent, {} churn",
        risk.commits, risk.recent_commits, risk.total_churn
    );
    if risk.bugfix_commits > 0 || risk.revert_commits > 0 || risk.test_cochanges > 0 {
        println!(
            "    signals: {} failure-like, {} revert, {} test cochange",
            risk.bugfix_commits, risk.revert_commits, risk.test_cochanges
        );
    }
    for reason in &risk.reasons {
        println!("    reason: {reason}");
    }
    if !risk.related_tests.is_empty() {
        println!("    related tests:");
        for test in &risk.related_tests {
            println!("      {} ({} cochange)", test.path, test.cochanges);
        }
    }
}

fn print_related_tests(tests: &[RelatedTest]) {
    if tests.is_empty() {
        println!("  No historically co-changed tests found.");
    } else {
        for test in tests {
            println!("  {} ({} cochange)", test.path, test.cochanges);
        }
    }
}

fn risks_for(matrix: &FragilityMatrix, files: &[String]) -> Vec<FileRisk> {
    files
        .iter()
        .map(|file| {
            analyze::find_file(matrix, file)
                .cloned()
                .unwrap_or_else(|| analyze::synthetic_quiet_file(file))
        })
        .collect()
}

fn advice_for(risks: &[FileRisk]) -> String {
    if risks.iter().any(|risk| risk.level == RiskLevel::High) {
        "high-risk file present; run targeted tests first, then full validation before commit"
            .into()
    } else if risks.iter().any(|risk| risk.level == RiskLevel::Medium) {
        "medium risk; run related tests and consider full validation if behavior changed".into()
    } else if risks.iter().any(|risk| !risk.known_in_matrix) {
        "unknown file history; use normal validation and consider adding focused tests".into()
    } else if risks.is_empty() {
        "no changed files detected".into()
    } else {
        "low historical risk; use normal validation for the project".into()
    }
}

fn recommendations_for(health: &MatrixHealth, risks: &[FileRisk]) -> Vec<String> {
    let mut recommendations = Vec::new();

    if health.stale || health.matrix_head_sha.is_none() {
        recommendations.push("sentinel scan --force".to_string());
    }
    if risks.iter().any(|risk| !risk.known_in_matrix) {
        recommendations.push("add or run focused validation for unknown files".to_string());
    }
    if risks.iter().any(|risk| risk.level == RiskLevel::High) {
        recommendations.push("run related targeted tests before full validation".to_string());
        recommendations.push("run full project validation before commit".to_string());
    } else if risks.iter().any(|risk| risk.level == RiskLevel::Medium) {
        recommendations.push("run related tests for medium-risk files".to_string());
    }
    if recommendations.is_empty() {
        recommendations.push("normal project validation is sufficient".to_string());
    }

    recommendations
}
