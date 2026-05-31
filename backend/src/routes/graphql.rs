use actix_web::{HttpMessage, HttpRequest, HttpResponse, Result, web};
use async_graphql::Data;
use async_graphql::http::GraphiQLSource;
use async_graphql_actix_web::{GraphQLRequest, GraphQLResponse, GraphQLSubscription};
use clerk_rs::validators::authorizer::ClerkJwt;
use clerk_rs::validators::{authorizer::validate_jwt, jwks::MemoryCacheJwksProvider};
use log::{error, info};
use std::sync::Arc;

use crate::graphql::AppSchema;
use crate::service::ai::chat::sessions::ChatSessionStore;
use crate::service::ai::chat::types::ChatJobRegistry;
use crate::service::ai::client::AgentsClient;
use crate::service::ai::types::AiEventBus;
use crate::service::ai::vector_database::client::VectorDatabaseClient;
use crate::service::r2::R2Client;
use crate::service::turso::TursoClient;

fn infer_operation_name(query: &str) -> &str {
    query
        .split_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .find_map(|parts| match parts {
            ["query" | "mutation" | "subscription", name] => Some(*name),
            _ => None,
        })
        .unwrap_or("anonymous")
}

#[allow(clippy::too_many_arguments)]
pub async fn graphql_handler(
    schema: web::Data<AppSchema>,
    http_req: HttpRequest,
    turso: web::Data<Arc<TursoClient>>,
    agents: web::Data<Arc<AgentsClient>>,
    vector_db: web::Data<Arc<VectorDatabaseClient>>,
    events: web::Data<AiEventBus>,
    chat_jobs: web::Data<ChatJobRegistry>,
    chat_session_store: web::Data<Arc<ChatSessionStore>>,
    r2: web::Data<Arc<R2Client>>,
    req: GraphQLRequest,
) -> GraphQLResponse {
    let mut request = req.into_inner();
    let operation_name = request
        .operation_name
        .clone()
        .unwrap_or_else(|| infer_operation_name(&request.query).to_string());
    let query_preview = request
        .query
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    let auth = http_req.extensions().get::<ClerkJwt>().cloned();
    let auth_subject = auth
        .as_ref()
        .map(|jwt| jwt.sub.clone())
        .unwrap_or_else(|| "anonymous".to_string());

    info!(
        "GraphQL request: operation={} method={} path={} auth_subject={} query_preview={:?}",
        operation_name,
        http_req.method(),
        http_req.path(),
        auth_subject,
        query_preview
    );

    if let Some(jwt) = auth {
        request = request.data(jwt);
    }
    request = request.data(turso.get_ref().clone());
    request = request.data(agents.get_ref().clone());
    request = request.data(vector_db.get_ref().clone());
    request = request.data(events.get_ref().clone());
    request = request.data(chat_jobs.get_ref().clone());
    request = request.data(chat_session_store.get_ref().clone());
    request = request.data(r2.get_ref().clone());
    request = request.data(async_graphql::dataloader::DataLoader::new(
        crate::graphql::tags::TagLoader {
            turso: turso.get_ref().clone(),
        },
        tokio::spawn,
    ));

    let response = schema.execute(request).await;

    if response.errors.is_empty() {
        info!(
            "GraphQL success: operation={} auth_subject={} error_count=0",
            operation_name, auth_subject
        );
    } else {
        error!(
            "GraphQL failure: operation={} auth_subject={} errors={:?}",
            operation_name, auth_subject, response.errors
        );
    }

    response.into()
}

pub async fn graphiql() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(GraphiQLSource::build().endpoint("/graphql").finish()))
}

#[allow(clippy::too_many_arguments)]
pub async fn graphql_ws_handler(
    schema: web::Data<AppSchema>,
    http_req: HttpRequest,
    turso: web::Data<Arc<TursoClient>>,
    agents: web::Data<Arc<AgentsClient>>,
    vector_db: web::Data<Arc<VectorDatabaseClient>>,
    events: web::Data<AiEventBus>,
    chat_jobs: web::Data<ChatJobRegistry>,
    chat_session_store: web::Data<Arc<ChatSessionStore>>,
    r2: web::Data<Arc<R2Client>>,
    jwks: web::Data<Arc<MemoryCacheJwksProvider>>,
    payload: web::Payload,
) -> Result<HttpResponse> {
    let mut data = Data::default();

    if let Some(jwt) = http_req.extensions().get::<ClerkJwt>().cloned() {
        data.insert(jwt);
    }
    data.insert(turso.get_ref().clone());
    data.insert(agents.get_ref().clone());
    data.insert(vector_db.get_ref().clone());
    data.insert(events.get_ref().clone());
    data.insert(chat_jobs.get_ref().clone());
    data.insert(chat_session_store.get_ref().clone());
    data.insert(r2.get_ref().clone());

    GraphQLSubscription::new(schema.get_ref().clone())
        .with_data(data)
        .on_connection_init({
            let jwks = jwks.get_ref().clone();
            move |payload| {
                let jwks = jwks.clone();
                async move {
                    let mut data = Data::default();
                    if let Some(token) = payload
                        .get("authorization")
                        .and_then(|value| value.as_str())
                        .or_else(|| payload.get("token").and_then(|value| value.as_str()))
                    {
                        let token = token.trim().trim_start_matches("Bearer ").to_string();
                        let jwt = validate_jwt(&token, jwks)
                            .await
                            .map_err(|error| async_graphql::Error::new(error.to_string()))?;
                        data.insert(jwt);
                    }
                    Ok(data)
                }
            }
        })
        .start(&http_req, payload)
}
