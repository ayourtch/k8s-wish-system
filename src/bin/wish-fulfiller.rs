use anyhow::anyhow;
use chrono::Utc;
use futures::StreamExt;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::{
    api::{Api, Patch, PatchParams, ResourceExt, DynamicObject},
    core::GroupVersionKind,
    discovery::{Discovery, Scope},
    runtime::{controller::Action, watcher::Config, Controller},
    Client,
};
use serde_json::json;
use std::collections::HashSet;
use std::process::Command as ProcessCommand;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tracing::{error, info, warn};
use wish_system::{Command, CommandType, Wish, WishPhase, WishStatus};

#[derive(Error, Debug)]
enum ReconcileError {
    #[error("Kube error: {0}")]
    KubeError(#[from] kube::Error),
    #[error("Anyhow error: {0}")]
    AnyhowError(#[from] anyhow::Error),
    #[error("Other error: {0}")]
    Other(String),
}

struct Context {
    client: Client,
}

struct PermissionConfig {
    allowed_namespaces: HashSet<String>,
    allowed_resources: HashSet<String>,
    forbidden_operations: HashSet<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    info!("Starting wish-fulfiller controller");

    let client = Client::try_default().await?;
    let wishes = Api::<Wish>::all(client.clone());

    Controller::new(wishes, Config::default())
        .run(reconcile, error_policy, Arc::new(Context { client }))
        .for_each(|_| futures::future::ready(()))
        .await;

    Ok(())
}

async fn reconcile(wish: Arc<Wish>, ctx: Arc<Context>) -> Result<Action, ReconcileError> {
    let namespace = wish.namespace().unwrap();
    let name = wish.name_any();

    info!("Reconciling wish: {}/{}", namespace, name);

    // Skip if not granted or already fulfilled
    let status = match &wish.status {
        Some(s) => s,
        None => return Ok(Action::await_change()),
    };

    if status.fulfilled {
        info!("Wish already fulfilled, skipping");
        return Ok(Action::await_change());
    }

    if !matches!(status.phase, Some(WishPhase::Granted)) {
        return Ok(Action::await_change());
    }

    // Check if dry-run mode
    if wish.spec.dry_run && !wish.spec.auto_fulfill {
        info!("Wish is in dry-run mode and not auto-fulfill, waiting for manual trigger");
        return Ok(Action::await_change());
    }

    // Load permission config
    let permissions = match load_permission_config(&ctx.client, &namespace).await {
        Ok(p) => p,
        Err(e) => {
            warn!("Failed to load permissions, using defaults: {}", e);
            PermissionConfig::default()
        }
    };

    // Execute the plan
    let plan = match &status.plan {
        Some(p) => p,
        None => {
            error!("No execution plan found");
            return Ok(Action::await_change());
        }
    };

    // Get target namespace for resource deployment
    let target_namespace = &wish.spec.target_namespace;
    info!("Executing plan with {} commands in target namespace: {}", plan.commands.len(), target_namespace);

    match execute_plan(&plan.commands, target_namespace, &permissions, &ctx.client).await {
        Ok(()) => {
            update_status_fulfilled(&ctx.client, &namespace, &name).await?;
            info!("Wish fulfilled successfully");
            Ok(Action::await_change())
        }
        Err(e) => {
            error!("Failed to execute plan: {}", e);
            update_status_failed(&ctx.client, &namespace, &name, &e.to_string()).await?;
            Ok(Action::requeue(Duration::from_secs(300)))
        }
    }
}

fn error_policy(_wish: Arc<Wish>, error: &ReconcileError, _ctx: Arc<Context>) -> Action {
    error!("Reconciliation error: {}", error);
    Action::requeue(Duration::from_secs(60))
}

impl Default for PermissionConfig {
    fn default() -> Self {
        Self {
            allowed_namespaces: HashSet::from_iter(vec!["default".to_string()]),
            allowed_resources: HashSet::from_iter(vec![
                "pods".to_string(),
                "deployments".to_string(),
                "services".to_string(),
                "configmaps".to_string(),
            ]),
            forbidden_operations: HashSet::from_iter(vec![
                "delete:namespaces".to_string(),
                "delete:persistentvolumes".to_string(),
            ]),
        }
    }
}

async fn load_permission_config(client: &Client, namespace: &str) -> anyhow::Result<PermissionConfig> {
    let cm_api: Api<ConfigMap> = Api::namespaced(client.clone(), namespace);
    let cm = cm_api.get("wish-fulfiller-permissions").await?;

    let data = cm.data.ok_or_else(|| anyhow!("No data in ConfigMap"))?;

    let allowed_namespaces = data
        .get("allowedNamespaces")
        .map(|s| s.split(',').map(|x| x.trim().to_string()).collect())
        .unwrap_or_else(|| HashSet::from_iter(vec!["default".to_string()]));

    let allowed_resources = data
        .get("allowedResources")
        .map(|s| s.split(',').map(|x| x.trim().to_string()).collect())
        .unwrap_or_else(|| {
            HashSet::from_iter(vec![
                "pods".to_string(),
                "deployments".to_string(),
                "services".to_string(),
            ])
        });

    let forbidden_operations = data
        .get("forbiddenOperations")
        .map(|s| s.split(',').map(|x| x.trim().to_string()).collect())
        .unwrap_or_default();

    Ok(PermissionConfig {
        allowed_namespaces,
        allowed_resources,
        forbidden_operations,
    })
}

async fn execute_plan(
    commands: &[Command],
    namespace: &str,
    permissions: &PermissionConfig,
    client: &Client,
) -> anyhow::Result<()> {
    for (i, cmd) in commands.iter().enumerate() {
        info!("Executing command {}/{}: {:?}", i + 1, commands.len(), cmd.command_type);

        // Validate permissions
        validate_command(cmd, namespace, permissions)?;

        match &cmd.command_type {
            CommandType::Kubectl => execute_kubectl(cmd, namespace, client).await?,
            CommandType::Shell => execute_shell(cmd).await?,
        }
    }

    Ok(())
}

fn validate_command(
    cmd: &Command,
    namespace: &str,
    permissions: &PermissionConfig,
) -> anyhow::Result<()> {
    // Check namespace
    if !permissions.allowed_namespaces.contains(namespace) {
        return Err(anyhow!(
            "Namespace '{}' not in allowed list",
            namespace
        ));
    }

    // Parse command to extract operation and resource
    let cmd_lower = cmd.command.to_lowercase();
    
    // Check forbidden operations
    for forbidden in &permissions.forbidden_operations {
        if cmd_lower.contains(forbidden) {
            return Err(anyhow!("Forbidden operation: {}", forbidden));
        }
    }

    // Basic resource check for kubectl commands
    if matches!(cmd.command_type, CommandType::Kubectl) {
        let has_allowed_resource = permissions.allowed_resources.iter()
            .any(|resource| cmd_lower.contains(resource));
        
        if !has_allowed_resource && !cmd_lower.contains("apply") {
            warn!("Command may reference non-allowed resource: {}", cmd.command);
        }
    }

    Ok(())
}

async fn execute_kubectl(cmd: &Command, namespace: &str, client: &Client) -> anyhow::Result<()> {
    let yaml_content = cmd.yaml.as_ref()
        .ok_or_else(|| anyhow!("No YAML content provided for kubectl command"))?;

    info!("Applying Kubernetes resource via API");

    // Parse YAML into a dynamic object
    let value: serde_yaml::Value = serde_yaml::from_str(yaml_content)?;
    let json_value = serde_json::to_value(value)?;

    let mut obj: DynamicObject = serde_json::from_value(json_value)?;

    // Set namespace if not already set and object is namespaced
    if obj.metadata.namespace.is_none() {
        obj.metadata.namespace = Some(namespace.to_string());
    }

    // Extract API information from the object
    let api_version = obj.types.as_ref()
        .map(|t| t.api_version.as_str())
        .ok_or_else(|| anyhow!("No apiVersion in resource"))?;
    let kind = obj.types.as_ref()
        .map(|t| t.kind.as_str())
        .ok_or_else(|| anyhow!("No kind in resource"))?;

    info!("Applying {} {} in namespace {}", kind, obj.name_any(), namespace);

    // Use discovery to find the API resource
    let discovery = Discovery::new(client.clone()).run().await?;

    // Parse API group and version
    // Core API resources have no group (e.g., "v1")
    // Other resources have group/version (e.g., "apps/v1")
    let (group, version) = if api_version.contains('/') {
        let parts: Vec<&str> = api_version.splitn(2, '/').collect();
        (parts[0].to_string(), parts[1].to_string())
    } else {
        // Core API - no group
        (String::new(), api_version.to_string())
    };

    // Find the API resource for this kind
    let (ar, caps) = discovery.resolve_gvk(&GroupVersionKind {
        group,
        version,
        kind: kind.to_string(),
    }).ok_or_else(|| anyhow!("Failed to resolve API resource for {}/{}", api_version, kind))?;

    // Create the appropriate API client
    let api: Api<DynamicObject> = if caps.scope == Scope::Namespaced {
        Api::namespaced_with(client.clone(), namespace, &ar)
    } else {
        Api::all_with(client.clone(), &ar)
    };

    // Apply the resource (server-side apply)
    let pp = PatchParams::apply("wish-fulfiller").force();
    let patch = Patch::Apply(&obj);
    let result = api.patch(&obj.name_any(), &pp, &patch).await?;

    info!("Successfully applied {} {}", kind, result.name_any());

    Ok(())
}

async fn execute_shell(cmd: &Command) -> anyhow::Result<()> {
    info!("Executing shell command: {}", cmd.command);

    let output = ProcessCommand::new("sh")
        .arg("-c")
        .arg(&cmd.command)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("Shell command failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    info!("Shell output: {}", stdout);

    Ok(())
}

async fn update_status_fulfilled(
    client: &Client,
    namespace: &str,
    name: &str,
) -> anyhow::Result<()> {
    let api: Api<Wish> = Api::namespaced(client.clone(), namespace);

    let status = json!({
        "phase": "Fulfilled",
        "fulfilled": true,
        "fulfilledAt": Utc::now().to_rfc3339(),
    });

    let patch = json!({
        "status": status
    });

    api.patch_status(name, &PatchParams::default(), &Patch::Merge(&patch))
        .await?;

    Ok(())
}

async fn update_status_failed(
    client: &Client,
    namespace: &str,
    name: &str,
    error: &str,
) -> anyhow::Result<()> {
    let api: Api<Wish> = Api::namespaced(client.clone(), namespace);

    let status = WishStatus {
        phase: Some(WishPhase::Failed),
        error: Some(error.to_string()),
        fulfilled: false,
        fulfilled_at: None,
        name: None,
        plan: None,
        dry_run_results: None,
    };

    let patch = json!({
        "status": status
    });

    api.patch_status(name, &PatchParams::default(), &Patch::Merge(&patch))
        .await?;

    Ok(())
}
