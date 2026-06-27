use axum::{extract::State, http::StatusCode, Json};
use serde_json::{json, Value};
use crate::routes::AuthState;

pub async fn health_handler(
    State(state): State<AuthState>,   // ahora recibe AuthState, no PgPool directamente
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {

    match sqlx::query!("SELECT 1 as check_val")
        .fetch_one(&state.pool)       // accedemos al pool desde state.pool
        .await
    {
        Ok(_) => Ok((
            StatusCode::OK,
            Json(json!({
                "status": "ok",
                "database": "conectada"
            })),
        )),
        Err(e) => {
            tracing::error!("Health check falló: {:?}", e);
            Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "status": "error",
                    "database": "desconectada"
                })),
            ))
        }
    }
}