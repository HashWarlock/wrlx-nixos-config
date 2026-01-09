use tonic::{Request, Response, Status};

pub mod proto {
    tonic::include_proto!("cvm.agent");
}

use proto::health_service_server::HealthService;
use proto::{HealthRequest, HealthResponse};

pub struct HealthServiceImpl;

#[tonic::async_trait]
impl HealthService for HealthServiceImpl {
    async fn check(
        &self,
        _request: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        let response = HealthResponse {
            healthy: true,
            version: env!("CARGO_PKG_VERSION").to_string(),
        };
        Ok(Response::new(response))
    }
}
