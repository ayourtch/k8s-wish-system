use anyhow::Result;
use clap::{Parser, Subcommand};
use kube::{
    api::{Api, DeleteParams, ListParams, Patch, PatchParams, PostParams},
    Client, ResourceExt,
};
use serde_json::json;
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
        } => create_wish(&client, &namespace, &wish, auto_fulfill, !no_dry_run, name).await?,

        Commands::List => list_wishes(&client, &namespace).await?,

        Commands::Describe { name } => describe_wish(&client, &namespace, &name).await?,

        Commands::Fulfill { name } => fulfill_wish(&client, &namespace, &name).await?,

        Commands::Delete { name } => delete_wish(&client, &namespace, &name).await?,
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
