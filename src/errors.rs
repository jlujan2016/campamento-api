use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

// Esta es nuestra lista de errores posibles
// #[derive(Debug)] permite imprimirlos para debug
// thiserror::Error genera automáticamente el trait Error de Rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    // Errores de base de datos — se generan automáticamente desde errores de SQLx
    #[error("Error de base de datos: {0}")]
    Database(#[from] sqlx::Error),

    // Errores de autenticación (credenciales incorrectas, token inválido, etc.)
    #[error("No autorizado: {0}")]
    Unauthorized(String),

    // Errores cuando algo no se encuentra (ej. evento que no existe)
    #[error("No encontrado: {0}")]
    NotFound(String),

    // Errores de validación (datos mal formados en el request)
    #[error("Datos inválidos: {0}")]
    Validation(String),
}

// Este trait le dice a Axum cómo convertir nuestros errores en respuestas HTTP
// Axum llama a "into_response()" automáticamente cuando un handler devuelve un error
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Decidimos qué código HTTP corresponde a cada tipo de error
        let (status, message) = match &self {
            AppError::Database(e) => {
                // Logueamos el error interno pero no lo exponemos al cliente
                tracing::error!("Error de DB: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Error interno del servidor".to_string())
            }
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
        };

        // Devolvemos una respuesta JSON con el código HTTP y el mensaje de error
        // json!() es una macro que crea JSON fácilmente
        (status, Json(json!({ "error": message }))).into_response()
    }
}