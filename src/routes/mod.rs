use axum::{middleware, routing::{get, post}, Router};
use sqlx::PgPool;

mod health;
pub mod auth;

pub use auth::AuthState;

pub fn create_router(pool: PgPool, jwt_secret: String) -> Router {
    let auth_state = AuthState {
        pool: pool.clone(),
        jwt_secret,
    };

    // Rutas públicas — todas usan AuthState como estado
    let public_routes = Router::new()
        .route("/health", get(health::health_handler))
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .with_state(auth_state.clone());

    // Rutas protegidas con middleware de autenticación
    let protected_routes = Router::new()
        .route("/auth/me", get(auth::me))
        .layer(middleware::from_fn_with_state(
            auth_state.clone(),
            auth::require_auth,
        ))
        .with_state(auth_state);

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
}