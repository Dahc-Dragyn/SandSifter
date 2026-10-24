use serde::{Deserialize, Serialize};

/// Autonomous rule that automatically triggers deep analysis on new viral outliers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomationRule {
    pub uuid: String,
    pub name: String,
    pub channel_uuid: String,
    pub outlier_score_min: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub views_min: Option<u64>,
    #[serde(default)]
    pub engagement_rate_min_percent: f64,
    #[serde(default = "default_daily_limit")]
    pub daily_limit: u32,
    #[serde(default = "default_true")]
    pub active: bool,
    #[serde(default)]
    pub managed: bool,
    pub created_at: String,
    pub updated_at: String,
}

fn default_daily_limit() -> u32 {
    10
}

fn default_true() -> bool {
    true
}

/// Request body to create a new automation rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomationRuleCreate {
    pub name: String,
    pub channel_uuid: String,
    #[serde(default = "default_outlier_min")]
    pub outlier_score_min: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub views_min: Option<u64>,
    #[serde(default = "default_engagement_min")]
    pub engagement_rate_min_percent: f64,
    #[serde(default = "default_daily_limit")]
    pub daily_limit: u32,
    #[serde(default = "default_true")]
    pub active: bool,
}

fn default_outlier_min() -> f64 {
    2.5
}

fn default_engagement_min() -> f64 {
    2.0
}

/// Workspace tenant definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceItem {
    pub uuid: String,
    pub name: String,
    pub is_owner: bool,
    #[serde(default)]
    pub is_active: bool,
    pub created_at: String,
}

/// Organization credit and quota usage state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrgUsageResponse {
    pub plan_tier: String,
    pub credits_remaining: u64,
    pub credits_total: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<String>,
}
