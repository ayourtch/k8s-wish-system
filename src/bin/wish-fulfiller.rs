use anyhow::anyhow;
use chrono::Utc;
use futures::StreamExt;
use kube::{
    api::{Api, Patch, PatchParams, ResourceExt, DynamicObject},
    config::Config as KubeConfig,
    core::GroupVersionKind,
    discovery::{Discovery, Scope},
    runtime::{controller::Action, watcher::Config, Controller},
    Client,
};
use serde_json::json;
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

    // Get creator identity for impersonation
    let creator = match &wish.spec.creator {
        Some(c) => c,
        None => {
            error!("No creator identity found in wish spec - cannot impersonate");
            update_status_failed(
                &ctx.client,
                &namespace,
                &name,
                "Missing creator identity for impersonation",
            )
            .await?;
            return Ok(Action::await_change());
        }
    };

    info!("Executing wish as user: {} with {} groups", creator.username, creator.groups.len());

    // Create impersonated client
    let impersonated_client = match create_impersonated_client(&ctx.client, &creator.username, &creator.groups).await {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create impersonated client: {}", e);
            update_status_failed(&ctx.client, &namespace, &name, &e.to_string()).await?;
            return Ok(Action::requeue(Duration::from_secs(300)));
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
    info!(
        "Executing plan with {} commands in target namespace: {} as user: {}",
        plan.commands.len(),
        target_namespace,
        creator.username
    );

    match execute_plan(&plan.commands, target_namespace, &impersonated_client).await {
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

async fn create_impersonated_client(_base_client: &Client, username: &str, groups: &[String]) -> anyhow::Result<Client> {
    info!("Impersonating user {} with groups: {:?}", username, groups);

    // Create impersonated client with username and groups
    let config = KubeConfig::infer().await?;
    let mut impersonated_config = config.clone();
    impersonated_config.auth_info.impersonate = Some(username.to_string());
    impersonated_config.auth_info.impersonate_groups = Some(groups.to_vec());

    let client = Client::try_from(impersonated_config)?;

    Ok(client)
}

async fn execute_plan(
    commands: &[Command],
    namespace: &str,
    client: &Client,
) -> anyhow::Result<()> {
    for (i, cmd) in commands.iter().enumerate() {
        info!("Executing command {}/{}: {:?}", i + 1, commands.len(), cmd.command_type);

        match &cmd.command_type {
            CommandType::Kubectl => execute_kubectl(cmd, namespace, client).await?,
            CommandType::Shell => {
                warn!("Shell commands are disabled for security reasons");
                return Err(anyhow!("Shell commands are not allowed"));
            }
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
