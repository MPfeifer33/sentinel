use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragilityMatrix {
    pub generated_at_unix: u64,
    pub repo: String,
    pub history_limit: usize,
    pub commits_scanned: usize,
    pub files: Vec<FileRisk>,
    pub summary: MatrixSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MatrixSummary {
    pub tracked_files: usize,
    pub high_risk: usize,
    pub medium_risk: usize,
    pub low_risk: usize,
    pub quiet: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRisk {
    pub path: String,
    pub risk_score: u32,
    pub level: RiskLevel,
    pub commits: usize,
    pub recent_commits: usize,
    pub bugfix_commits: usize,
    pub revert_commits: usize,
    pub test_cochanges: usize,
    pub total_churn: usize,
    pub last_touched: Option<String>,
    pub related_tests: Vec<RelatedTest>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelatedTest {
    pub path: String,
    pub cochanges: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    High,
    Medium,
    Low,
    Quiet,
}

impl RiskLevel {
    pub fn label(self) -> &'static str {
        match self {
            RiskLevel::High => "high",
            RiskLevel::Medium => "medium",
            RiskLevel::Low => "low",
            RiskLevel::Quiet => "quiet",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileStats {
    pub path: String,
    pub commits: usize,
    pub recent_commits: usize,
    pub bugfix_commits: usize,
    pub revert_commits: usize,
    pub test_cochanges: usize,
    pub total_churn: usize,
    pub last_touched: Option<String>,
    pub related_tests: std::collections::BTreeMap<String, usize>,
}

impl FileStats {
    pub fn new(path: String) -> Self {
        Self {
            path,
            commits: 0,
            recent_commits: 0,
            bugfix_commits: 0,
            revert_commits: 0,
            test_cochanges: 0,
            total_churn: 0,
            last_touched: None,
            related_tests: std::collections::BTreeMap::new(),
        }
    }
}
