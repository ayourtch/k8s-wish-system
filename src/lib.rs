use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[kube(
    group = "wish.ayourt.ch",
    version = "v1alpha1",
    kind = "Wish",
    plural = "wishes",
    status = "WishStatus",
    namespaced
)]
#[kube(printcolumn = r#"{"name":"Phase", "type":"string", "jsonPath":".status.phase"}"#)]
#[kube(printcolumn = r#"{"name":"Name", "type":"string", "jsonPath":".status.name"}"#)]
#[kube(printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#)]
#[serde(rename_all = "camelCase")]
pub struct WishSpec {
    /// Natural language wish text
    pub wish: String,

    /// Auto-execute after granting
    #[serde(default)]
    pub auto_fulfill: bool,

    /// If true, plan but don't execute
    #[serde(default = "default_dry_run")]
    pub dry_run: bool,

    /// Target namespace for deployed resources (defaults to "default")
    #[serde(default = "default_target_namespace")]
    pub target_namespace: String,

    /// Optional LLM configuration override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_config: Option<LlmConfig>,

    /// Creator identity for RBAC impersonation
    /// This field is set automatically by kubectl-wish
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator: Option<CreatorIdentity>,
}

fn default_dry_run() -> bool {
    true
}

fn default_target_namespace() -> String {
    "default".to_string()
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LlmConfig {
    pub endpoint: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credentials_secret_ref: Option<SecretRef>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct SecretRef {
    pub name: String,
    pub key: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatorIdentity {
    /// Kubernetes username for impersonation
    pub username: String,
    /// User groups at creation time (for impersonation)
    pub groups: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct WishStatus {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<WishPhase>,
    
    /// LLM-assigned semantic name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    
    /// Execution plan
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<ExecutionPlan>,
    
    /// Dry run results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dry_run_results: Option<Vec<DryRunResult>>,
    
    /// Immutable once true
    #[serde(default)]
    pub fulfilled: bool,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfilled_at: Option<DateTime<Utc>>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema, PartialEq)]
pub enum WishPhase {
    Requested,
    Granted,
    Fulfilled,
    Failed,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct ExecutionPlan {
    pub commands: Vec<Command>,
    pub reasoning: String,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    #[serde(rename = "type")]
    pub command_type: CommandType,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yaml: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum CommandType {
    Kubectl,
    Shell,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DryRunResult {
    pub command: String,
    pub expected_outcome: String,
}

// LLM client types
#[derive(Deserialize, Serialize, Debug)]
pub struct LlmRequest {
    pub model: String,
    pub messages: Vec<LlmMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LlmResponse {
    pub choices: Vec<LlmChoice>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct LlmChoice {
    pub message: LlmMessage,
}
