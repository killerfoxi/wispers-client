use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use wispers_connect::{FileNodeStateStore, NodeStateStore, NodeStateStage, NodeStorage};

#[derive(Parser)]
#[command(name = "wconnect")]
#[command(about = "CLI for Wispers Connect nodes")]
struct Cli {
    /// Hub address (e.g., http://localhost:50051)
    #[arg(long, env = "WCONNECT_HUB", default_value = "http://localhost:50051")]
    hub: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Register this node using a registration token
    Register {
        /// The registration token from the integrator
        token: String,
    },
    /// List nodes in the connectivity group
    Nodes,
    /// Show current registration status
    Status,
    /// Clear stored credentials and state
    Logout,
}

fn get_storage() -> Result<NodeStorage<FileNodeStateStore>> {
    let store = FileNodeStateStore::with_app_name("wconnect")
        .context("could not determine config directory")?;
    Ok(NodeStorage::new(store))
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Register { token } => register(&cli.hub, &token).await,
        Command::Nodes => nodes(&cli.hub).await,
        Command::Status => status(),
        Command::Logout => logout(),
    }
}

async fn register(hub_addr: &str, token: &str) -> Result<()> {
    let storage = get_storage()?;

    // TODO: remove app/profile namespaces later
    let stage = storage
        .restore_or_init_node_state("unused", None::<String>)
        .context("failed to load node state")?;

    let pending = match stage {
        NodeStateStage::Pending(p) => p,
        NodeStateStage::Registered(r) => {
            let reg = r.registration();
            anyhow::bail!(
                "Already registered as node {} in group {}. Use 'wconnect logout' to clear.",
                reg.node_number,
                reg.connectivity_group_id
            );
        }
    };

    println!("Connecting to hub at {}...", hub_addr);
    println!("Registering with token {}...", token);

    let registered = pending
        .register(hub_addr, token)
        .await
        .context("registration failed")?;

    let reg = registered.registration();
    println!("Registration successful!");
    println!("  Connectivity group: {}", reg.connectivity_group_id);
    println!("  Node number: {}", reg.node_number);
    Ok(())
}

async fn nodes(hub_addr: &str) -> Result<()> {
    let storage = get_storage()?;
    let stage = storage
        .restore_or_init_node_state("unused", None::<String>)
        .context("failed to load node state")?;

    let registered = match stage {
        NodeStateStage::Registered(r) => r,
        NodeStateStage::Pending(_) => {
            anyhow::bail!("Not registered. Use 'wconnect register <token>' first.");
        }
    };

    let reg = registered.registration();
    let nodes = registered
        .list_nodes(hub_addr)
        .await
        .context("failed to list nodes")?;

    if nodes.is_empty() {
        println!("No nodes in connectivity group.");
    } else {
        println!("Nodes in connectivity group {}:", reg.connectivity_group_id);
        for node in nodes {
            let name = if node.name.is_empty() {
                "(unnamed)".to_string()
            } else {
                node.name
            };
            let you = if node.node_number == reg.node_number {
                " (you)"
            } else {
                ""
            };
            println!("  {}: {}{}", node.node_number, name, you);
        }
    }
    Ok(())
}

fn status() -> Result<()> {
    let storage = get_storage()?;
    let stage = storage
        .restore_or_init_node_state("unused", None::<String>)
        .context("failed to load node state")?;

    match stage {
        NodeStateStage::Registered(r) => {
            let reg = r.registration();
            println!("Registered:");
            println!("  Connectivity group: {}", reg.connectivity_group_id);
            println!("  Node number: {}", reg.node_number);
        }
        NodeStateStage::Pending(_) => {
            println!("Not registered.");
        }
    }
    Ok(())
}

fn logout() -> Result<()> {
    let storage = get_storage()?;
    let stage = storage
        .restore_or_init_node_state("unused", None::<String>)
        .context("failed to load node state")?;

    match stage {
        NodeStateStage::Registered(r) => {
            r.delete().context("failed to delete state")?;
            println!("Credentials cleared.");
        }
        NodeStateStage::Pending(p) => {
            // Delete pending state too
            let app = p.app_namespace().clone();
            let profile = p.profile_namespace().clone();
            drop(p);
            // Need to delete manually - PendingNodeState doesn't have delete
            let store = FileNodeStateStore::with_app_name("wconnect")
                .context("could not determine config directory")?;
            store.delete(&app, &profile).context("failed to delete state")?;
            println!("State cleared.");
        }
    }
    Ok(())
}
