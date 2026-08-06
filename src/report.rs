use crate::analyze;
use crate::model::{
    CoverageStatus, FileRisk, FragilityMatrix, MatrixConfidence, MatrixHealth, RelatedTest,
    RiskLevel,
};
use crate::store::StoreStatus;
use crate::SentinelError;

const SCORING_VERSION: &str = "sentinel.git-history.v1";
const SCAN_SCHEMA_VERSION: &str = "sentinel.scan.v1";
const RISK_SCHEMA_VERSION: &str = "sentinel.risk.v1";
const MATRIX_SCHEMA_VERSION: &str = "sentinel.matrix.v1";
const TESTS_SCHEMA_VERSION: &str = "sentinel.tests.v1";
const STATUS_SCHEMA_VERSION: &str = "sentinel.status.v1";
const DOCTOR_SCHEMA_VERSION: &str = "sentinel.doctor.v1";

#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentDoctor {
    pub schema_version: String,
    pub scoring_version: String,
    pub status: DoctorStatus,
    pub action_level: ActionLevel,
    pub gates: AgentGates,
    pub matrix_health: MatrixHealth,
    pub changed_files: Vec<String>,
    pub changed_file_risks: Vec<FileRisk>,
    pub advice: String,
    pub recommendations: Vec<String>,
    pub recommended_commands: Vec<RecommendedCommand>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorStatus {
    Ready,
    Caution,
    Blocked,
}

impl DoctorStatus {
    fn label(self) -> &'static str {
        match self {
            DoctorStatus::Ready => "ready",
            DoctorStatus::Caution => "caution",
            DoctorStatus::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionLevel {
    None,
    Validate,
    Refresh,
    Review,
    Stop,
}

impl ActionLevel {
    fn label(self) -> &'static str {
        match self {
            ActionLevel::None => "none",
            ActionLevel::Validate => "validate",
            ActionLevel::Refresh => "refresh",
            ActionLevel::Review => "review",
            ActionLevel::Stop => "stop",
        }
    }

    pub fn strict_exit_code(self) -> i32 {
        match self {
            ActionLevel::None => 0,
            ActionLevel::Refresh => 10,
            ActionLevel::Validate => 20,
            ActionLevel::Review | ActionLevel::Stop => 30,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentGates {
    pub matrix_fresh: bool,
    pub confidence_ok: bool,
    pub unknown_files: bool,
    pub high_risk_changed: bool,
    pub medium_risk_changed: bool,
    pub dirty_now: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RecommendedCommand {
    pub kind: RecommendationKind,
    pub command: Option<String>,
    pub argv: Option<Vec<String>>,
    pub label: String,
    pub reason: String,
    pub reason_code: String,
    pub required: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationKind {
    Command,
    Manual,
}

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
                "schema_version": SCAN_SCHEMA_VERSION,
                "scoring_version": SCORING_VERSION,
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
    let files: Vec<FileRisk> = matrix
        .files
        .iter()
        .take(top)
        .cloned()
        .map(analyze::normalize_agent_fields)
        .collect();

    if is_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "schema_version": MATRIX_SCHEMA_VERSION,
                "scoring_version": SCORING_VERSION,
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
        print_file_rows(&files.iter().collect::<Vec<_>>());
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
                "schema_version": RISK_SCHEMA_VERSION,
                "scoring_version": SCORING_VERSION,
                "matrix_health": health,
                "files": risks,
                "advice": advice_for(health, &risks),
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
        println!("  Advice: {}", advice_for(health, &risks));
    }
    Ok(())
}

pub fn print_doctor(
    matrix: &FragilityMatrix,
    health: &MatrixHealth,
    changed_files: &[String],
    is_json: bool,
) -> Result<AgentDoctor, SentinelError> {
    let doctor = build_doctor(matrix, health, changed_files);

    if is_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "ok": true,
                "schema_version": &doctor.schema_version,
                "scoring_version": &doctor.scoring_version,
                "status": doctor.status,
                "action_level": doctor.action_level,
                "doctor": &doctor,
                "matrix_health": &doctor.matrix_health,
                "changed_files": &doctor.changed_files,
                "changed_file_risks": &doctor.changed_file_risks,
                "advice": &doctor.advice,
                "recommendations": &doctor.recommendations,
                "recommended_commands": &doctor.recommended_commands,
            }))?
        );
    } else {
        println!(
            "sentinel doctor: {} ({})",
            doctor.status.label(),
            doctor.action_level.label()
        );
        println!();
        println!(
            "  Matrix confidence: {}",
            doctor.matrix_health.confidence.label()
        );
        println!("  Matrix stale: {}", doctor.matrix_health.stale);
        println!(
            "  Commits scanned: {}",
            doctor.matrix_health.commits_scanned
        );
        println!("  Tracked files: {}", doctor.matrix_health.tracked_files);
        println!("  Changed files: {}", doctor.changed_files.len());
        println!();
        print_doctor_gates(&doctor.gates);
        println!();
        print_health_warnings(&doctor.matrix_health);
        println!();

        if doctor.changed_file_risks.is_empty() {
            println!("  No changed files detected.");
        } else {
            println!("  Changed-file risk:");
            for risk in &doctor.changed_file_risks {
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
        println!("  Advice: {}", doctor.advice);
        println!();
        println!("  Recommended next commands:");
        for recommendation in &doctor.recommended_commands {
            let required = if recommendation.required {
                "required"
            } else {
                "optional"
            };
            match recommendation.kind {
                RecommendationKind::Command => println!(
                    "    {} [{}] — {} ({})",
                    recommendation.command.as_deref().unwrap_or(""),
                    required,
                    recommendation.label,
                    recommendation.reason_code
                ),
                RecommendationKind::Manual => println!(
                    "    {} [{}] — {}",
                    recommendation.label, required, recommendation.reason_code
                ),
            }
        }
    }
    Ok(doctor)
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
                "schema_version": TESTS_SCHEMA_VERSION,
                "scoring_version": SCORING_VERSION,
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
                "schema_version": STATUS_SCHEMA_VERSION,
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
            let risk = analyze::find_file(matrix, file)
                .cloned()
                .unwrap_or_else(|| analyze::synthetic_quiet_file(file));
            analyze::normalize_agent_fields(risk)
        })
        .collect()
}

pub fn build_doctor(
    matrix: &FragilityMatrix,
    health: &MatrixHealth,
    changed_files: &[String],
) -> AgentDoctor {
    let mut risks = risks_for(matrix, changed_files);
    sort_doctor_risks(&mut risks);
    let gates = gates_for(health, &risks);
    let action_level = action_level_for(&gates);
    let status = status_for(action_level);
    let recommended_commands = recommended_commands_for(action_level, &gates, &risks);
    let recommendations = recommended_commands
        .iter()
        .map(recommendation_label)
        .collect();

    AgentDoctor {
        schema_version: DOCTOR_SCHEMA_VERSION.to_string(),
        scoring_version: SCORING_VERSION.to_string(),
        status,
        action_level,
        gates,
        matrix_health: health.clone(),
        changed_files: changed_files.to_vec(),
        changed_file_risks: risks.clone(),
        advice: advice_for(health, &risks),
        recommendations,
        recommended_commands,
    }
}

fn gates_for(health: &MatrixHealth, risks: &[FileRisk]) -> AgentGates {
    AgentGates {
        matrix_fresh: health.matrix_head_sha.is_some() && health.head_matches,
        confidence_ok: health.confidence != MatrixConfidence::Low,
        unknown_files: risks
            .iter()
            .any(|risk| risk.coverage_status == CoverageStatus::Unknown),
        high_risk_changed: risks.iter().any(|risk| risk.level == RiskLevel::High),
        medium_risk_changed: risks.iter().any(|risk| risk.level == RiskLevel::Medium),
        dirty_now: health.dirty_now,
    }
}

fn action_level_for(gates: &AgentGates) -> ActionLevel {
    if !gates.matrix_fresh && !gates.confidence_ok && gates.high_risk_changed {
        ActionLevel::Stop
    } else if gates.high_risk_changed {
        ActionLevel::Review
    } else if !gates.matrix_fresh || !gates.confidence_ok {
        ActionLevel::Refresh
    } else if gates.unknown_files || gates.medium_risk_changed {
        ActionLevel::Validate
    } else {
        ActionLevel::None
    }
}

fn status_for(action_level: ActionLevel) -> DoctorStatus {
    match action_level {
        ActionLevel::None => DoctorStatus::Ready,
        ActionLevel::Refresh | ActionLevel::Validate => DoctorStatus::Caution,
        ActionLevel::Review | ActionLevel::Stop => DoctorStatus::Blocked,
    }
}

fn recommended_commands_for(
    action_level: ActionLevel,
    gates: &AgentGates,
    risks: &[FileRisk],
) -> Vec<RecommendedCommand> {
    let mut commands = Vec::new();

    if !gates.matrix_fresh {
        commands.push(command_recommendation(
            "sentinel scan --force",
            &["sentinel", "scan", "--force"],
            "Refresh the matrix before relying on risk scores",
            "matrix_not_fresh",
            "matrix is missing HEAD metadata or was generated from a different HEAD",
            true,
        ));
    }

    if !gates.confidence_ok {
        commands.push(manual_recommendation(
            "Treat Sentinel scores as low-confidence hints and broaden validation",
            "low_confidence_matrix",
            "matrix confidence is low",
            action_level != ActionLevel::Refresh,
        ));
    }

    if gates.unknown_files {
        commands.push(manual_recommendation(
            "Run focused validation for files unknown to the matrix",
            "unknown_file_coverage",
            "one or more changed files has coverage_status=unknown",
            true,
        ));
    }

    if gates.high_risk_changed {
        commands.push(manual_recommendation(
            "Request review before proceeding with high-risk changed files",
            "high_risk_changed",
            "one or more changed files is high risk",
            true,
        ));
        commands.push(manual_recommendation(
            "Run targeted related tests, then full project validation",
            "high_risk_validation",
            "high-risk files need stronger validation evidence",
            true,
        ));
    } else if gates.medium_risk_changed {
        commands.push(manual_recommendation(
            "Run related or focused tests for medium-risk changed files",
            "medium_risk_validation",
            "one or more changed files is medium risk",
            true,
        ));
    }

    if commands.is_empty() {
        if risks.is_empty() {
            commands.push(manual_recommendation(
                "No changed files detected; no Sentinel-specific action required",
                "no_changed_files",
                "sentinel found no changed files",
                false,
            ));
        } else {
            commands.push(manual_recommendation(
                "Use normal project validation",
                "normal_validation",
                "changed files have low or quiet historical risk",
                false,
            ));
        }
    }

    commands
}

fn command_recommendation(
    command: &str,
    argv: &[&str],
    label: &str,
    reason_code: &str,
    reason: &str,
    required: bool,
) -> RecommendedCommand {
    RecommendedCommand {
        kind: RecommendationKind::Command,
        command: Some(command.to_string()),
        argv: Some(argv.iter().map(|part| part.to_string()).collect()),
        label: label.to_string(),
        reason: reason.to_string(),
        reason_code: reason_code.to_string(),
        required,
    }
}

fn manual_recommendation(
    label: &str,
    reason_code: &str,
    reason: &str,
    required: bool,
) -> RecommendedCommand {
    RecommendedCommand {
        kind: RecommendationKind::Manual,
        command: None,
        argv: None,
        label: label.to_string(),
        reason: reason.to_string(),
        reason_code: reason_code.to_string(),
        required,
    }
}

fn recommendation_label(recommendation: &RecommendedCommand) -> String {
    recommendation
        .command
        .clone()
        .unwrap_or_else(|| recommendation.label.clone())
}

fn sort_doctor_risks(risks: &mut [FileRisk]) {
    risks.sort_by(|a, b| {
        risk_urgency(a)
            .cmp(&risk_urgency(b))
            .then_with(|| b.risk_score.cmp(&a.risk_score))
            .then_with(|| a.path.cmp(&b.path))
    });
}

fn risk_urgency(risk: &FileRisk) -> u8 {
    if risk.coverage_status == CoverageStatus::Unknown {
        0
    } else {
        match risk.level {
            RiskLevel::High => 1,
            RiskLevel::Medium => 2,
            RiskLevel::Low => 3,
            RiskLevel::Quiet => 4,
        }
    }
}

fn print_doctor_gates(gates: &AgentGates) {
    println!("  Gates:");
    println!("    matrix_fresh: {}", gates.matrix_fresh);
    println!("    confidence_ok: {}", gates.confidence_ok);
    println!("    unknown_files: {}", gates.unknown_files);
    println!("    high_risk_changed: {}", gates.high_risk_changed);
    println!("    medium_risk_changed: {}", gates.medium_risk_changed);
    println!("    dirty_now: {}", gates.dirty_now);
}

fn advice_for(health: &MatrixHealth, risks: &[FileRisk]) -> String {
    let file_advice = if risks.iter().any(|risk| risk.level == RiskLevel::High) {
        "high-risk file present; run targeted tests first, then full validation before commit"
            .to_string()
    } else if risks.iter().any(|risk| risk.level == RiskLevel::Medium) {
        "medium risk; run related tests and consider full validation if behavior changed"
            .to_string()
    } else if risks.iter().any(|risk| !risk.known_in_matrix) {
        "unknown file history; use normal validation and consider adding focused tests".to_string()
    } else if risks.is_empty() {
        "no changed files detected".to_string()
    } else {
        "low historical risk; use normal validation for the project".to_string()
    };

    if health.stale || health.matrix_head_sha.is_none() {
        format!("matrix is stale or unverified; run `sentinel scan --force` before relying on file risk. {file_advice}")
    } else if health.confidence == MatrixConfidence::Low {
        format!("low-confidence matrix; treat scores as hints. {file_advice}")
    } else {
        file_advice
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_precedence_refreshes_stale_unknown_before_validation() {
        let gates = AgentGates {
            matrix_fresh: false,
            confidence_ok: true,
            unknown_files: true,
            high_risk_changed: false,
            medium_risk_changed: false,
            dirty_now: true,
        };

        assert_eq!(action_level_for(&gates), ActionLevel::Refresh);
        assert_eq!(status_for(ActionLevel::Refresh), DoctorStatus::Caution);
    }

    #[test]
    fn action_precedence_reviews_fresh_high_risk_changes() {
        let gates = AgentGates {
            matrix_fresh: true,
            confidence_ok: true,
            unknown_files: false,
            high_risk_changed: true,
            medium_risk_changed: false,
            dirty_now: true,
        };

        assert_eq!(action_level_for(&gates), ActionLevel::Review);
        assert_eq!(status_for(ActionLevel::Review), DoctorStatus::Blocked);
    }

    #[test]
    fn action_precedence_stops_on_stale_low_confidence_high_risk_changes() {
        let gates = AgentGates {
            matrix_fresh: false,
            confidence_ok: false,
            unknown_files: false,
            high_risk_changed: true,
            medium_risk_changed: false,
            dirty_now: true,
        };

        assert_eq!(action_level_for(&gates), ActionLevel::Stop);
        assert_eq!(status_for(ActionLevel::Stop), DoctorStatus::Blocked);
    }

    #[test]
    fn action_precedence_validates_fresh_unknown_or_medium_risk_changes() {
        let unknown = AgentGates {
            matrix_fresh: true,
            confidence_ok: true,
            unknown_files: true,
            high_risk_changed: false,
            medium_risk_changed: false,
            dirty_now: true,
        };
        let medium = AgentGates {
            matrix_fresh: true,
            confidence_ok: true,
            unknown_files: false,
            high_risk_changed: false,
            medium_risk_changed: true,
            dirty_now: true,
        };

        assert_eq!(action_level_for(&unknown), ActionLevel::Validate);
        assert_eq!(action_level_for(&medium), ActionLevel::Validate);
    }

    #[test]
    fn clean_gates_need_no_sentinel_specific_action() {
        let gates = AgentGates {
            matrix_fresh: true,
            confidence_ok: true,
            unknown_files: false,
            high_risk_changed: false,
            medium_risk_changed: false,
            dirty_now: false,
        };

        assert_eq!(action_level_for(&gates), ActionLevel::None);
        assert_eq!(status_for(ActionLevel::None), DoctorStatus::Ready);
    }

    #[test]
    fn strict_exit_codes_cover_full_action_contract() {
        assert_eq!(ActionLevel::None.strict_exit_code(), 0);
        assert_eq!(ActionLevel::Refresh.strict_exit_code(), 10);
        assert_eq!(ActionLevel::Validate.strict_exit_code(), 20);
        assert_eq!(ActionLevel::Review.strict_exit_code(), 30);
        assert_eq!(ActionLevel::Stop.strict_exit_code(), 30);
    }
}
