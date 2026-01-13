//! gRPC client for Agent API

pub mod agent {
    tonic::include_proto!("cvm.agent");
}

use agent::chat_service_client::ChatServiceClient;
use agent::health_service_client::HealthServiceClient;
use agent::{ChatRequest, HealthRequest};
use anyhow::Result;
use futures::StreamExt;
use tonic::transport::Channel;

pub struct AgentClient {
    chat: ChatServiceClient<Channel>,
    health: HealthServiceClient<Channel>,
}

impl AgentClient {
    pub async fn connect(addr: &str) -> Result<Self> {
        let channel = Channel::from_shared(addr.to_string())?
            .connect()
            .await?;
        let chat = ChatServiceClient::new(channel.clone());
        let health = HealthServiceClient::new(channel);
        Ok(Self { chat, health })
    }

    /// Send a message and collect all streaming responses into a single string
    pub async fn send_message(
        &mut self,
        message: &str,
        conversation_id: Option<String>,
    ) -> Result<String> {
        let request = tonic::Request::new(ChatRequest {
            message: message.to_string(),
            conversation_id: conversation_id.unwrap_or_default(),
        });

        let response = self.chat.send_message(request).await?;
        let mut stream = response.into_inner();

        let mut full_response = String::new();

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(chat_response) => {
                    if let Some(response) = chat_response.response {
                        match response {
                            agent::chat_response::Response::Text(text) => {
                                full_response.push_str(&text);
                            }
                            agent::chat_response::Response::Error(err) => {
                                return Err(anyhow::anyhow!("Agent error: {}", err));
                            }
                        }
                    }
                    if chat_response.done {
                        break;
                    }
                }
                Err(e) => {
                    return Err(anyhow::anyhow!("Stream error: {}", e));
                }
            }
        }

        Ok(full_response)
    }

    /// Check if the agent service is healthy
    pub async fn health_check(&mut self) -> Result<(bool, String)> {
        let request = tonic::Request::new(HealthRequest {});
        let response = self.health.check(request).await?;
        let inner = response.into_inner();
        Ok((inner.healthy, inner.version))
    }
}
