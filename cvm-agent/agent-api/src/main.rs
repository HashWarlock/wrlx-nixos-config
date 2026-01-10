mod db;
mod llm;
mod risk;
mod services;
mod skills;

use std::net::SocketAddr;
use std::sync::Arc;
use tonic::transport::Server;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use llm::RedpillClient;
use services::health::proto::chat_service_server::ChatServiceServer;
use services::health::proto::git_ops_service_server::GitOpsServiceServer;
use services::health::proto::gui_service_server::GuiServiceServer;
use services::health::proto::health_service_server::HealthServiceServer;
use services::health::proto::nix_ops_service_server::NixOpsServiceServer;
use services::health::proto::shell_service_server::ShellServiceServer;
use services::health::proto::skills_service_server::SkillsServiceServer;
use services::health::proto::voice_service_server::VoiceServiceServer;
use services::{ChatServiceImpl, GitOpsServiceImpl, GUIServiceImpl, HealthServiceImpl, NixOpsServiceImpl, ShellServiceImpl, SkillsServiceImpl, VoiceServiceImpl};

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
    let nixops_service = NixOpsServiceImpl;
    let gitops_service = GitOpsServiceImpl;
    let gui_service = GUIServiceImpl::new();
    let voice_service = VoiceServiceImpl::new();
    let skills_service = SkillsServiceImpl::new("/app/cvm-agent/skills").await;

    tracing::info!("GUI service initialized (vision: {})",
        if gui_service.has_vision() { "enabled" } else { "disabled" });
    tracing::info!("Voice service initialized");
    tracing::info!("Skills service initialized");

    Server::builder()
        .accept_http1(true)
        .layer(cors)
        .layer(tonic_web::GrpcWebLayer::new())
        .add_service(HealthServiceServer::new(health_service))
        .add_service(ShellServiceServer::new(shell_service))
        .add_service(ChatServiceServer::new(chat_service))
        .add_service(NixOpsServiceServer::new(nixops_service))
        .add_service(GitOpsServiceServer::new(gitops_service))
        .add_service(GuiServiceServer::new(gui_service))
        .add_service(VoiceServiceServer::new(voice_service))
        .add_service(SkillsServiceServer::new(skills_service))
        .serve(addr)
        .await?;

    Ok(())
}
