//! TEMPER server library: router assembly + process entry points.

pub mod error;
pub mod practice;
pub mod routes;
pub mod state;
pub mod store;

use axum::extract::DefaultBodyLimit;
use axum::Router;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::state::SharedState;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn app(state: SharedState) -> Router {
    let static_dir = state.config.static_dir.clone();
    let cors = state.config.cors;

    let mut router = Router::new()
        .nest("/api", routes::api_router())
        .merge(routes::page_router())
        .fallback_service(
            ServeDir::new(static_dir).append_index_html_on_directories(true),
        )
        .layer(DefaultBodyLimit::max(64 * 1024))
        .layer(CatchPanicLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    if cors {
        router = router.layer(CorsLayer::permissive());
    }
    router
}
