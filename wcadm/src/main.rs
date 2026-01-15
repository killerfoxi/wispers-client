use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use reqwest::blocking::Client;
use serde::Deserialize;

/// CLI tool for managing Wispers Connect domains.
#[derive(Parser)]
#[command(name = "wcadm", version, about)]
struct Cli {
    /// API key (can also be set via WC_API_KEY env var)
    #[arg(long, env = "WC_API_KEY", hide_env_values = true)]
    api_key: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List connectivity groups, or nodes within a group
    List {
        /// Connectivity group ID (if omitted, lists all groups)
        group_id: Option<String>,
    },
    /// Add a new connectivity group
    Add,
    /// Remove a connectivity group
    Remove {
        /// Connectivity group ID to remove
        group_id: String,
    },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    let api_key = ApiKey::parse(&cli.api_key)?;
    let base_url = api_key.base_url();
    let client = Client::new();

    match cli.command {
        Command::List { group_id: None } => list_groups(&client, &base_url, &cli.api_key),
        Command::List {
            group_id: Some(id),
        } => list_group(&client, &base_url, &cli.api_key, &id),
        Command::Add => add_group(&client, &base_url, &cli.api_key),
        Command::Remove { group_id } => remove_group(&client, &base_url, &cli.api_key, &group_id),
    }
}

/// Parsed API key with extracted environment.
struct ApiKey {
    env: String,
}

impl ApiKey {
    /// Parse an API key in the format `wc_{env}_{id}.{secret}`.
    fn parse(raw: &str) -> Result<Self> {
        let raw = raw.trim();
        let rest = raw
            .strip_prefix("wc_")
            .ok_or_else(|| anyhow!("API key must start with 'wc_'"))?;

        let underscore_pos = rest
            .find('_')
            .ok_or_else(|| anyhow!("invalid API key format"))?;

        let env = &rest[..underscore_pos];
        if env.is_empty() {
            bail!("API key environment is empty");
        }

        Ok(Self {
            env: env.to_string(),
        })
    }

    /// Map the environment to a base URL.
    fn base_url(&self) -> String {
        match self.env.as_str() {
            "local" => "http://localhost:16363".to_string(),
            "staging" => "https://staging.connect.wispers.dev".to_string(),
            "prod" => "https://connect.wispers.dev".to_string(),
            other => {
                eprintln!("warning: unknown environment '{other}', assuming staging");
                "https://staging.connect.wispers.dev".to_string()
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListGroupsResponse {
    #[serde(default)]
    connectivity_group_ids: Vec<String>,
}

fn list_groups(client: &Client, base_url: &str, api_key: &str) -> Result<()> {
    let url = format!("{base_url}/api/v1/connectivity-groups");

    let resp = client
        .get(&url)
        .bearer_auth(api_key)
        .send()
        .context("failed to send request")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        bail!("server returned {status}: {body}");
    }

    let data: ListGroupsResponse = resp.json().context("failed to parse response")?;

    if data.connectivity_group_ids.is_empty() {
        println!("No connectivity groups found.");
    } else {
        println!("Connectivity groups:");
        for id in data.connectivity_group_ids {
            println!("  {id}");
        }
    }

    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroupResponse {
    id: String,
    created_at: String,
    #[serde(default)]
    nodes: Vec<NodeResponse>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NodeResponse {
    id: String,
    name: Option<String>,
    last_seen_at: Option<String>,
    created_at: String,
}

fn list_group(client: &Client, base_url: &str, api_key: &str, group_id: &str) -> Result<()> {
    let url = format!("{base_url}/api/v1/connectivity-groups/{group_id}");

    let resp = client
        .get(&url)
        .bearer_auth(api_key)
        .send()
        .context("failed to send request")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        bail!("server returned {status}: {body}");
    }

    let data: GroupResponse = resp.json().context("failed to parse response")?;

    println!("Connectivity group: {}", data.id);
    println!("  Created: {}", data.created_at);
    if data.nodes.is_empty() {
        println!("  Nodes: (none)");
    } else {
        println!("  Nodes:");
        for node in &data.nodes {
            let name = node.name.as_deref().unwrap_or("(unnamed)");
            let last_seen = node
                .last_seen_at
                .as_deref()
                .unwrap_or("never");
            println!("    {} - {} (last seen: {})", node.id, name, last_seen);
        }
    }

    Ok(())
}

fn add_group(client: &Client, base_url: &str, api_key: &str) -> Result<()> {
    let url = format!("{base_url}/api/v1/connectivity-groups");

    let resp = client
        .post(&url)
        .bearer_auth(api_key)
        .send()
        .context("failed to send request")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        bail!("server returned {status}: {body}");
    }

    let data: GroupResponse = resp.json().context("failed to parse response")?;

    println!("Created connectivity group:");
    println!("  ID: {}", data.id);
    println!("  Created: {}", data.created_at);

    Ok(())
}

fn remove_group(client: &Client, base_url: &str, api_key: &str, group_id: &str) -> Result<()> {
    let url = format!("{base_url}/api/v1/connectivity-groups/{group_id}");

    let resp = client
        .delete(&url)
        .bearer_auth(api_key)
        .send()
        .context("failed to send request")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        bail!("server returned {status}: {body}");
    }

    println!("Deleted connectivity group: {group_id}");

    Ok(())
}
