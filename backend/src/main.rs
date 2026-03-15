mod graphql;
mod routes;
mod service;
use actix_cors::Cors;
use actix_web::{App, HttpServer, middleware::Logger, web};
use clerk_rs::validators::actix::ClerkMiddleware;
use log::info;
use service::auth::create_jwks_provider;
use service::cloudinary::{CloudinaryClient, CloudinaryConfig};
use service::turso::TursoClient;
use service::turso::TursoConfig;
use std::sync::Arc;
fn cors_allowed_origins() -> Vec<String> {
    let defaults = [
        "http://localhost:3000",
        "http://127.0.0.1:3000",
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
#[actix_web::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    env_logger::init();
    info!("Starting backend...");

    // --- DEBUG: print env vars to confirm they're loaded ---
    println!("TURSO_DB_URL: {:?}", std::env::var("TURSO_DB_URL"));
    println!(
        "TURSO_DB_TOKEN: {:?}",
        std::env::var("TURSO_DB_TOKEN").map(|t| format!("{}...", &t[..20]))
    );
    // --------------------------------------------------------

    let config = TursoConfig::from_env()?;
    let turso_client = Arc::new(TursoClient::new(config).await?);
    let cloudinary_client = Arc::new(CloudinaryClient::new(CloudinaryConfig::from_env()?));
    turso_client.health_check().await?;
    info!("Database healthy and migrations applied");
    let clerk_secret = std::env::var("CLERK_SECRET_KEY")?;
    info!("Clerk authentication configured");
    let schema = graphql::build_schema();
    let allowed_origins = cors_allowed_origins();
    info!("Starting server on 0.0.0.0:8080");
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
            .wrap(ClerkMiddleware::new(jwks_provider, None, true))
            .wrap(cors)
            .app_data(web::Data::new(schema.clone()))
            .app_data(web::Data::new(turso_client.clone()))
            .app_data(web::Data::new(cloudinary_client.clone()))
            .configure(routes::configure)
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await?;
    Ok(())
}
