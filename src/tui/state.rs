use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Agent {
    pub name: String,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub status: AgentStatus,
    pub agent_type: String,
    pub port: Option<u16>,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum AgentStatus {
    Stopped,
    Running,
    Unhealthy,
    Building,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Stopped => write!(f, "Stopped"),
            AgentStatus::Running => write!(f, "Running"),
            AgentStatus::Unhealthy => write!(f, "Unhealthy"),
            AgentStatus::Building => write!(f, "Building"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub repo: String,
    pub agent: Option<String>,
    pub status: ProjectStatus,
    pub base_branch: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ProjectStatus {
    Active,
    Inactive,
    Error,
}

impl std::fmt::Display for ProjectStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectStatus::Active => write!(f, "Active"),
            ProjectStatus::Inactive => write!(f, "Inactive"),
            ProjectStatus::Error => write!(f, "Error"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pipeline {
    pub id: String,
    pub project: String,
    pub stage: PipelineStage,
    pub status: PipelineStatus,
    pub started_at: Option<u64>,
    pub output: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PipelineStage {
    Plan,
    Develop,
    Review,
    Release,
}

impl std::fmt::Display for PipelineStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineStage::Plan => write!(f, "Plan"),
            PipelineStage::Develop => write!(f, "Develop"),
            PipelineStage::Review => write!(f, "Review"),
            PipelineStage::Release => write!(f, "Release"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PipelineStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

impl std::fmt::Display for PipelineStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineStatus::Pending => write!(f, "Pending"),
            PipelineStatus::Running => write!(f, "Running"),
            PipelineStatus::Completed => write!(f, "Completed"),
            PipelineStatus::Failed => write!(f, "Failed"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SystemStatus {
    pub docker_available: bool,
    pub dagger_available: bool,
    pub smith_installed: bool,
    pub version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct UserPreferences {
    pub last_active_tab: String,
    pub refresh_interval: u64,
    pub theme: String,
}

impl UserPreferences {
    pub fn default_prefs() -> Self {
        Self {
            last_active_tab: "dashboard".to_string(),
            refresh_interval: 5000,
            theme: "dark".to_string(),
        }
    }
}
