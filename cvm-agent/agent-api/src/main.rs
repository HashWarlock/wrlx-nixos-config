mod llm;
mod risk;
mod services;

use std::net::SocketAddr;
use std::sync::Arc;
use tonic::transport::Server;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use llm::RedpillClient;
use services::health::proto::chat_service_server::ChatServiceServer;
use services::health::proto::health_service_server::HealthServiceServer;
use services::health::proto::shell_service_server::ShellServiceServer;
use services::{ChatServiceImpl, HealthServiceImpl, ShellServiceImpl};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let addr: SocketAddr = "0.0.0.0:8080".parse()?;
    tracing::info!("CVM Agent API listening on {}", addr);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_headers(Any)
        .allow_methods(Any);

    // Initialize LLM client
    let llm_client = match RedpillClient::new() {
        Ok(client) => {
            tracing::info!("Redpill LLM client initialized");
            Arc::new(client)
        }
        Err(e) => {
            tracing::warn!("LLM client not configured: {}. Chat service will return errors.", e);
            Arc::new(RedpillClient::new_dummy())
        }
    };

    let health_service = HealthServiceImpl;
    let shell_service = ShellServiceImpl;
    let chat_service = ChatServiceImpl::new(llm_client);

    Server::builder()
        .accept_http1(true)
        .layer(cors)
        .layer(tonic_web::GrpcWebLayer::new())
        .add_service(HealthServiceServer::new(health_service))
        .add_service(ShellServiceServer::new(shell_service))
        .add_service(ChatServiceServer::new(chat_service))
        .serve(addr)
        .await?;

    Ok(())
}
