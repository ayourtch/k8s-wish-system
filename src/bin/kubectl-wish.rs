use anyhow::Result;
use clap::{Parser, Subcommand};
use k8s_openapi::api::core::v1::{ConfigMap, Secret};
use kube::{
    api::{Api, DeleteParams, ListParams, Patch, PatchParams, PostParams},
    Client, ResourceExt,
};
use serde_json::json;
use std::collections::BTreeMap;
use wish_system::{Wish, WishSpec};

#[derive(Parser)]
#[command(name = "kubectl-wish")]
#[command(about = "Manage Kubernetes wishes", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short = 'n', long, global = true)]
    namespace: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new wish
    Create {
        /// The wish text
        wish: String,

        /// Auto-fulfill after granting
        #[arg(long)]
        auto_fulfill: bool,

        /// Disable dry-run mode (execute immediately after granting)
        #[arg(long)]
        no_dry_run: bool,

        /// Optional name for the wish resource
        #[arg(long)]
        name: Option<String>,

        /// Target namespace for deployed resources (defaults to "default")
        #[arg(long, default_value = "default")]
        target_namespace: String,
    },

    /// List all wishes
    List,

    /// Describe a wish
    Describe {
        /// Name of the wish
        name: String,
    },

    /// Fulfill a granted wish (disable dry-run and trigger execution)
    Fulfill {
        /// Name of the wish
        name: String,
    },

    /// Delete a wish
    Delete {
        /// Name of the wish
        name: String,
    },

    /// Configure LLM settings
    Configure {
        /// LLM endpoint URL (e.g., http://localhost:11434/v1 for Ollama)
        #[arg(long)]
        endpoint: Option<String>,

        /// LLM model name (e.g., llama3.2:latest)
        #[arg(long)]
        model: Option<String>,

        /// API key for remote LLM services
        #[arg(long)]
        api_key: Option<String>,

        /// Show current configuration
        #[arg(long)]
        show: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = Client::try_default().await?;

    let namespace = cli
        .namespace
        .or_else(|| std::env::var("KUBECTL_NAMESPACE").ok())
        .unwrap_or_else(|| "default".to_string());

    match cli.command {
        Commands::Create {
            wish,
            auto_fulfill,
            no_dry_run,
            name,
            target_namespace,
        } => create_wish(&client, &namespace, &wish, auto_fulfill, !no_dry_run, name, target_namespace).await?,

        Commands::List => list_wishes(&client, &namespace).await?,

        Commands::Describe { name } => describe_wish(&client, &namespace, &name).await?,

        Commands::Fulfill { name } => fulfill_wish(&client, &namespace, &name).await?,

        Commands::Delete { name } => delete_wish(&client, &namespace, &name).await?,

        Commands::Configure {
            endpoint,
            model,
            api_key,
            show,
        } => configure_llm(&client, &namespace, endpoint, model, api_key, show).await?,
    }

    Ok(())
}

async fn create_wish(
    client: &Client,
    namespace: &str,
    wish_text: &str,
    auto_fulfill: bool,
    dry_run: bool,
    name: Option<String>,
    target_namespace: String,
) -> Result<()> {
    let api: Api<Wish> = Api::namespaced(client.clone(), namespace);

    let resource_name = name.unwrap_or_else(|| {
        format!(
            "wish-{}",
            chrono::Utc::now().timestamp()
        )
    });

    let wish = Wish::new(
        &resource_name,
        WishSpec {
            wish: wish_text.to_string(),
            auto_fulfill,
            dry_run,
            target_namespace,
            llm_config: None,
        },
    );

    let created = api.create(&PostParams::default(), &wish).await?;

    println!("Wish created: {}", created.name_any());
    println!("Status: Requested");
    
    if dry_run {
        println!("Mode: Dry-run (will not execute automatically)");
        println!("Use 'kubectl wish fulfill {}' to execute after review", resource_name);
    } else if auto_fulfill {
        println!("Mode: Auto-fulfill enabled (will execute after granting)");
    }

    Ok(())
}

async fn list_wishes(client: &Client, namespace: &str) -> Result<()> {
    let api: Api<Wish> = Api::namespaced(client.clone(), namespace);
    let wishes = api.list(&ListParams::default()).await?;

    if wishes.items.is_empty() {
        println!("No wishes found in namespace '{}'", namespace);
        return Ok(());
    }

    println!("{:<30} {:<15} {:<30} {:<10}", "NAME", "PHASE", "WISH-NAME", "AGE");
    println!("{}", "-".repeat(90));

    for wish in wishes.items {
        let name = wish.name_any();
        let phase = wish
            .status
            .as_ref()
            .and_then(|s| s.phase.as_ref())
            .map(|p| format!("{:?}", p))
            .unwrap_or_else(|| "Unknown".to_string());
        let wish_name = wish
            .status
            .as_ref()
            .and_then(|s| s.name.as_ref())
            .cloned()
            .unwrap_or_else(|| "-".to_string());
        let age = wish
            .metadata
            .creation_timestamp
            .as_ref()
            .map(|ts| {
                let duration = chrono::Utc::now().signed_duration_since(ts.0);
                format_duration(duration)
            })
            .unwrap_or_else(|| "-".to_string());

        println!("{:<30} {:<15} {:<30} {:<10}", name, phase, wish_name, age);
    }

    Ok(())
}

async fn describe_wish(client: &Client, namespace: &str, name: &str) -> Result<()> {
    let api: Api<Wish> = Api::namespaced(client.clone(), namespace);
    let wish = api.get(name).await?;

    println!("Name:      {}", wish.name_any());
    println!("Namespace: {}", namespace);
    println!();
    println!("Spec:");
    println!("  Wish:        {}", wish.spec.wish);
    println!("  Auto-fulfill: {}", wish.spec.auto_fulfill);
    println!("  Dry-run:     {}", wish.spec.dry_run);
    println!();

    if let Some(status) = wish.status {
        println!("Status:");
        println!(
            "  Phase:     {:?}",
            status.phase.unwrap_or(wish_system::WishPhase::Requested)
        );
        
        if let Some(wish_name) = status.name {
            println!("  Name:      {}", wish_name);
        }

        if let Some(plan) = status.plan {
            println!();
            println!("  Execution Plan:");
            println!("    Reasoning: {}", plan.reasoning);
            println!("    Commands ({}):", plan.commands.len());
            for (i, cmd) in plan.commands.iter().enumerate() {
                println!("      {}. Type: {:?}", i + 1, cmd.command_type);
                println!("         Command: {}", cmd.command);
                if let Some(yaml) = &cmd.yaml {
                    println!("         YAML:");
                    for line in yaml.lines() {
                        println!("           {}", line);
                    }
                }
            }
        }

        if let Some(dry_run_results) = status.dry_run_results {
            println!();
            println!("  Dry-run Results:");
            for (i, result) in dry_run_results.iter().enumerate() {
                println!("    {}. Command: {}", i + 1, result.command);
                println!("       Expected: {}", result.expected_outcome);
            }
        }

        if status.fulfilled {
            println!();
            println!("  Fulfilled: true");
            if let Some(fulfilled_at) = status.fulfilled_at {
                println!("  Fulfilled At: {}", fulfilled_at);
            }
        }

        if let Some(error) = status.error {
            println!();
            println!("  Error: {}", error);
        }
    }

    Ok(())
}

async fn fulfill_wish(client: &Client, namespace: &str, name: &str) -> Result<()> {
    let api: Api<Wish> = Api::namespaced(client.clone(), namespace);
    
    // Get current wish
    let wish = api.get(name).await?;
    
    // Check if already fulfilled
    if let Some(status) = &wish.status {
        if status.fulfilled {
            println!("Wish '{}' is already fulfilled", name);
            return Ok(());
        }
        
        if !matches!(status.phase, Some(wish_system::WishPhase::Granted)) {
            println!("Wish '{}' is not in Granted state (current: {:?})", name, status.phase);
            return Ok(());
        }
    } else {
        println!("Wish '{}' has no status yet", name);
        return Ok(());
    }

    // Patch to disable dry-run
    let patch = json!({
        "spec": {
            "dryRun": false
        }
    });

    api.patch(name, &PatchParams::default(), &Patch::Merge(&patch))
        .await?;

    println!("Wish '{}' marked for fulfillment", name);
    println!("The wish-fulfiller controller will execute it shortly");

    Ok(())
}

async fn delete_wish(client: &Client, namespace: &str, name: &str) -> Result<()> {
    let api: Api<Wish> = Api::namespaced(client.clone(), namespace);
    api.delete(name, &DeleteParams::default()).await?;

    println!("Wish '{}' deleted", name);

    Ok(())
}

async fn configure_llm(
    client: &Client,
    namespace: &str,
    endpoint: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    show: bool,
) -> Result<()> {
    let cm_api: Api<ConfigMap> = Api::namespaced(client.clone(), namespace);
    let secret_api: Api<Secret> = Api::namespaced(client.clone(), namespace);

    if show {
        // Show current configuration
        match cm_api.get("wish-grantor-config").await {
            Ok(cm) => {
                println!("Current LLM Configuration (namespace: {}):", namespace);
                println!();
                if let Some(data) = cm.data {
                    println!("  Endpoint: {}", data.get("llmEndpoint").unwrap_or(&"-".to_string()));
                    println!("  Model:    {}", data.get("llmModel").unwrap_or(&"-".to_string()));

                    if let Some(secret_name) = data.get("credentialsSecretName") {
                        println!("  API Key:  (stored in secret '{}')", secret_name);
                    } else {
                        println!("  API Key:  (not configured)");
                    }
                } else {
                    println!("  No configuration found");
                }
            }
            Err(_) => {
                println!("No LLM configuration found in namespace '{}'", namespace);
                println!("Use 'kubectl wish configure' to set it up.");
            }
        }
        return Ok(());
    }

    // Update configuration
    let mut changes = Vec::new();
    let mut data = BTreeMap::new();

    // Get existing config first
    if let Ok(existing_cm) = cm_api.get("wish-grantor-config").await {
        if let Some(existing_data) = existing_cm.data {
            data = existing_data;
        }
    }

    if let Some(ep) = endpoint {
        data.insert("llmEndpoint".to_string(), ep.clone());
        changes.push(format!("endpoint = {}", ep));
    }

    if let Some(mdl) = model {
        data.insert("llmModel".to_string(), mdl.clone());
        changes.push(format!("model = {}", mdl));
    }

    if changes.is_empty() && api_key.is_none() {
        println!("No changes specified. Use --help to see available options.");
        return Ok(());
    }

    // Update ConfigMap
    if !changes.is_empty() {
        let cm = ConfigMap {
            metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                name: Some("wish-grantor-config".to_string()),
                namespace: Some(namespace.to_string()),
                ..Default::default()
            },
            data: Some(data),
            ..Default::default()
        };

        let pp = PatchParams::apply("kubectl-wish").force();
        let patch = Patch::Apply(&cm);
        cm_api.patch("wish-grantor-config", &pp, &patch).await?;

        println!("✓ Updated LLM configuration:");
        for change in &changes {
            println!("  - {}", change);
        }
    }

    // Update API key if provided
    if let Some(key) = api_key {
        let mut secret_data = BTreeMap::new();
        secret_data.insert("apiKey".to_string(), k8s_openapi::ByteString(key.as_bytes().to_vec()));

        let secret = Secret {
            metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                name: Some("llm-credentials".to_string()),
                namespace: Some(namespace.to_string()),
                ..Default::default()
            },
            data: Some(secret_data),
            ..Default::default()
        };

        let pp = PatchParams::apply("kubectl-wish");
        let patch = Patch::Apply(&secret);
        secret_api.patch("llm-credentials", &pp, &patch).await?;

        println!("✓ Updated API key in secret 'llm-credentials'");

        // Update ConfigMap to reference the secret
        let mut cm_data = BTreeMap::new();
        if let Ok(existing_cm) = cm_api.get("wish-grantor-config").await {
            if let Some(existing_data) = existing_cm.data {
                cm_data = existing_data;
            }
        }
        cm_data.insert("credentialsSecretName".to_string(), "llm-credentials".to_string());
        cm_data.insert("credentialsSecretKey".to_string(), "apiKey".to_string());

        let cm = ConfigMap {
            metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                name: Some("wish-grantor-config".to_string()),
                namespace: Some(namespace.to_string()),
                ..Default::default()
            },
            data: Some(cm_data),
            ..Default::default()
        };

        let pp = PatchParams::apply("kubectl-wish").force();
        let patch = Patch::Apply(&cm);
        cm_api.patch("wish-grantor-config", &pp, &patch).await?;
    }

    println!();
    println!("Configuration updated successfully!");
    println!("The wish-grantor will automatically retry any failed wishes with connection errors.");

    Ok(())
}

fn format_duration(duration: chrono::Duration) -> String {
    let days = duration.num_days();
    let hours = duration.num_hours() % 24;
    let minutes = duration.num_minutes() % 60;

    if days > 0 {
        format!("{}d", days)
    } else if hours > 0 {
        format!("{}h", hours)
    } else {
        format!("{}m", minutes)
    }
}
