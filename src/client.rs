use anyhow::{Context, Result};
use futures::stream::StreamExt;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};


#[derive(Debug, Serialize)]
struct PostRequest {
    status: String,
    in_reply_to_id: Option<String>,
    visibility: String,
}

pub struct ClientData {
    pub instance_url: String,
    pub api_token: String
}

pub struct Client {
    pub data: ClientData
}

pub trait ClientInteractor {
    async fn post(&self, message: String, to: Option<String>) -> Result<()>;
    async fn watch_notifications(
        &self
    ) -> Result<tokio::sync::mpsc::Receiver<serde_json::Value>>;
}

impl ClientInteractor for Client {
    async fn post(&self, message: String, to: Option<String>) -> Result<()> {
        let body = PostRequest {
            status: message.to_string(),
            in_reply_to_id: to,
            visibility: "public".to_string(),
        };

        let url = format!("{}/api/v1/statuses", self.data.instance_url);
        reqwest::Client::new()
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.data.api_token))
            .json(&body)
            .send()
            .await
            .context("POST request failed")?
            .json::<serde_json::Value>()
            .await
            .context("Failed to parse JSON response")?;
        Ok(())
    }


    async fn watch_notifications(
        &self
    ) -> Result<tokio::sync::mpsc::Receiver<serde_json::Value>> {
        let url = format!(
            "{}/api/v1/streaming/user/notification?access_token={}",
            self.data.instance_url, self.data.api_token
        );

        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .send()
            .await
            .context("GET request failed")?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.ok();
            anyhow::bail!("Failed to connect to streaming API: {} - {:?}", status, body);
        }

        let (tx, rx) = tokio::sync::mpsc::channel::<serde_json::Value>(64);

        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk_result) = stream.next().await {
                if let Ok(chunk) = chunk_result {
                    let chunk_str = String::from_utf8_lossy(&chunk);
                    buffer.push_str(&chunk_str);

                    while let Some(event_end) = buffer.find("\n\n") {
                        let event_block = buffer[..event_end].to_string();
                        buffer = buffer[event_end + 2..].to_string();

                        let mut event_data_lines = Vec::new();

                        for line in event_block.lines() {
                            if let Some(data) = line.strip_prefix("data:") {
                                event_data_lines.push(data.trim().to_string());
                            }
                        }

                        if !event_data_lines.is_empty() {
                            let complete_data = event_data_lines.join("\n");
                            if let Ok(event) = serde_json::from_str::<serde_json::Value>(&complete_data) {
                                if tx.send(event).await.is_err() {
                                    return; // listener dropped
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(rx)
    }
}



