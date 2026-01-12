use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct RedpillClient {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

/// OpenAI-compatible chat completion request
#[derive(Serialize)]
struct ChatRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
    stream: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// OpenAI-compatible streaming response chunk
#[derive(Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Deserialize)]
struct StreamChoice {
    delta: Option<StreamDelta>,
}

#[derive(Deserialize)]
struct StreamDelta {
    content: Option<String>,
}

impl RedpillClient {
    pub fn new() -> Result<Self> {
        let api_key = std::env::var("REDPILL_API_KEY")
            .or_else(|_| std::env::var("ANTHROPIC_AUTH_TOKEN"))
            .map_err(|_| anyhow::anyhow!("REDPILL_API_KEY or ANTHROPIC_AUTH_TOKEN required"))?;

        let base_url = std::env::var("REDPILL_BASE_URL")
            .or_else(|_| std::env::var("ANTHROPIC_BASE_URL"))
            .unwrap_or_else(|_| "https://api.redpill.ai".to_string());

        let model = std::env::var("REDPILL_MODEL")
            .or_else(|_| std::env::var("ANTHROPIC_MODEL"))
            .unwrap_or_else(|_| "claude-3-5-sonnet-20241022".to_string());

        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            client,
            api_key,
            base_url,
            model,
        })
    }

    pub fn new_dummy() -> Self {
        Self {
            client: Client::new(),
            api_key: String::new(),
            base_url: String::new(),
            model: String::new(),
        }
    }

    pub async fn chat_stream(
        &self,
        messages: Vec<Message>,
        tx: mpsc::Sender<Result<String>>,
    ) -> Result<()> {
        if self.api_key.is_empty() {
            tx.send(Err(anyhow::anyhow!("LLM not configured. Set REDPILL_API_KEY."))).await.ok();
            return Ok(());
        }

        let request = ChatRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            messages,
            stream: true,
        };

        // Use OpenAI-compatible endpoint with Bearer auth (works with RedPill, OpenRouter, etc.)
        let response = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            tx.send(Err(anyhow::anyhow!("API error: {}", error_text))).await.ok();
            return Ok(());
        }

        let mut stream = response.bytes_stream();
        use futures::StreamExt;

        let mut buffer = String::new();
        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    tx.send(Err(anyhow::anyhow!("Stream error: {}", e))).await.ok();
                    return Err(e.into());
                }
            };
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Process complete SSE events (OpenAI format: "data: {...}\n\n")
            while let Some(pos) = buffer.find("\n") {
                let line = buffer[..pos].to_string();
                buffer = buffer[pos + 1..].to_string();

                // Skip empty lines
                if line.trim().is_empty() {
                    continue;
                }

                // Parse data lines
                if let Some(data) = line.strip_prefix("data: ") {
                    if data == "[DONE]" {
                        continue;
                    }
                    // Parse OpenAI-compatible streaming chunk
                    if let Ok(chunk) = serde_json::from_str::<StreamChunk>(data) {
                        if let Some(choice) = chunk.choices.first() {
                            if let Some(delta) = &choice.delta {
                                if let Some(content) = &delta.content {
                                    tx.send(Ok(content.clone())).await.ok();
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
