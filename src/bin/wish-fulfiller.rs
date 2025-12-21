use anyhow::{anyhow, Result};
use chrono::Utc;
use futures::StreamExt;
use k8s_openapi::api::core::v1::ConfigMap;
use kube::{
    api::{Api, Patch, PatchParams, ResourceExt},
    runtime::{controller::Action, watcher::Config, Controller},
    Client,
};
use serde_json::json;
use std::collections::HashSet;
use std::process::Command as ProcessCommand;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};
use wish_system::{Command, CommandType, Wish, WishPhase, WishStatus};

struct Context {
    client: Client,
}

struct PermissionConfig {
    allowed_namespaces: HashSet<String>,
    allowed_resources: HashSet<String>,
    forbidden_operations: HashSet<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
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

async fn reconcile(wish: Arc<Wish>, ctx: Arc<Context>) -> Result<Action> {
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

    info!("Executing plan with {} commands", plan.commands.len());

    match execute_plan(&plan.commands, &namespace, &permissions).await {
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

fn error_policy(_wish: Arc<Wish>, error: &anyhow::Error, _ctx: Arc<Context>) -> Action {
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

async fn load_permission_config(client: &Client, namespace: &str) -> Result<PermissionConfig> {
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
) -> Result<()> {
    for (i, cmd) in commands.iter().enumerate() {
        info!("Executing command {}/{}: {:?}", i + 1, commands.len(), cmd.command_type);

        // Validate permissions
        validate_command(cmd, namespace, permissions)?;

        match &cmd.command_type {
            CommandType::Kubectl => execute_kubectl(cmd, namespace).await?,
            CommandType::Shell => execute_shell(cmd).await?,
        }
    }

    Ok(())
}

fn validate_command(
    cmd: &Command,
    namespace: &str,
    permissions: &PermissionConfig,
) -> Result<()> {
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

async fn execute_kubectl(cmd: &Command, namespace: &str) -> Result<()> {
    let mut args: Vec<&str> = cmd.command.split_whitespace().collect();
    
    // Remove 'kubectl' if present
    if args.first() == Some(&"kubectl") {
        args.remove(0);
    }

    // Add namespace if not present
    if !args.contains(&"-n") && !args.contains(&"--namespace") {
        args.push("-n");
        args.push(namespace);
    }

    let output = if let Some(yaml) = &cmd.yaml {
        // Pipe YAML to kubectl
        ProcessCommand::new("kubectl")
            .args(&args)
            .arg("--dry-run=client")
            .arg("-o")
            .arg("yaml")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(mut stdin) = child.stdin.take() {
                    stdin.write_all(yaml.as_bytes())?;
                }
                child.wait_with_output()
            })?
    } else {
        ProcessCommand::new("kubectl")
            .args(&args)
            .output()?
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("kubectl command failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    info!("kubectl output: {}", stdout);

    Ok(())
}

async fn execute_shell(cmd: &Command) -> Result<()> {
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
) -> Result<()> {
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
) -> Result<()> {
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
