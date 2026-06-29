use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;
use crate::{
    auth::Claims,
    errors::AppError,
    routes::AuthState,
};

// Para vincular un grupo de Telegram a un evento
#[derive(Deserialize)]
pub struct LinkGroupRequest {
    pub telegram_chat_id: String,   // el ID del grupo (número negativo en Telegram)
}

// Para vincular tu cuenta personal de Telegram
#[derive(Deserialize)]
pub struct LinkAccountRequest {
    pub telegram_chat_id: String,   // tu chat_id personal
}

// POST /events/:id/telegram/group — vincular grupo al evento
pub async fn link_group(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
    Json(req): Json<LinkGroupRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    crate::routes::events::verify_event_admin(
        &state.pool, event_id, user_id, claims.is_super_admin
    ).await?;

    // Verificamos que no esté ya vinculado
    let existing = sqlx::query!(
        "SELECT id FROM telegram_links WHERE event_id = $1 AND link_type = 'group'",
        event_id
    )
    .fetch_optional(&state.pool)
    .await?;

    if let Some(_) = existing {
        // Actualizamos el chat_id existente
        sqlx::query!(
            "UPDATE telegram_links SET telegram_chat_id = $1 WHERE event_id = $2 AND link_type = 'group'",
            req.telegram_chat_id, event_id
        )
        .execute(&state.pool)
        .await?;
    } else {
        sqlx::query!(
            r#"
            INSERT INTO telegram_links (event_id, telegram_chat_id, link_type)
            VALUES ($1, $2, 'group')
            "#,
            event_id,
            req.telegram_chat_id,
        )
        .execute(&state.pool)
        .await?;
    }

    Ok((StatusCode::OK, Json(serde_json::json!({
        "message": "Grupo de Telegram vinculado exitosamente",
        "event_id": event_id,
        "telegram_chat_id": req.telegram_chat_id
    }))))
}

// POST /telegram/link-account — vincular cuenta personal
pub async fn link_account(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<LinkAccountRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    let existing = sqlx::query!(
        "SELECT id FROM telegram_links WHERE user_id = $1 AND link_type = 'private'",
        user_id
    )
    .fetch_optional(&state.pool)
    .await?;

    if existing.is_some() {
        sqlx::query!(
            "UPDATE telegram_links SET telegram_chat_id = $1 WHERE user_id = $2 AND link_type = 'private'",
            req.telegram_chat_id, user_id
        )
        .execute(&state.pool)
        .await?;
    } else {
        sqlx::query!(
            r#"
            INSERT INTO telegram_links (user_id, telegram_chat_id, link_type)
            VALUES ($1, $2, 'private')
            "#,
            user_id,
            req.telegram_chat_id,
        )
        .execute(&state.pool)
        .await?;
    }

    Ok((StatusCode::OK, Json(serde_json::json!({
        "message": "Cuenta de Telegram vinculada exitosamente",
        "user_id": user_id
    }))))
}