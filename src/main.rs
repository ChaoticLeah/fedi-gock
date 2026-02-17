use anyhow::{Context, Result};
use futures::stream::StreamExt;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

mod client;

use client::{Client, ClientData, ClientInteractor};


#[derive(Debug, Deserialize)]
struct Config {
    instance_url: String,
    api_token: String,
    responses: Vec<String>,
}

fn load_config() -> Result<Config> {
    let config_str = std::fs::read_to_string("config.yaml")
        .context("Failed to read config.yaml")?;
    let config: Config = serde_yaml::from_str(&config_str)
        .context("Failed to parse config.yaml")?;
    Ok(config)
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = load_config().context("Config load failed")?;

    let client = Client { data: ClientData { instance_url: config.instance_url.clone(), api_token: config.api_token.clone() } };
    let mut watcher = client.watch_notifications().await.context("Failed to watch notifications")?;

    while let Some(notification) = watcher.recv().await {
        if notification.get("type").and_then(|v| v.as_str()) == Some("mention") {
            // Dont reply to bots
            if notification.get("account").and_then(|v| v.get("bot")).and_then(|v| v.as_bool()) == Some(true) {
                continue;
            }

            let account = notification.get("account").and_then(|v| v.get("acct")).and_then(|v| v.as_str()).unwrap_or("unknown");
            let default_response = "No Responses Set".to_string();
            let response = format!("@{} {}", account, config.responses.choose(&mut rand::thread_rng()).unwrap_or(&default_response));
            client.post(response, notification.get("status").and_then(|v| v.get("id").and_then(|v| v.as_str().map(|s| s.to_string())))).await.context("Failed to post response")?;
        }
    }

    Ok(())
}
