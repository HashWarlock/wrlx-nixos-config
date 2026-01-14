use std::collections::HashMap;
use tonic::{Request, Response, Status};

use crate::db::{layer, DbPool, MemoryRepository};
use super::health::proto::memory_service_server::MemoryService;
use super::health::proto::{
    ConfirmForgetResponse, ForgetCandidate, ForgetCandidatesResponse, ForgetDecision,
    GetForgetCandidatesRequest, MemoryEntry, MemoryLayer, MemoryQuery, MemoryQueryResponse,
    PinRequest, PinResponse, RefreshFactsRequest, StoreMemoryRequest, StoreMemoryResponse,
};

pub struct MemoryServiceImpl {
    repo: MemoryRepository,
}

impl MemoryServiceImpl {
    pub fn new(db: DbPool) -> Self {
        Self {
            repo: MemoryRepository::new(db),
        }
    }

    /// Convert a database MemoryRecord to a proto MemoryEntry
    fn record_to_entry(record: crate::db::MemoryRecord) -> MemoryEntry {
        let metadata: HashMap<String, String> = serde_json::from_str(&record.metadata_json)
            .unwrap_or_default();

        MemoryEntry {
            id: record.id,
            layer: record.layer,
            content: record.content,
            metadata,
            created_at_ms: record.created_at_ms,
            last_accessed_ms: record.last_accessed_ms,
            expires_at_ms: record.expires_at_ms.unwrap_or(0),
            pinned: record.pinned,
            access_count: record.access_count,
        }
    }

    /// Convert proto MemoryLayer enum to i32
    fn layer_to_i32(layer: MemoryLayer) -> i32 {
        match layer {
            MemoryLayer::Unspecified => 0,
            MemoryLayer::Working => 1,
            MemoryLayer::Archive => 2,
            MemoryLayer::Facts => 3,
            MemoryLayer::Preferences => 4,
        }
    }

    /// Determine the reason a memory is a forget candidate
    fn get_forget_reason(record: &crate::db::MemoryRecord, now_ms: i64) -> (String, bool) {
        if let Some(expires) = record.expires_at_ms {
            if expires < now_ms {
                return ("Expired".to_string(), true);
            }
        }

        // Working layer auto-forgets, others need confirmation
        let auto_forget = record.layer == layer::WORKING;
        ("Not accessed in 30+ days".to_string(), auto_forget)
    }
}

#[tonic::async_trait]
impl MemoryService for MemoryServiceImpl {
    async fn query(
        &self,
        request: Request<MemoryQuery>,
    ) -> Result<Response<MemoryQueryResponse>, Status> {
        let req = request.into_inner();
        let layers: Vec<i32> = req.layers.iter().map(|&l| Self::layer_to_i32(MemoryLayer::try_from(l).unwrap_or(MemoryLayer::Unspecified))).collect();
        let limit = if req.limit > 0 { req.limit } else { 50 };

        let records = if !req.search_text.is_empty() {
            self.repo.search(&req.search_text, &layers, limit)
        } else {
            self.repo.query(&layers, limit)
        }
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let entries: Vec<MemoryEntry> = records
            .into_iter()
            .map(Self::record_to_entry)
            .collect();

        Ok(Response::new(MemoryQueryResponse {
            entries: entries.clone(),
            total_count: entries.len() as i32,
        }))
    }

    async fn store(
        &self,
        request: Request<StoreMemoryRequest>,
    ) -> Result<Response<StoreMemoryResponse>, Status> {
        let req = request.into_inner();
        let layer = Self::layer_to_i32(MemoryLayer::try_from(req.layer).unwrap_or(MemoryLayer::Working));
        let metadata_json = serde_json::to_string(&req.metadata)
            .map_err(|e| Status::invalid_argument(format!("Invalid metadata: {}", e)))?;

        let ttl_ms = if req.ttl_ms > 0 { Some(req.ttl_ms) } else { None };

        let id = self.repo
            .store(layer, &req.content, &metadata_json, ttl_ms)
            .map_err(|e| Status::internal(format!("Store error: {}", e)))?;

        tracing::debug!("Stored memory {} in layer {}", id, layer);

        Ok(Response::new(StoreMemoryResponse { id }))
    }

    async fn get_forget_candidates(
        &self,
        _request: Request<GetForgetCandidatesRequest>,
    ) -> Result<Response<ForgetCandidatesResponse>, Status> {
        let records = self.repo
            .get_forget_candidates()
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let now_ms = chrono::Utc::now().timestamp_millis();

        let candidates: Vec<ForgetCandidate> = records
            .into_iter()
            .map(|record| {
                let (reason, auto_forget) = Self::get_forget_reason(&record, now_ms);
                ForgetCandidate {
                    entry: Some(Self::record_to_entry(record)),
                    reason,
                    auto_forget,
                }
            })
            .collect();

        Ok(Response::new(ForgetCandidatesResponse { candidates }))
    }

    async fn confirm_forget(
        &self,
        request: Request<ForgetDecision>,
    ) -> Result<Response<ConfirmForgetResponse>, Status> {
        let req = request.into_inner();

        // Update kept items (reset decay by updating access time)
        let mut kept_count = 0;
        for id in &req.keep_ids {
            if self.repo.update_access(id).is_ok() {
                kept_count += 1;
            }
        }

        // Delete forgotten items
        let deleted_count = self.repo
            .delete_many(&req.forget_ids)
            .map_err(|e| Status::internal(format!("Delete error: {}", e)))? as i32;

        tracing::info!("Memory cleanup: {} deleted, {} kept", deleted_count, kept_count);

        Ok(Response::new(ConfirmForgetResponse {
            deleted_count,
            kept_count,
        }))
    }

    async fn pin(
        &self,
        request: Request<PinRequest>,
    ) -> Result<Response<PinResponse>, Status> {
        let req = request.into_inner();
        let success = self.repo
            .pin(&req.id, req.pinned)
            .map_err(|e| Status::internal(format!("Pin error: {}", e)))?;

        if success {
            tracing::debug!("Memory {} pinned={}", req.id, req.pinned);
        }

        Ok(Response::new(PinResponse { success }))
    }

    async fn refresh_facts(
        &self,
        _request: Request<RefreshFactsRequest>,
    ) -> Result<Response<MemoryQueryResponse>, Status> {
        // Query current facts
        let facts = self.repo
            .query(&[layer::FACTS], 100)
            .map_err(|e| Status::internal(format!("Query error: {}", e)))?;

        // For now, just return current facts
        // In a full implementation, this would refresh from system state
        let entries: Vec<MemoryEntry> = facts
            .into_iter()
            .map(Self::record_to_entry)
            .collect();

        Ok(Response::new(MemoryQueryResponse {
            entries: entries.clone(),
            total_count: entries.len() as i32,
        }))
    }
}
