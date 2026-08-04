use std::sync::Arc;

use actix_web::{HttpRequest, HttpResponse, web};
use serde_json::Value;
use tracing::{error, info, warn};

use crate::service::countly::Countly;
use crate::service::db::client::Db;
use crate::service::r2::R2Client;
use crate::service::users::purge::{collect_r2_keys, delete_user_by_clerk_uuid};
use crate::service::webhooks::svix::verify_svix_signature;

pub fn deleted_user_id(payload: &Value) -> Option<&str> {
    if payload.get("type")?.as_str()? != "user.deleted" {
        return None;
    }
    payload.get("data")?.get("id")?.as_str()
}

pub fn created_user_id(payload: &Value) -> Option<&str> {
    if payload.get("type")?.as_str()? != "user.created" {
        return None;
    }
    payload.get("data")?.get("id")?.as_str()
}

fn header<'a>(req: &'a HttpRequest, name: &str) -> Option<&'a str> {
    req.headers().get(name)?.to_str().ok()
}

pub async fn clerk_webhook(
    req: HttpRequest,
    body: web::Bytes,
    db: web::Data<Arc<Db>>,
    r2: web::Data<Arc<R2Client>>,
    countly: web::Data<Option<Arc<Countly>>>,
) -> HttpResponse {
    let Ok(secret) = std::env::var("CLERK_WEBHOOK_SECRET") else {
        error!("CLERK_WEBHOOK_SECRET is not set; refusing the webhook");
        return HttpResponse::InternalServerError().finish();
    };

    let (Some(svix_id), Some(timestamp), Some(signature)) = (
        header(&req, "svix-id"),
        header(&req, "svix-timestamp"),
        header(&req, "svix-signature"),
    ) else {
        return HttpResponse::BadRequest().body("missing svix headers");
    };

    let now = chrono::Utc::now().timestamp();
    if let Err(err) = verify_svix_signature(&secret, svix_id, timestamp, signature, &body, now) {
        warn!(error = %err, "rejected a clerk webhook");
        return HttpResponse::Unauthorized().finish();
    }

    let Ok(payload) = serde_json::from_slice::<Value>(&body) else {
        return HttpResponse::BadRequest().body("body is not json");
    };

    if let Some(clerk_uuid) = created_user_id(&payload) {
        if let Some(countly) = countly.get_ref().as_ref() {
            countly
                .capture(clerk_uuid, "user_signed_up", serde_json::json!({}))
                .await;
        }
        return HttpResponse::Ok().finish();
    }

    let Some(clerk_uuid) = deleted_user_id(&payload) else {
        return HttpResponse::Ok().finish();
    };

    let pool = db.pool();

    let user_id =
        match sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE clerk_uuid = $1")
            .bind(clerk_uuid)
            .fetch_optional(pool)
            .await
        {
            Ok(Some(id)) => id,
            Ok(None) => return HttpResponse::Ok().finish(),
            Err(err) => {
                error!(error = %err, "failed to look up the deleted user");
                return HttpResponse::InternalServerError().finish();
            }
        };

    // The keys are only reachable through rows the delete is about to remove, so the
    // objects go first; a failure here returns 500 so Clerk retries rather than
    // silently leaking blobs.
    match collect_r2_keys(pool, &user_id).await {
        Ok(keys) => {
            for key in keys {
                if let Err(err) = r2.delete_object(&key).await {
                    error!(error = %err, key = %key, "failed to delete an r2 object");
                    return HttpResponse::InternalServerError().finish();
                }
            }
        }
        Err(err) => {
            error!(error = %err, "failed to collect r2 keys");
            return HttpResponse::InternalServerError().finish();
        }
    }

    match delete_user_by_clerk_uuid(pool, clerk_uuid).await {
        Ok(Some(id)) => {
            info!(user_id = %id, "purged a deleted clerk user");
            HttpResponse::Ok().finish()
        }
        Ok(None) => HttpResponse::Ok().finish(),
        Err(err) => {
            error!(error = %err, "failed to delete the user row");
            HttpResponse::InternalServerError().finish()
        }
    }
}
