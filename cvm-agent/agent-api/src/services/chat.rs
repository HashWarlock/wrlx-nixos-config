use std::pin::Pin;
use std::sync::Arc;
use futures::Stream;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use super::health::proto::chat_service_server::ChatService;
use super::health::proto::{chat_response, ChatRequest, ChatResponse};
use crate::llm::{RedpillClient, redpill::Message};

pub struct ChatServiceImpl {
    llm: Arc<RedpillClient>,
}

impl ChatServiceImpl {
    pub fn new(llm: Arc<RedpillClient>) -> Self {
        Self { llm }
    }
}

#[tonic::async_trait]
impl ChatService for ChatServiceImpl {
    type SendMessageStream = Pin<Box<dyn Stream<Item = Result<ChatResponse, Status>> + Send>>;

    async fn send_message(
        &self,
        request: Request<ChatRequest>,
    ) -> Result<Response<Self::SendMessageStream>, Status> {
        let req = request.into_inner();
        let message = req.message;

        let (tx, rx) = mpsc::channel(128);
        let llm = self.llm.clone();

        tokio::spawn(async move {
            let (llm_tx, mut llm_rx) = mpsc::channel(128);

            let messages = vec![
                Message {
                    role: "user".to_string(),
                    content: message,
                },
            ];

            let llm_handle = {
                let llm = llm.clone();
                tokio::spawn(async move {
                    if let Err(e) = llm.chat_stream(messages, llm_tx).await {
                        tracing::error!("LLM error: {}", e);
                    }
                })
            };

            while let Some(result) = llm_rx.recv().await {
                match result {
                    Ok(text) => {
                        let _ = tx
                            .send(Ok(ChatResponse {
                                response: Some(chat_response::Response::Text(text)),
                                done: false,
                            }))
                            .await;
                    }
                    Err(e) => {
                        let _ = tx
                            .send(Ok(ChatResponse {
                                response: Some(chat_response::Response::Error(e.to_string())),
                                done: true,
                            }))
                            .await;
                    }
                }
            }

            let _ = llm_handle.await;

            let _ = tx
                .send(Ok(ChatResponse {
                    response: None,
                    done: true,
                }))
                .await;
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }
}
