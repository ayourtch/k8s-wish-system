use anyhow::anyhow;
use futures::StreamExt;
use k8s_openapi::api::core::v1::{ConfigMap, Secret};
use kube::{
    api::{Api, ListParams, Patch, PatchParams, ResourceExt},
    runtime::{controller::Action, watcher::Config, watcher, Controller, WatchStreamExt},
    Client, CustomResourceExt,
};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tracing::{error, info, warn};
use wish_system::{
    ExecutionPlan, LlmConfig, LlmMessage, LlmRequest, LlmResponse, Wish, WishPhase, WishStatus,
    Command, DryRunResult,
};

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

async fn watch_config_changes(client: Client) -> anyhow::Result<()> {
    info!("Starting ConfigMap watcher for LLM configuration changes");

    loop {
        // Watch all wish-grantor-config ConfigMaps across all namespaces
        let cm_api: Api<ConfigMap> = Api::all(client.clone());
        let mut stream = watcher(cm_api, Config::default().any_semantic()).applied_objects().boxed();

        while let Some(event) = stream.next().await {
            match event {
                Ok(cm) => {
                    let ns = cm.namespace().unwrap_or_else(|| "default".to_string());
                    info!("ConfigMap wish-grantor-config changed in namespace: {}", ns);

                    // Retry failed wishes in this namespace
                    if let Err(e) = retry_failed_wishes(&client, &ns).await {
                        error!("Failed to retry wishes in namespace {}: {}", ns, e);
                    }
                }
                Err(e) => {
                    error!("ConfigMap watch error: {}", e);
                    // Wait before retrying
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    break; // Restart the watch
                }
            }
        }

        // If we get here, the stream ended, wait and retry
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

async fn retry_failed_wishes(client: &Client, namespace: &str) -> anyhow::Result<()> {
    let wishes_api: Api<Wish> = Api::namespaced(client.clone(), namespace);
    let lp = ListParams::default();
    let wishes = wishes_api.list(&lp).await?;

    let mut retry_count = 0;
    for wish in wishes.items {
        // Only retry wishes that failed due to LLM connection issues
        if let Some(status) = &wish.status {
            if matches!(status.phase, Some(WishPhase::Failed)) {
                if let Some(error) = &status.error {
                    // Check if error is related to LLM connectivity
                    if error.contains("Connection refused")
                        || error.contains("connection error")
                        || error.contains("error sending request") {

                        let name = wish.name_any();
                        info!("Retrying wish {} after config change", name);

                        // Reset to Requested state to trigger reconciliation
                        let patch = json!({
                            "status": {
                                "phase": "Requested",
                                "error": null
                            }
                        });

                        if let Err(e) = wishes_api
                            .patch_status(&name, &PatchParams::default(), &Patch::Merge(&patch))
                            .await
                        {
                            warn!("Failed to reset wish {}: {}", name, e);
                        } else {
                            retry_count += 1;
                        }
                    }
                }
            }
        }
    }

    if retry_count > 0 {
        info!("Reset {} failed wish(es) for retry in namespace {}", retry_count, namespace);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    info!("Starting wish-grantor controller");

    let client = Client::try_default().await?;
    let wishes = Api::<Wish>::all(client.clone());

    // Print CRD for installation
    println!("{}", serde_yaml::to_string(&Wish::crd())?);

    // Clone client for config watcher
    let config_client = client.clone();

    // Spawn config watcher task
    tokio::spawn(async move {
        if let Err(e) = watch_config_changes(config_client).await {
            error!("Config watcher error: {}", e);
        }
    });

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

    // Skip if already granted, fulfilled, or failed
    if let Some(status) = &wish.status {
        if matches!(
            status.phase,
            Some(WishPhase::Granted) | Some(WishPhase::Fulfilled) | Some(WishPhase::Failed)
        ) {
            info!("Wish already processed: {:?}", status.phase);
            return Ok(Action::await_change());
        }
    }

    // Load LLM configuration
    let llm_config = match load_llm_config(&ctx.client, &namespace, &wish).await {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load LLM config: {}", e);
            update_status_failed(&ctx.client, &namespace, &name, &e.to_string()).await?;
            return Ok(Action::requeue(Duration::from_secs(300)));
        }
    };

    // Get API key if needed
    let api_key = if let Some(ref secret_ref) = llm_config.credentials_secret_ref {
        match get_secret_value(&ctx.client, &namespace, &secret_ref.name, &secret_ref.key).await {
            Ok(key) => Some(key),
            Err(e) => {
                warn!("Failed to get API key: {}, proceeding without auth", e);
                None
            }
        }
    } else {
        None
    };

    // Call LLM to generate plan
    match generate_plan(&llm_config, &wish.spec.wish, &api_key, wish.spec.dry_run).await {
        Ok((plan, wish_name, dry_run_results)) => {
            update_status_granted(
                &ctx.client,
                &namespace,
                &name,
                &wish_name,
                &plan,
                dry_run_results,
            )
            .await?;
            info!("Wish granted: {}", wish_name);
            Ok(Action::await_change())
        }
        Err(e) => {
            error!("Failed to generate plan: {}", e);
            update_status_failed(&ctx.client, &namespace, &name, &e.to_string()).await?;
            Ok(Action::requeue(Duration::from_secs(300)))
        }
    }
}

fn error_policy(_wish: Arc<Wish>, error: &ReconcileError, _ctx: Arc<Context>) -> Action {
    error!("Reconciliation error: {}", error);
    Action::requeue(Duration::from_secs(60))
}

async fn load_llm_config(client: &Client, namespace: &str, wish: &Wish) -> anyhow::Result<LlmConfig> {
    // Priority: wish spec > configmap > env vars
    if let Some(config) = &wish.spec.llm_config {
        return Ok(config.clone());
    }

    // Try to load from ConfigMap
    let cm_api: Api<ConfigMap> = Api::namespaced(client.clone(), namespace);
    if let Ok(cm) = cm_api.get("wish-grantor-config").await {
        if let Some(data) = cm.data {
            let endpoint = data
                .get("llmEndpoint")
                .ok_or_else(|| anyhow!("llmEndpoint not found in ConfigMap"))?;
            let model = data
                .get("llmModel")
                .ok_or_else(|| anyhow!("llmModel not found in ConfigMap"))?;

            let credentials_secret_ref = data.get("credentialsSecretName").map(|name| {
                wish_system::SecretRef {
                    name: name.clone(),
                    key: data
                        .get("credentialsSecretKey")
                        .cloned()
                        .unwrap_or_else(|| "apiKey".to_string()),
                }
            });

            return Ok(LlmConfig {
                endpoint: endpoint.clone(),
                model: model.clone(),
                credentials_secret_ref,
            });
        }
    }

    // Fallback to env vars
    Ok(LlmConfig {
        endpoint: std::env::var("LLM_ENDPOINT")
            .unwrap_or_else(|_| "http://localhost:11434/v1".to_string()),
        model: std::env::var("LLM_MODEL").unwrap_or_else(|_| "llama3.2:latest".to_string()),
        credentials_secret_ref: None,
    })
}

async fn get_secret_value(
    client: &Client,
    namespace: &str,
    secret_name: &str,
    key: &str,
) -> anyhow::Result<String> {
    let secret_api: Api<Secret> = Api::namespaced(client.clone(), namespace);
    let secret = secret_api.get(secret_name).await?;

    let data = secret
        .data
        .ok_or_else(|| anyhow!("Secret has no data"))?;
    let value = data
        .get(key)
        .ok_or_else(|| anyhow!("Key {} not found in secret", key))?;

    Ok(String::from_utf8(value.0.clone())?)
}

async fn generate_plan(
    config: &LlmConfig,
    wish_text: &str,
    api_key: &Option<String>,
    dry_run: bool,
) -> anyhow::Result<(ExecutionPlan, String, Option<Vec<DryRunResult>>)> {
    let system_prompt = r#"You are a Kubernetes operations assistant. Given a user's wish, you must:
1. Assign a concise semantic name to the wish (lowercase-with-dashes)
2. Generate the exact kubectl commands and/or YAML manifests needed to fulfill it
3. Provide clear reasoning for your plan

Respond ONLY with valid JSON in this exact format:
{
  "name": "descriptive-wish-name",
  "reasoning": "Brief explanation of the approach",
  "commands": [
    {
      "type": "kubectl",
      "command": "kubectl apply -f -",
      "yaml": "apiVersion: v1\nkind: Pod\n..."
    },
    {
      "type": "shell",
      "command": "echo 'example'"
    }
  ]
}

For dry-run mode, also include:
{
  "dryRunResults": [
    {
      "command": "kubectl apply...",
      "expectedOutcome": "Pod 'example' would be created in namespace 'default'"
    }
  ]
}
"#;

    let user_prompt = if dry_run {
        format!("Wish: {}\n\nMode: DRY RUN - describe what would happen without executing", wish_text)
    } else {
        format!("Wish: {}", wish_text)
    };

    let request = LlmRequest {
        model: config.model.clone(),
        messages: vec![
            LlmMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            LlmMessage {
                role: "user".to_string(),
                content: user_prompt,
            },
        ],
        temperature: Some(0.1),
        max_tokens: Some(2000),
    };

    let mut req = reqwest::Client::new()
        .post(format!("{}/chat/completions", config.endpoint))
        .json(&request);

    if let Some(key) = api_key {
        req = req.header("Authorization", format!("Bearer {}", key));
    }

    let response: LlmResponse = req.send().await?.json().await?;

    let content = &response
        .choices
        .first()
        .ok_or_else(|| anyhow!("No choices in LLM response"))?
        .message
        .content;

    // Parse JSON response
    let parsed: serde_json::Value = serde_json::from_str(content)
        .map_err(|e| anyhow!("Failed to parse LLM response as JSON: {}\nContent: {}", e, content))?;

    let wish_name = parsed["name"]
        .as_str()
        .ok_or_else(|| anyhow!("Missing 'name' in response"))?
        .to_string();

    let reasoning = parsed["reasoning"]
        .as_str()
        .ok_or_else(|| anyhow!("Missing 'reasoning' in response"))?
        .to_string();

    let commands: Vec<Command> = serde_json::from_value(parsed["commands"].clone())
        .map_err(|e| anyhow!("Failed to parse commands: {}", e))?;

    let plan = ExecutionPlan { commands, reasoning };

    let dry_run_results = if dry_run {
        parsed["dryRunResults"]
            .as_array()
            .and_then(|arr| serde_json::from_value(serde_json::Value::Array(arr.clone())).ok())
    } else {
        None
    };

    Ok((plan, wish_name, dry_run_results))
}

async fn update_status_granted(
    client: &Client,
    namespace: &str,
    name: &str,
    wish_name: &str,
    plan: &ExecutionPlan,
    dry_run_results: Option<Vec<DryRunResult>>,
) -> anyhow::Result<()> {
    let api: Api<Wish> = Api::namespaced(client.clone(), namespace);

    let status = WishStatus {
        phase: Some(WishPhase::Granted),
        name: Some(wish_name.to_string()),
        plan: Some(plan.clone()),
        dry_run_results,
        fulfilled: false,
        fulfilled_at: None,
        error: None,
    };

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
        ..Default::default()
    };

    let patch = json!({
        "status": status
    });

    api.patch_status(name, &PatchParams::default(), &Patch::Merge(&patch))
        .await?;

    Ok(())
}
