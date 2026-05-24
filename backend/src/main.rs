use axum::Router;
use tower_http::cors::{Any, CorsLayer};

use fundos_backend::routes;

#[tokio::main]
async fn main() {
    env_logger::init();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .nest("/api", routes::api_router())
        .layer(cors);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind to port 3000");

    log::info!("Fundos API listening on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}
