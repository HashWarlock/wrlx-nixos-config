#[allow(unused)]
mod llm;
mod services;

use std::net::SocketAddr;
use tonic::transport::Server;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use services::health::proto::health_service_server::HealthServiceServer;
use services::health::proto::shell_service_server::ShellServiceServer;
use services::{HealthServiceImpl, ShellServiceImpl};

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

    let health_service = HealthServiceImpl;
    let shell_service = ShellServiceImpl;

    Server::builder()
        .accept_http1(true)
        .layer(cors)
        .layer(tonic_web::GrpcWebLayer::new())
        .add_service(HealthServiceServer::new(health_service))
        .add_service(ShellServiceServer::new(shell_service))
        .serve(addr)
        .await?;

    Ok(())
}
