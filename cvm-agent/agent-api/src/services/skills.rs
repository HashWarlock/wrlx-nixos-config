use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use futures::Stream;
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use crate::context::ServiceContext;
use crate::skills::{SkillsLoader, SkillMatcher, WorkflowExecutor, Skill, SkillRegistry};
use super::health::proto::skills_service_server::SkillsService;
use super::health::proto::{
    skill_progress, CreateSkillRequest, DeleteSkillRequest, DeleteSkillResponse,
    ExecuteSkillRequest, GetSkillRequest, ListSkillsRequest, ListSkillsResponse,
    MatchSkillsRequest, MatchSkillsResponse, SkillMatch, SkillProgress, SkillResponse,
    SkillSummary, SkillType, UpdateSkillRequest,
};

pub struct SkillsServiceImpl {
    loader: Arc<RwLock<SkillsLoader>>,
    registry: Arc<SkillRegistry>,
    ctx: ServiceContext,
}

impl SkillsServiceImpl {
    pub async fn new(skills_dir: &str, registry: Arc<SkillRegistry>, ctx: ServiceContext) -> Self {
        let mut loader = SkillsLoader::new(skills_dir);
        if let Err(e) = loader.load_all().await {
            tracing::error!("Failed to load skills: {}", e);
        }

        Self {
            loader: Arc::new(RwLock::new(loader)),
            registry,
            ctx,
        }
    }
}

#[tonic::async_trait]
impl SkillsService for SkillsServiceImpl {
    type ExecuteStream = Pin<Box<dyn Stream<Item = Result<SkillProgress, Status>> + Send>>;

    async fn list(
        &self,
        request: Request<ListSkillsRequest>,
    ) -> Result<Response<ListSkillsResponse>, Status> {
        let req = request.into_inner();
        let loader = self.loader.read().await;

        let skills: Vec<SkillSummary> = loader
            .list()
            .into_iter()
            .filter(|s| {
                req.type_filter == SkillType::SkillAll as i32 ||
                (req.type_filter == SkillType::SkillInstruction as i32 && matches!(s, Skill::Instruction(_))) ||
                (req.type_filter == SkillType::SkillWorkflow as i32 && matches!(s, Skill::Workflow(_)))
            })
            .map(skill_to_summary)
            .collect();

        Ok(Response::new(ListSkillsResponse { skills }))
    }

    async fn get(
        &self,
        request: Request<GetSkillRequest>,
    ) -> Result<Response<SkillResponse>, Status> {
        let req = request.into_inner();
        let loader = self.loader.read().await;

        let skill = loader
            .get(&req.name)
            .ok_or_else(|| Status::not_found(format!("Skill not found: {}", req.name)))?;

        Ok(Response::new(skill_to_response(skill)))
    }

    async fn create(
        &self,
        _request: Request<CreateSkillRequest>,
    ) -> Result<Response<SkillResponse>, Status> {
        Err(Status::unimplemented("Skill creation not yet implemented"))
    }

    async fn update(
        &self,
        _request: Request<UpdateSkillRequest>,
    ) -> Result<Response<SkillResponse>, Status> {
        Err(Status::unimplemented("Skill update not yet implemented"))
    }

    async fn delete(
        &self,
        _request: Request<DeleteSkillRequest>,
    ) -> Result<Response<DeleteSkillResponse>, Status> {
        Err(Status::unimplemented("Skill deletion not yet implemented"))
    }

    async fn execute(
        &self,
        request: Request<ExecuteSkillRequest>,
    ) -> Result<Response<Self::ExecuteStream>, Status> {
        let req = request.into_inner();
        let loader = self.loader.read().await;

        let skill = loader
            .get(&req.name)
            .ok_or_else(|| Status::not_found(format!("Skill not found: {}", req.name)))?;

        let workflow = match skill {
            Skill::Workflow(w) => w.clone(),
            Skill::Instruction(_) => {
                return Err(Status::invalid_argument("Cannot execute instruction skill"));
            }
        };

        drop(loader); // Release read lock before spawning

        let (tx, rx) = mpsc::channel(128);
        let parameters: HashMap<String, String> = req.parameters.into_iter().collect();
        let registry = self.registry.clone();
        let ctx = self.ctx.clone();

        tokio::spawn(async move {
            let (exec_tx, mut exec_rx) = mpsc::channel(128);
            let executor = WorkflowExecutor::new(registry, ctx);

            let exec_handle = tokio::spawn(async move {
                executor.execute(&workflow, parameters, exec_tx).await
            });

            while let Some((step_num, total, desc, output, error, completed)) = exec_rx.recv().await {
                let result = if let Some(e) = error {
                    Some(skill_progress::Result::Error(e))
                } else if let Some(o) = output {
                    Some(skill_progress::Result::Output(o))
                } else {
                    None
                };

                let progress = SkillProgress {
                    step_number: step_num,
                    total_steps: total,
                    step_description: desc,
                    result,
                    completed,
                };

                if tx.send(Ok(progress)).await.is_err() {
                    break;
                }
            }

            let _ = exec_handle.await;
        });

        let stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }

    async fn r#match(
        &self,
        request: Request<MatchSkillsRequest>,
    ) -> Result<Response<MatchSkillsResponse>, Status> {
        let req = request.into_inner();
        let loader = self.loader.read().await;

        let skills: Vec<&Skill> = loader.list();
        let matches = SkillMatcher::match_skills(&req.input, &skills);

        let response_matches: Vec<SkillMatch> = matches
            .into_iter()
            .map(|m| {
                let skill_type = loader.get(&m.skill_name).map(|s| {
                    match s {
                        Skill::Instruction(_) => SkillType::SkillInstruction as i32,
                        Skill::Workflow(_) => SkillType::SkillWorkflow as i32,
                    }
                }).unwrap_or(SkillType::SkillAll as i32);

                SkillMatch {
                    name: m.skill_name,
                    r#type: skill_type,
                    relevance: m.relevance,
                    matched_trigger: m.matched_trigger,
                }
            })
            .collect();

        Ok(Response::new(MatchSkillsResponse { matches: response_matches }))
    }
}

fn skill_to_summary(skill: &Skill) -> SkillSummary {
    SkillSummary {
        name: skill.name().to_string(),
        description: skill.description().to_string(),
        r#type: match skill {
            Skill::Instruction(_) => SkillType::SkillInstruction as i32,
            Skill::Workflow(_) => SkillType::SkillWorkflow as i32,
        },
        triggers: skill.triggers().to_vec(),
        source: skill.source().to_string(),
    }
}

fn skill_to_response(skill: &Skill) -> SkillResponse {
    let content = match skill {
        Skill::Instruction(i) => i.content.clone(),
        Skill::Workflow(w) => serde_yaml::to_string(w).unwrap_or_default(),
    };

    SkillResponse {
        name: skill.name().to_string(),
        description: skill.description().to_string(),
        r#type: match skill {
            Skill::Instruction(_) => SkillType::SkillInstruction as i32,
            Skill::Workflow(_) => SkillType::SkillWorkflow as i32,
        },
        triggers: skill.triggers().to_vec(),
        content,
        source: skill.source().to_string(),
    }
}
