use axum::{extract::State, http::StatusCode, Json};
use serde_json::{json, Value};
use sqlx::PgPool;

// Handler de GET /health
// "State(pool)" extrae el pool de la base de datos del estado global del servidor
// Axum inyecta esto automáticamente en cada petición
// Devuelve un Result: Ok con JSON si todo va bien, o un error HTTP si algo falla
pub async fn health_handler(
    State(pool): State<PgPool>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {

    // Ejecutamos una query ultra simple para verificar que la DB responde
    // sqlx::query! verifica la query contra la DB real en tiempo de compilación
    // (por eso necesitamos la DB corriendo incluso para compilar)
    match sqlx::query!("SELECT 1 as check_val")
        .fetch_one(&pool)
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