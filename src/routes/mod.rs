use axum::Router;
use sqlx::PgPool;

// Importamos el módulo health (el archivo health.rs)
mod health;

// Esta función construye y devuelve el Router completo con todas las rutas
// A medida que agreguemos más rutas (auth, eventos, turnos), las sumamos acá
pub fn create_router(pool: PgPool) -> Router {
    Router::new()
        .route("/health", axum::routing::get(health::health_handler))
        // .with_state() hace que el pool esté disponible en todos los handlers
        .with_state(pool)
}