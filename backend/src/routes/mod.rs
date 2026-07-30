pub mod clerk_webhook;
mod graphql;
mod notebook_assistance;
mod notebook_images;
pub mod notebook_media;

use actix_web::{HttpResponse, web};

pub use graphql::{graphiql, graphql_handler, graphql_ws_handler};
pub use notebook_images::{delete_notebook_image, get_notebook_image, upload_notebook_image};

async fn health() -> HttpResponse {
    HttpResponse::Ok().finish()
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.route("/health", web::get().to(health))
        .service(
            web::resource("/graphql")
                .route(web::post().to(graphql_handler))
                .route(web::get().to(graphiql)),
        )
        .service(web::resource("/graphql/ws").route(web::get().to(graphql_ws_handler)))
        .service(
            web::resource("/webhooks/clerk").route(web::post().to(clerk_webhook::clerk_webhook)),
        )
        .service(
            web::scope("/notebook/images")
                .route("/upload", web::post().to(upload_notebook_image))
                .route("/{id}", web::delete().to(delete_notebook_image))
                .route("/{id}", web::get().to(get_notebook_image)),
        )
        .service(
            web::scope("/notebook/media")
                .route(
                    "/upload",
                    web::post().to(notebook_media::upload_notebook_media),
                )
                .route(
                    "/{hash}/thumb",
                    web::get().to(notebook_media::get_notebook_media_thumb),
                )
                .route("/{hash}", web::get().to(notebook_media::get_notebook_media))
                .route(
                    "/{hash}",
                    web::delete().to(notebook_media::delete_notebook_media),
                ),
        )
        .service(
            web::scope("/notebook/assist")
                .route(
                    "/autocomplete",
                    web::post().to(notebook_assistance::autocomplete_handler),
                )
                .route(
                    "/transform",
                    web::post().to(notebook_assistance::transform_handler),
                ),
        );
}
