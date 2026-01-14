use futures::{Stream, StreamExt, SinkExt};
use std::pin::Pin;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tonic::{Request, Response, Status, Streaming};

use super::health::proto::voice_service_server::VoiceService;
use super::health::proto::{
    AudioChunk, ListModelsRequest, ListModelsResponse, TranscriptChunk,
    VoiceStatusRequest, VoiceStatusResponse, WhisperModel,
};

pub struct VoiceServiceImpl {
    whisper_url: String,
}

impl VoiceServiceImpl {
    pub fn new() -> Self {
        let port = std::env::var("WHISPER_PORT").unwrap_or_else(|_| "8082".to_string());
        Self {
            whisper_url: format!("ws://127.0.0.1:{}/transcribe", port),
        }
    }

    async fn check_whisper_available(&self) -> bool {
        let status_url = self.whisper_url.replace("/transcribe", "/status");
        let http_url = status_url.replace("ws://", "http://");

        reqwest::get(&http_url)
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }
}

#[tonic::async_trait]
impl VoiceService for VoiceServiceImpl {
    type TranscribeStream = Pin<Box<dyn Stream<Item = Result<TranscriptChunk, Status>> + Send>>;

    async fn transcribe(
        &self,
        request: Request<Streaming<AudioChunk>>,
    ) -> Result<Response<Self::TranscribeStream>, Status> {
        let mut audio_stream = request.into_inner();
        let (tx, rx) = mpsc::channel(128);
        let whisper_url = self.whisper_url.clone();

        tokio::spawn(async move {
            // Connect to Whisper WebSocket
            let ws_stream = match connect_async(&whisper_url).await {
                Ok((stream, _)) => stream,
                Err(e) => {
                    let _ = tx.send(Ok(TranscriptChunk {
                        text: format!("Failed to connect to Whisper: {}", e),
                        is_partial: false,
                        confidence: 0.0,
                        start_ms: 0,
                        end_ms: 0,
                    })).await;
                    return;
                }
            };

            let (mut ws_tx, mut ws_rx) = ws_stream.split();

            // Forward audio chunks to Whisper
            let tx_clone = tx.clone();
            let audio_forward = tokio::spawn(async move {
                while let Some(Ok(chunk)) = audio_stream.next().await {
                    if ws_tx.send(Message::Binary(chunk.data.into())).await.is_err() {
                        break;
                    }
                    if chunk.is_final {
                        let _ = ws_tx.close().await;
                        break;
                    }
                }
                drop(tx_clone); // Signal completion
            });

            // Receive transcriptions from Whisper
            while let Some(msg) = ws_rx.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        // Parse Whisper response (assuming JSON format)
                        if let Ok(response) = serde_json::from_str::<WhisperResponse>(&text) {
                            let _ = tx.send(Ok(TranscriptChunk {
                                text: response.text,
                                is_partial: response.is_partial,
                                confidence: response.confidence,
                                start_ms: response.start_ms,
                                end_ms: response.end_ms,
                            })).await;
                        } else {
                            // Plain text response
                            let _ = tx.send(Ok(TranscriptChunk {
                                text,
                                is_partial: false,
                                confidence: 1.0,
                                start_ms: 0,
                                end_ms: 0,
                            })).await;
                        }
                    }
                    Ok(Message::Close(_)) => break,
                    Err(_) => break,
                    _ => {}
                }
            }

            let _ = audio_forward.await;
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }

    async fn list_models(
        &self,
        _request: Request<ListModelsRequest>,
    ) -> Result<Response<ListModelsResponse>, Status> {
        // List available Whisper models
        let models = vec![
            WhisperModel {
                name: "tiny".to_string(),
                available: true,
                size_bytes: 75_000_000,
            },
            WhisperModel {
                name: "base".to_string(),
                available: true,
                size_bytes: 142_000_000,
            },
            WhisperModel {
                name: "small".to_string(),
                available: true,
                size_bytes: 466_000_000,
            },
            WhisperModel {
                name: "medium".to_string(),
                available: true,
                size_bytes: 1_500_000_000,
            },
        ];

        Ok(Response::new(ListModelsResponse { models }))
    }

    async fn status(
        &self,
        _request: Request<VoiceStatusRequest>,
    ) -> Result<Response<VoiceStatusResponse>, Status> {
        let available = self.check_whisper_available().await;
        let model = std::env::var("WHISPER_MODEL").unwrap_or_else(|_| "base".to_string());

        Ok(Response::new(VoiceStatusResponse {
            available,
            current_model: model,
            is_processing: false,
        }))
    }
}

#[derive(serde::Deserialize)]
struct WhisperResponse {
    text: String,
    #[serde(default)]
    is_partial: bool,
    #[serde(default = "default_confidence")]
    confidence: f32,
    #[serde(default)]
    start_ms: i32,
    #[serde(default)]
    end_ms: i32,
}

fn default_confidence() -> f32 {
    1.0
}
