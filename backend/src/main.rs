use actix_cors::Cors;
use actix_web::{App, HttpServer, middleware::Logger, web};
use clerk_rs::validators::actix::ClerkMiddleware;
use log::info;
use std::sync::Arc;
use tokio::sync::broadcast;
use tradstry_backend::graphql;
use tradstry_backend::routes;
use tradstry_backend::service::ai::client::AgentsClient;
use tradstry_backend::service::ai::run_worker_loop;
use tradstry_backend::service::ai::vector_database::client::VectorDatabaseClient;
use tradstry_backend::service::auth::create_jwks_provider;
use tradstry_backend::service::brokerage::client::BrokerageClient;
use tradstry_backend::service::r2::R2Client;
use tradstry_backend::service::redis::client::RedisClient;
use tradstry_backend::service::turso::TursoClient;
use tradstry_backend::service::turso::TursoConfig;
fn cors_allowed_origins() -> Vec<String> {
    let defaults = [
        "http://localhost:3038",
        "http://127.0.0.1:3038",
        "http://localhost:3001",
        "http://127.0.0.1:3001",
    ];
    std::env::var("CORS_ALLOWED_ORIGINS")
        .ok()
        .map(|origins| {
            origins
                .split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|origins| !origins.is_empty())
        .unwrap_or_else(|| defaults.into_iter().map(str::to_owned).collect())
}
// Multi-threaded tokio runtime (not actix-rt's single-threaded `#[actix_web::main]`):
// libsql's embedded-replica `Database::connect()` calls `tokio::task::block_in_place`,
// which panics on a current-thread runtime. The codebase uses no `spawn_local`/`System`,
// so running actix-web under a multi-thread tokio runtime is safe.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
    dotenvy::dotenv().ok();
    env_logger::init();
    info!("Starting backend...");

    let config = TursoConfig::from_env()?;
    let turso_client = Arc::new(TursoClient::new(config).await?);
    let r2_client = Arc::new(R2Client::from_env()?);
    let agents_client = Arc::new(AgentsClient::from_env()?);
    let vector_database_client = Arc::new(VectorDatabaseClient::from_env()?);
    let brokerage_client = Arc::new(BrokerageClient::from_env()?);
    let redis_client = match RedisClient::from_env().await {
        Ok(c) => {
            info!("Redis cache enabled");
            Some(Arc::new(c))
        }
        Err(e) => {
            log::warn!("Redis unavailable, running without cache: {e}");
            None
        }
    };
    let checkpoint_saver =
        tradstry_backend::service::ai::chat::checkpoint::init_checkpoint_saver().await;
    // Warm the checkpoint saver's Postgres connection now so the first chat
    // message doesn't pay connection-setup latency. The saver is a separate
    // synchronous postgres connection from the sqlx pools health-checked below.
    {
        let saver = checkpoint_saver.clone();
        let _ = tokio::task::spawn_blocking(move || {
            saver.get(&langgraph::prelude::CheckpointConfig::new("__warmup__"))
        })
        .await;
        info!("Checkpoint saver connection warmed");
    }
    let memory_store = tradstry_backend::service::ai::chat::memory_store::init_memory_store().await;
    let chat_session_store =
        Arc::new(tradstry_backend::service::ai::chat::sessions::ChatSessionStore::from_env()?);
    turso_client.health_check().await?;
    vector_database_client.health_check().await?;
    vector_database_client.ensure_schema().await?;
    chat_session_store.ensure_table().await?;
    info!("Database healthy and migrations applied");
    info!(
        "Gemini client configured with model [{}]",
        agents_client.models_display()
    );
    info!(
        "Vector database client configured with embedding model {}",
        vector_database_client.config().voyage.embedding_model
    );
    let clerk_secret = std::env::var("CLERK_SECRET_KEY")?;
    let jwks_provider_data = Arc::new(create_jwks_provider(&clerk_secret));
    let (ai_events_tx, _) = broadcast::channel(256);
    let chat_jobs: tradstry_backend::service::ai::chat::types::ChatJobRegistry =
        Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
    info!("Clerk authentication configured");
    let schema = graphql::build_schema(
        brokerage_client.clone(),
        checkpoint_saver.clone(),
        memory_store.clone(),
        redis_client.clone(),
    );
    let allowed_origins = cors_allowed_origins();

    {
        let turso_client = turso_client.clone();
        let agents_client = agents_client.clone();
        let vector_database_client = vector_database_client.clone();
        let ai_events_tx = ai_events_tx.clone();
        tokio::spawn(async move {
            if let Err(error) = run_worker_loop(
                turso_client,
                agents_client,
                vector_database_client,
                ai_events_tx,
            )
            .await
            {
                log::error!("AI worker stopped: {}", error);
            }
        });
    }

    // Brokerage sync scheduler
    {
        let turso_client = turso_client.clone();
        let brokerage_client = brokerage_client.clone();
        let redis_client = redis_client.clone();
        tokio::spawn(async move {
            tradstry_backend::service::brokerage::sync::run_sync_scheduler(
                turso_client,
                brokerage_client,
                redis_client,
            )
            .await;
        });
    }

    info!("Starting server on 0.0.0.0:7899");
    info!("Allowed CORS origins: {:?}", allowed_origins);
    HttpServer::new(move || {
        let jwks_provider = create_jwks_provider(&clerk_secret);
        let cors = allowed_origins
            .iter()
            .fold(Cors::default(), |cors, origin| cors.allowed_origin(origin))
            .allowed_methods(vec!["GET", "POST", "DELETE", "OPTIONS"])
            .allowed_headers(vec![
                actix_web::http::header::AUTHORIZATION,
                actix_web::http::header::CONTENT_TYPE,
            ])
            .max_age(3600);
        App::new()
            .wrap(Logger::new(
                r#"%a "%r" %s %b "%{Referer}i" "%{User-Agent}i" %T"#,
            ))
            .wrap(ClerkMiddleware::new(
                jwks_provider,
                Some(vec![
                    "/health".to_string(),
                    "/graphql".to_string(),
                    "/notebook/images/upload".to_string(),
                    "/notebook/images/{id}".to_string(),
                    "/notebook/assist/autocomplete".to_string(),
                    "/notebook/assist/transform".to_string(),
                ]),
                true,
            ))
            .wrap(cors)
            .app_data(web::Data::new(schema.clone()))
            .app_data(web::Data::new(turso_client.clone()))
            .app_data(web::Data::new(r2_client.clone()))
            .app_data(web::Data::new(agents_client.clone()))
            .app_data(web::Data::new(vector_database_client.clone()))
            .app_data(web::Data::new(brokerage_client.clone()))
            .app_data(web::Data::new(ai_events_tx.clone()))
            .app_data(web::Data::new(chat_jobs.clone()))
            .app_data(web::Data::new(chat_session_store.clone()))
            .app_data(web::Data::new(jwks_provider_data.clone()))
            .configure(routes::configure)
    })
    .workers() // leave empty "number of workers"
    .bind("0.0.0.0:7899")?
    .run()
    .await?;
    Ok(())
}
