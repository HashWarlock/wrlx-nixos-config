use tonic::{Request, Response, Status};

use crate::db::{DbPool, LessonsRepository};
use super::health::proto::lessons_service_server::LessonsService;
use super::health::proto::{
    DeleteLessonRequest, DeleteLessonResponse, GetLessonRequest, LessonEntry,
    ListLessonsRequest, ListLessonsResponse, RecordLessonOutcomeRequest,
    RecordLessonOutcomeResponse, SearchLessonsRequest, SearchLessonsResponse,
    StoreLessonRequest, StoreLessonResponse, UpdateLessonRequest,
};

pub struct LessonsServiceImpl {
    repo: LessonsRepository,
}

impl LessonsServiceImpl {
    pub fn new(db: DbPool) -> Self {
        Self {
            repo: LessonsRepository::new(db),
        }
    }

    /// Convert a database LessonRecord to a proto LessonEntry
    fn record_to_entry(record: crate::db::LessonRecord) -> LessonEntry {
        // Calculate confidence before moving fields
        let confidence = record.confidence();
        LessonEntry {
            id: record.id,
            trigger_pattern: record.trigger_pattern,
            solution: record.solution,
            context: record.context,
            success_count: record.success_count,
            failure_count: record.failure_count,
            confidence,
            created_at_ms: record.created_at_ms,
            last_used_ms: record.last_used_ms,
        }
    }
}

#[tonic::async_trait]
impl LessonsService for LessonsServiceImpl {
    async fn search(
        &self,
        request: Request<SearchLessonsRequest>,
    ) -> Result<Response<SearchLessonsResponse>, Status> {
        let req = request.into_inner();
        let limit = if req.limit > 0 { req.limit } else { 10 };

        let records = self
            .repo
            .search(&req.query, limit)
            .map_err(|e| Status::internal(format!("Search error: {}", e)))?;

        // Filter by minimum confidence if specified
        let lessons: Vec<LessonEntry> = records
            .into_iter()
            .map(Self::record_to_entry)
            .filter(|l| req.min_confidence <= 0.0 || l.confidence >= req.min_confidence)
            .collect();

        Ok(Response::new(SearchLessonsResponse { lessons }))
    }

    async fn store(
        &self,
        request: Request<StoreLessonRequest>,
    ) -> Result<Response<StoreLessonResponse>, Status> {
        let req = request.into_inner();

        if req.trigger_pattern.is_empty() {
            return Err(Status::invalid_argument("trigger_pattern is required"));
        }
        if req.solution.is_empty() {
            return Err(Status::invalid_argument("solution is required"));
        }

        let context = if req.context.is_empty() {
            "{}".to_string()
        } else {
            req.context
        };

        let id = self
            .repo
            .store(&req.trigger_pattern, &req.solution, &context)
            .map_err(|e| Status::internal(format!("Store error: {}", e)))?;

        tracing::debug!("Stored lesson {} for pattern '{}'", id, req.trigger_pattern);

        Ok(Response::new(StoreLessonResponse { id }))
    }

    async fn get(
        &self,
        request: Request<GetLessonRequest>,
    ) -> Result<Response<LessonEntry>, Status> {
        let req = request.into_inner();

        let record = self
            .repo
            .get_by_id(&req.id)
            .map_err(|e| Status::internal(format!("Query error: {}", e)))?
            .ok_or_else(|| Status::not_found(format!("Lesson {} not found", req.id)))?;

        Ok(Response::new(Self::record_to_entry(record)))
    }

    async fn list(
        &self,
        request: Request<ListLessonsRequest>,
    ) -> Result<Response<ListLessonsResponse>, Status> {
        let req = request.into_inner();

        let all_records = self
            .repo
            .get_all(1000)
            .map_err(|e| Status::internal(format!("Query error: {}", e)))?;

        let total_count = all_records.len() as i32;
        let offset = req.offset.max(0) as usize;
        let limit = if req.limit > 0 { req.limit as usize } else { 50 };

        let lessons: Vec<LessonEntry> = all_records
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(Self::record_to_entry)
            .collect();

        Ok(Response::new(ListLessonsResponse {
            lessons,
            total_count,
        }))
    }

    async fn record_success(
        &self,
        request: Request<RecordLessonOutcomeRequest>,
    ) -> Result<Response<RecordLessonOutcomeResponse>, Status> {
        let req = request.into_inner();

        self.repo
            .record_success(&req.id)
            .map_err(|e| Status::internal(format!("Update error: {}", e)))?;

        // Get updated record
        let record = self
            .repo
            .get_by_id(&req.id)
            .map_err(|e| Status::internal(format!("Query error: {}", e)))?
            .ok_or_else(|| Status::not_found(format!("Lesson {} not found", req.id)))?;

        tracing::debug!("Recorded success for lesson {}", req.id);

        Ok(Response::new(RecordLessonOutcomeResponse {
            success: true,
            new_success_count: record.success_count,
            new_failure_count: record.failure_count,
            new_confidence: record.confidence(),
        }))
    }

    async fn record_failure(
        &self,
        request: Request<RecordLessonOutcomeRequest>,
    ) -> Result<Response<RecordLessonOutcomeResponse>, Status> {
        let req = request.into_inner();

        self.repo
            .record_failure(&req.id)
            .map_err(|e| Status::internal(format!("Update error: {}", e)))?;

        // Get updated record
        let record = self
            .repo
            .get_by_id(&req.id)
            .map_err(|e| Status::internal(format!("Query error: {}", e)))?
            .ok_or_else(|| Status::not_found(format!("Lesson {} not found", req.id)))?;

        tracing::debug!("Recorded failure for lesson {}", req.id);

        Ok(Response::new(RecordLessonOutcomeResponse {
            success: true,
            new_success_count: record.success_count,
            new_failure_count: record.failure_count,
            new_confidence: record.confidence(),
        }))
    }

    async fn update(
        &self,
        request: Request<UpdateLessonRequest>,
    ) -> Result<Response<LessonEntry>, Status> {
        let req = request.into_inner();

        if req.solution.is_empty() {
            return Err(Status::invalid_argument("solution is required"));
        }

        self.repo
            .update_solution(&req.id, &req.solution)
            .map_err(|e| Status::internal(format!("Update error: {}", e)))?;

        // Get updated record
        let record = self
            .repo
            .get_by_id(&req.id)
            .map_err(|e| Status::internal(format!("Query error: {}", e)))?
            .ok_or_else(|| Status::not_found(format!("Lesson {} not found", req.id)))?;

        tracing::debug!("Updated lesson {}", req.id);

        Ok(Response::new(Self::record_to_entry(record)))
    }

    async fn delete(
        &self,
        request: Request<DeleteLessonRequest>,
    ) -> Result<Response<DeleteLessonResponse>, Status> {
        let req = request.into_inner();

        let success = self
            .repo
            .delete(&req.id)
            .map_err(|e| Status::internal(format!("Delete error: {}", e)))?;

        if success {
            tracing::debug!("Deleted lesson {}", req.id);
        }

        Ok(Response::new(DeleteLessonResponse { success }))
    }
}
