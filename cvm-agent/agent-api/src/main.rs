mod config;
mod context;
mod db;
mod llm;
mod risk;
mod services;
mod skills;

use std::net::SocketAddr;
use std::sync::Arc;
use skills::SkillRegistry;
use tonic::transport::Server;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use llm::RedpillClient;
use services::health::proto::chat_service_server::ChatServiceServer;
use services::health::proto::git_ops_service_server::GitOpsServiceServer;
use services::health::proto::gui_service_server::GuiServiceServer;
use services::health::proto::health_service_server::HealthServiceServer;
use services::health::proto::lessons_service_server::LessonsServiceServer;
use services::health::proto::memory_service_server::MemoryServiceServer;
use services::health::proto::nix_ops_service_server::NixOpsServiceServer;
use services::health::proto::setup_service_server::SetupServiceServer;
use services::health::proto::shell_service_server::ShellServiceServer;
use services::health::proto::skills_service_server::SkillsServiceServer;
use services::health::proto::voice_service_server::VoiceServiceServer;
use services::{ChatServiceImpl, GitOpsServiceImpl, GUIServiceImpl, HealthServiceImpl, LessonsServiceImpl, MemoryServiceImpl, NixOpsServiceImpl, SetupServiceImpl, ShellServiceImpl, SkillsServiceImpl, VoiceServiceImpl};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Initialize configuration
    let config = config::Config::init();
    tracing::info!("Configuration loaded");

    let addr: SocketAddr = config.server.listen_addr.parse()?;
    tracing::info!("CVM Agent API listening on {}", addr);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_headers(Any)
        .allow_methods(Any);

    // Initialize database
    let db_path = config.db.path.to_string_lossy();
    let db = db::init_db(&db_path).expect("Failed to initialize database");
    tracing::info!("Database initialized at {}", db_path);

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

    // Initialize skill registry with all built-in actions
    let mut registry = SkillRegistry::new();
    skills::register_all(&mut registry);
    let registry = Arc::new(registry);
    tracing::info!("Skill registry initialized with {} actions", registry.len());

    // Create service context for skill actions
    let service_ctx = context::ServiceContext::new(db.clone(), llm_client.clone());
    tracing::info!("Service context initialized");

    let health_service = HealthServiceImpl;
    let shell_service = ShellServiceImpl;
    let chat_service = ChatServiceImpl::new(llm_client);
    let nixops_service = NixOpsServiceImpl;
    let gitops_service = GitOpsServiceImpl;
    let gui_service = GUIServiceImpl::new();
    let voice_service = VoiceServiceImpl::new();
    let skills_dir = config.skills.dir.to_string_lossy();
    let skills_service = SkillsServiceImpl::new(&skills_dir, registry, service_ctx).await;
    let memory_service = MemoryServiceImpl::new(db.clone());
    let lessons_service = LessonsServiceImpl::new(db);
    let setup_service = SetupServiceImpl::new();

    tracing::info!("GUI service initialized (vision: {})",
        if gui_service.has_vision() { "enabled" } else { "disabled" });
    tracing::info!("Voice service initialized");
    tracing::info!("Skills service initialized");
    tracing::info!("Memory service initialized");
    tracing::info!("Lessons service initialized");
    tracing::info!("Setup service initialized (config_dir: {})", config.paths.config_dir.display());

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
        .add_service(MemoryServiceServer::new(memory_service))
        .add_service(LessonsServiceServer::new(lessons_service))
        .add_service(SetupServiceServer::new(setup_service))
        .serve(addr)
        .await?;

    Ok(())
}
