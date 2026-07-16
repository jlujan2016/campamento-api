use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Extension,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;
use crate::{
    auth::{Claims, hash_password},
    errors::AppError,
    models::user::{CreateUserRequest, UserPublic},
    routes::AuthState,
};

// Query params para buscar usuarios
#[derive(Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,      // buscar por nombre o email
    pub event_id: Option<Uuid>, // filtrar por evento (no usado aún, para futuro)
}

// GET /users — listar todos los usuarios (solo super admin)
pub async fn list_users(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Query(params): Query<SearchParams>,
) -> Result<Json<Vec<UserPublic>>, AppError> {

    if !claims.is_super_admin {
        return Err(AppError::Unauthorized(
            "Solo el super admin puede ver todos los usuarios".to_string()
        ));
    }

    let search = params.q.unwrap_or_default();
    let search_pattern = format!("%{}%", search.to_lowercase());

    let users = sqlx::query_as!(
        UserPublic,
        r#"
        SELECT id, email, name, phone, is_super_admin,
               is_guest, is_blocked, created_at
        FROM users
        WHERE (
            LOWER(name) LIKE $1
            OR LOWER(COALESCE(email, '')) LIKE $1
        )
        ORDER BY created_at DESC
        LIMIT 50
        "#,
        search_pattern,
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(users))
}

// POST /users — crear usuario
// Super admin puede crear admins y participantes
// Admin de evento puede crear solo participantes
pub async fn create_user(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserPublic>), AppError> {

    let caller_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    // Solo super admin puede crear otros super admins
    let is_super_admin = req.is_super_admin.unwrap_or(false);
    if is_super_admin && !claims.is_super_admin {
        return Err(AppError::Unauthorized(
            "Solo el super admin puede crear otros admins".to_string()
        ));
    }

    // Verificar que quien llama tiene permisos para crear usuarios
    if !claims.is_super_admin {
        let is_any_admin = sqlx::query!(
            r#"
            SELECT id FROM event_members
            WHERE user_id = $1 AND role = 'admin' AND status = 'active'
            "#,
            caller_id
        )
        .fetch_optional(&state.pool)
        .await?;

        if is_any_admin.is_none() {
            return Err(AppError::Unauthorized(
                "Solo los admins pueden crear usuarios".to_string()
            ));
        }
    }

    // Validaciones básicas
    if req.email.is_empty() || req.password.is_empty() || req.name.is_empty() {
        return Err(AppError::Validation(
            "Email, contraseña y nombre son requeridos".to_string()
        ));
    }

    if req.password.len() < 8 {
        return Err(AppError::Validation(
            "La contraseña debe tener al menos 8 caracteres".to_string()
        ));
    }

    // Verificar que el email no esté ya registrado
    let existing = sqlx::query!(
        "SELECT id FROM users WHERE email = $1",
        req.email
    )
    .fetch_optional(&state.pool)
    .await?;

    if existing.is_some() {
        return Err(AppError::Validation(
            "El email ya está registrado".to_string()
        ));
    }

    let password_hash = hash_password(&req.password)?;

    let user = sqlx::query_as!(
        UserPublic,
        r#"
        INSERT INTO users (email, password_hash, name, phone, is_super_admin, is_guest)
        VALUES ($1, $2, $3, $4, $5, false)
        RETURNING id, email, name, phone, is_super_admin,
                  is_guest, is_blocked, created_at
        "#,
        req.email,
        password_hash,
        req.name,
        req.phone,
        is_super_admin,
    )
    .fetch_one(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(user)))
}

// PUT /users/:id/block — bloquear o desbloquear usuario (toggle)
pub async fn block_user(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {

    if !claims.is_super_admin {
        return Err(AppError::Unauthorized(
            "Solo el super admin puede bloquear usuarios".to_string()
        ));
    }

    // No puede bloquearse a sí mismo
    let self_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    if self_id == user_id {
        return Err(AppError::Validation(
            "No podés bloquearte a vos mismo".to_string()
        ));
    }

    // Verificar que el usuario existe y obtener su estado actual
    let current = sqlx::query!(
        "SELECT is_blocked, name FROM users WHERE id = $1",
        user_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Usuario no encontrado".to_string()))?;

    // Toggle: si está bloqueado lo desbloqueamos, si no lo bloqueamos
    let new_blocked = !current.is_blocked;

    sqlx::query!(
        "UPDATE users SET is_blocked = $1 WHERE id = $2",
        new_blocked,
        user_id
    )
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({
        "user_id": user_id,
        "name": current.name,
        "is_blocked": new_blocked,
        "message": if new_blocked {
            "Usuario bloqueado — no podrá iniciar sesión"
        } else {
            "Usuario desbloqueado — puede volver a iniciar sesión"
        }
    })))
}

// DELETE /users/:id — eliminar usuario permanentemente (solo super admin)
pub async fn delete_user(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {

    if !claims.is_super_admin {
        return Err(AppError::Unauthorized(
            "Solo el super admin puede eliminar usuarios".to_string()
        ));
    }

    let self_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    if self_id == user_id {
        return Err(AppError::Validation(
            "No podés eliminarte a vos mismo".to_string()
        ));
    }

    // Verificar que el usuario existe
    let user = sqlx::query!(
        "SELECT name FROM users WHERE id = $1",
        user_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Usuario no encontrado".to_string()))?;

    // Eliminamos en cascada respetando las foreign keys
    // El orden importa: primero las tablas hijas, luego el usuario
    let mut tx = state.pool.begin().await?;

    sqlx::query!(
        "DELETE FROM notifications WHERE user_id = $1", user_id)
        .execute(&mut *tx).await?;

    sqlx::query!(
        "DELETE FROM telegram_links WHERE user_id = $1", user_id)
        .execute(&mut *tx).await?;

    sqlx::query!(
        "DELETE FROM final_attendance WHERE user_id = $1", user_id)
        .execute(&mut *tx).await?;

    sqlx::query!(
        "DELETE FROM checkins WHERE user_id = $1", user_id)
        .execute(&mut *tx).await?;

    sqlx::query!(
        r#"DELETE FROM shift_replacements
           WHERE original_user_id = $1 OR replacement_user_id = $1"#,
        user_id)
        .execute(&mut *tx).await?;

    sqlx::query!(
        "DELETE FROM shifts WHERE user_id = $1", user_id)
        .execute(&mut *tx).await?;

    sqlx::query!(
        "DELETE FROM slot_signups WHERE user_id = $1", user_id)
        .execute(&mut *tx).await?;

    sqlx::query!(
        "DELETE FROM contributions WHERE user_id = $1", user_id)
        .execute(&mut *tx).await?;

    sqlx::query!(
        "DELETE FROM schedule_slots WHERE created_by = $1", user_id)
        .execute(&mut *tx).await?;

    sqlx::query!(
        "DELETE FROM schedule_links WHERE created_by = $1", user_id)
        .execute(&mut *tx).await?;

    sqlx::query!(
        "DELETE FROM event_members WHERE user_id = $1", user_id)
        .execute(&mut *tx).await?;

    sqlx::query!(
        "DELETE FROM users WHERE id = $1", user_id)
        .execute(&mut *tx).await?;

    tx.commit().await?;

    Ok(Json(serde_json::json!({
        "message": "Usuario eliminado permanentemente",
        "user_id": user_id,
        "name": user.name
    })))
}

// POST /events/:id/assign-admin — asignar admin a un evento (solo super admin)
pub async fn assign_event_admin(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {

    if !claims.is_super_admin {
        return Err(AppError::Unauthorized(
            "Solo el super admin puede asignar admins de evento".to_string()
        ));
    }

    let target_user_id = body["user_id"]
        .as_str()
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| AppError::Validation(
            "user_id inválido o faltante".to_string()
        ))?;

    // Verificar que el usuario existe
    let user = sqlx::query!(
        "SELECT name FROM users WHERE id = $1",
        target_user_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Usuario no encontrado".to_string()))?;

    // Verificar que el evento existe
    sqlx::query!(
        "SELECT id FROM events WHERE id = $1",
        event_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Evento no encontrado".to_string()))?;

    // Si ya es miembro, actualizamos su rol a admin
    // Si no es miembro, lo agregamos como admin
    let existing = sqlx::query!(
        r#"
        SELECT id FROM event_members
        WHERE event_id = $1 AND user_id = $2
        "#,
        event_id, target_user_id
    )
    .fetch_optional(&state.pool)
    .await?;

    if let Some(member) = existing {
        sqlx::query!(
            "UPDATE event_members SET role = 'admin', status = 'active' WHERE id = $1",
            member.id
        )
        .execute(&state.pool)
        .await?;
    } else {
        sqlx::query!(
            r#"
            INSERT INTO event_members (event_id, user_id, role)
            VALUES ($1, $2, 'admin')
            "#,
            event_id,
            target_user_id
        )
        .execute(&state.pool)
        .await?;
    }

    Ok(Json(serde_json::json!({
        "message": "Admin asignado exitosamente",
        "event_id": event_id,
        "user_id": target_user_id,
        "user_name": user.name
    })))
}