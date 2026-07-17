use actix_web::{HttpMessage, HttpRequest, HttpResponse, web};
use clerk_rs::validators::authorizer::ClerkJwt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::service::ai::chat::assistance::{autocomplete, summary};
use crate::service::ai::client::AgentsClient;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutocompleteRequest {
    #[serde(default)]
    pub title: String,
    pub text: String,
}

#[derive(Serialize)]
pub struct AutocompleteResponse {
    pub completion: String,
}

#[derive(Deserialize)]
pub struct TransformRequest {
    pub text: String,
    pub action: String,
}

#[derive(Serialize)]
pub struct TransformResponse {
    pub result: String,
}

pub async fn autocomplete_handler(
    req: HttpRequest,
    body: web::Json<AutocompleteRequest>,
    agents: web::Data<Arc<AgentsClient>>,
) -> HttpResponse {
    let jwt = match req.extensions().get::<ClerkJwt>().cloned() {
        Some(jwt) => jwt,
        None => {
            return HttpResponse::Unauthorized().json(serde_json::json!({"error": "Unauthorized"}));
        }
    };
    let _ = jwt.sub;

    match autocomplete::complete(&agents, &body.title, &body.text).await {
        Ok(completion) => HttpResponse::Ok().json(AutocompleteResponse { completion }),
        Err(e) => {
            log::error!("Autocomplete error: {e}");
            HttpResponse::InternalServerError()
                .json(serde_json::json!({"error": "Autocomplete failed"}))
        }
    }
}

pub async fn transform_handler(
    req: HttpRequest,
    body: web::Json<TransformRequest>,
    agents: web::Data<Arc<AgentsClient>>,
) -> HttpResponse {
    let jwt = match req.extensions().get::<ClerkJwt>().cloned() {
        Some(jwt) => jwt,
        None => {
            return HttpResponse::Unauthorized().json(serde_json::json!({"error": "Unauthorized"}));
        }
    };
    let _ = jwt.sub;

    match summary::transform(&agents, &body.text, &body.action).await {
        Ok(result) => HttpResponse::Ok().json(TransformResponse { result }),
        Err(e) => {
            log::error!("Transform error: {e}");
            HttpResponse::InternalServerError()
                .json(serde_json::json!({"error": "Transform failed"}))
        }
    }
}
