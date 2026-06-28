use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension,
    Json,
};
use chrono::Utc;
use uuid::Uuid;
use crate::{
    auth::Claims,
    errors::AppError,
    models::replacement::{
        CreateReplacementRequest, GapShift,
        RespondReplacementRequest, ShiftReplacementWithUsers,
    },
    routes::AuthState,
};

// POST /shifts/:id/replacement — solicitar reemplazo
pub async fn create_replacement(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(shift_id): Path<Uuid>,
    Json(req): Json<CreateReplacementRequest>,
) -> Result<(StatusCode, Json<ShiftReplacementWithUsers>), AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    // Traemos el turno para validar
    let shift = sqlx::query!(
        "SELECT id, event_id, user_id, status FROM shifts WHERE id = $1",
        shift_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Turno no encontrado".to_string()))?;

    // Solo el dueño del turno o el reemplazante pueden solicitar
    let is_original = shift.user_id == user_id;
    let is_replacement = req.replacement_user_id == user_id;

    if !is_original && !is_replacement && !claims.is_super_admin {
        return Err(AppError::Unauthorized(
            "Solo el dueño del turno o el reemplazante pueden solicitar un reemplazo".to_string()
        ));
    }

    if shift.status != "approved" && shift.status != "pending" {
        return Err(AppError::Validation(format!(
            "No se puede reemplazar un turno en estado '{}'", shift.status
        )));
    }

    // Verificar que el reemplazante es miembro del evento
    let is_member = sqlx::query!(
        r#"
        SELECT id FROM event_members
        WHERE event_id = $1 AND user_id = $2 AND status = 'active'
        "#,
        shift.event_id,
        req.replacement_user_id
    )
    .fetch_optional(&state.pool)
    .await?;

    if is_member.is_none() {
        return Err(AppError::Validation(
            "El reemplazante debe ser miembro activo del evento".to_string()
        ));
    }

    // Validar reemplazo parcial: si viene covers_start debe venir covers_end y viceversa
    if req.covers_start.is_some() != req.covers_end.is_some() {
        return Err(AppError::Validation(
            "covers_start y covers_end deben venir juntos para reemplazo parcial".to_string()
        ));
    }

    let requested_by = if is_original { "original" } else { "replacement" };

    // Verificar que no haya ya un reemplazo pendiente para este turno
    let existing = sqlx::query!(
        "SELECT id FROM shift_replacements WHERE shift_id = $1 AND status = 'pending'",
        shift_id
    )
    .fetch_optional(&state.pool)
    .await?;

    if existing.is_some() {
        return Err(AppError::Validation(
            "Ya existe una solicitud de reemplazo pendiente para este turno".to_string()
        ));
    }

    let replacement = sqlx::query_as!(
        ShiftReplacementWithUsers,
        r#"
        WITH inserted AS (
            INSERT INTO shift_replacements (
                shift_id, original_user_id, replacement_user_id,
                requested_by, covers_start, covers_end
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
        )
        SELECT
            i.id, i.shift_id,
            i.original_user_id,
            ou.name as original_user_name,
            i.replacement_user_id,
            ru.name as replacement_user_name,
            i.requested_by, i.status,
            i.covers_start, i.covers_end,
            i.created_at, i.confirmed_at
        FROM inserted i
        JOIN users ou ON ou.id = i.original_user_id
        JOIN users ru ON ru.id = i.replacement_user_id
        "#,
        shift_id,
        shift.user_id,
        req.replacement_user_id,
        requested_by,
        req.covers_start,
        req.covers_end,
    )
    .fetch_one(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(replacement)))
}

// PUT /shifts/:id/replacement/:rid — confirmar o rechazar reemplazo
pub async fn respond_replacement(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path((shift_id, replacement_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<RespondReplacementRequest>,
) -> Result<Json<ShiftReplacementWithUsers>, AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    if req.action != "confirm" && req.action != "reject" {
        return Err(AppError::Validation(
            "La acción debe ser 'confirm' o 'reject'".to_string()
        ));
    }

    // Traemos el reemplazo
    let replacement = sqlx::query!(
        r#"
        SELECT id, shift_id, original_user_id, replacement_user_id,
               requested_by, status, covers_start, covers_end
        FROM shift_replacements
        WHERE id = $1 AND shift_id = $2
        "#,
        replacement_id, shift_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Solicitud de reemplazo no encontrada".to_string()))?;

    if replacement.status != "pending" {
        return Err(AppError::Validation(
            "Esta solicitud ya fue procesada".to_string()
        ));
    }

    // Solo puede responder la parte que NO solicitó
    // Si lo pidió el original, debe confirmar el reemplazante (y viceversa)
    let can_respond = match replacement.requested_by.as_str() {
        "original" => replacement.replacement_user_id == user_id,
        "replacement" => replacement.original_user_id == user_id,
        _ => false,
    } || claims.is_super_admin;

    if !can_respond {
        return Err(AppError::Unauthorized(
            "Solo la otra parte puede confirmar o rechazar el reemplazo".to_string()
        ));
    }

    let new_status = if req.action == "confirm" { "confirmed" } else { "rejected" };

    // Usamos transacción porque si se confirma hay que actualizar también el shift
    let mut tx = state.pool.begin().await?;

    // Actualizamos el estado del reemplazo
    sqlx::query!(
        r#"
        UPDATE shift_replacements
        SET status = $1, confirmed_at = $2
        WHERE id = $3
        "#,
        new_status,
        Utc::now(),
        replacement_id,
    )
    .execute(&mut *tx)
    .await?;

    // Si se confirmó, actualizamos el shift para reflejar el reemplazo
    if req.action == "confirm" {
        // Para reemplazo total: cambiamos el user_id del shift
        // Para reemplazo parcial: solo registramos el reemplazo (el shift original se mantiene)
        if replacement.covers_start.is_none() {
            // Reemplazo total — el shift pasa al reemplazante
            sqlx::query!(
                "UPDATE shifts SET user_id = $1 WHERE id = $2",
                replacement.replacement_user_id,
                shift_id
            )
            .execute(&mut *tx)
            .await?;
        }
        // Para reemplazo parcial, el admin lo gestiona manualmente
        // (se crea un shift extra para el reemplazante si hace falta)
    }

    tx.commit().await?;

    // Devolvemos el reemplazo actualizado con nombres
    let updated = sqlx::query_as!(
        ShiftReplacementWithUsers,
        r#"
        SELECT
            r.id, r.shift_id,
            r.original_user_id, ou.name as original_user_name,
            r.replacement_user_id, ru.name as replacement_user_name,
            r.requested_by, r.status,
            r.covers_start, r.covers_end,
            r.created_at, r.confirmed_at
        FROM shift_replacements r
        JOIN users ou ON ou.id = r.original_user_id
        JOIN users ru ON ru.id = r.replacement_user_id
        WHERE r.id = $1
        "#,
        replacement_id
    )
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(updated))
}

// POST /events/:id/members/:uid/withdraw — retirar participante
pub async fn withdraw_member(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path((event_id, target_user_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    // Solo admin del evento o super admin puede retirar participantes
    crate::routes::events::verify_event_admin(
        &state.pool, event_id, user_id, claims.is_super_admin
    ).await?;

    // Verificar que el miembro existe y está activo
    let member = sqlx::query!(
        r#"
        SELECT id FROM event_members
        WHERE event_id = $1 AND user_id = $2 AND status = 'active'
        "#,
        event_id, target_user_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Miembro no encontrado o ya retirado".to_string()))?;

    let mut tx = state.pool.begin().await?;

    // Marcamos al miembro como retirado
    sqlx::query!(
        r#"
        UPDATE event_members
        SET status = 'withdrawn', withdrawn_at = $1
        WHERE id = $2
        "#,
        Utc::now(),
        member.id,
    )
    .execute(&mut *tx)
    .await?;

    // Cancelamos todos sus turnos futuros
    let cancelled = sqlx::query!(
        r#"
        UPDATE shifts
        SET status = 'cancelled'
        WHERE event_id = $1
        AND user_id = $2
        AND scheduled_start > NOW()
        AND status IN ('pending', 'approved')
        RETURNING id, slot_id
        "#,
        event_id,
        target_user_id,
    )
    .fetch_all(&mut *tx)
    .await?;

    // Para cada turno cancelado que venía de un slot,
    // cancelamos también el slot_signup (para que el cupo quede libre)
    for shift in &cancelled {
        if let Some(slot_id) = shift.slot_id {
            sqlx::query!(
                r#"
                UPDATE slot_signups
                SET status = 'cancelled'
                WHERE slot_id = $1 AND user_id = $2
                "#,
                slot_id,
                target_user_id,
            )
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;

    // Creamos notificación de huecos liberados (la procesará el worker de Telegram)
    if !cancelled.is_empty() {
        sqlx::query!(
            r#"
            INSERT INTO notifications (event_id, type, payload)
            VALUES ($1, 'slot_freed', $2)
            "#,
            event_id,
            serde_json::json!({
                "freed_count": cancelled.len(),
                "user_id": target_user_id,
                "message": format!("Se liberaron {} turnos por retiro de un participante", cancelled.len())
            })
        )
        .execute(&state.pool)
        .await?;
    }

    Ok(Json(serde_json::json!({
        "message": "Participante retirado exitosamente",
        "cancelled_shifts": cancelled.len(),
        "freed_slots": cancelled.iter().filter(|s| s.slot_id.is_some()).count()
    })))
}

// GET /events/:id/shifts/gaps — ver turnos con vacío sin resolver
pub async fn list_gaps(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<Vec<GapShift>>, AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    crate::routes::events::verify_event_admin(
        &state.pool, event_id, user_id, claims.is_super_admin
    ).await?;

    let gaps = sqlx::query_as!(
        GapShift,
        r#"
        SELECT
            s.id as shift_id,
            s.user_id,
            u.name as user_name,
            s.scheduled_start,
            s.scheduled_end,
            s.created_at as gap_since
        FROM shifts s
        JOIN users u ON u.id = s.user_id
        WHERE s.event_id = $1 AND s.status = 'gap_unresolved'
        ORDER BY s.scheduled_start ASC
        "#,
        event_id
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(gaps))
}

// POST /shifts/:id/mark-gap — marcar turno como vacío sin resolver
pub async fn mark_gap(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(shift_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    // Traemos el turno para verificar permisos
    let shift = sqlx::query!(
        "SELECT id, event_id, status FROM shifts WHERE id = $1",
        shift_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Turno no encontrado".to_string()))?;

    // Solo el admin del evento o el propio usuario del turno puede marcar gap
    let is_own_shift = sqlx::query!(
        "SELECT id FROM shifts WHERE id = $1 AND user_id = $2",
        shift_id, user_id
    )
    .fetch_optional(&state.pool)
    .await?
    .is_some();

    if !is_own_shift {
        crate::routes::events::verify_event_admin(
            &state.pool, shift.event_id, user_id, claims.is_super_admin
        ).await?;
    }

    if shift.status != "approved" && shift.status != "pending" {
        return Err(AppError::Validation(format!(
            "No se puede marcar como vacío un turno en estado '{}'", shift.status
        )));
    }

    // Marcamos el turno como gap_unresolved
    sqlx::query!(
        "UPDATE shifts SET status = 'gap_unresolved' WHERE id = $1",
        shift_id
    )
    .execute(&state.pool)
    .await?;

    // Notificamos al grupo del evento
    sqlx::query!(
        r#"
        INSERT INTO notifications (event_id, type, payload)
        VALUES ($1, 'gap_unresolved', $2)
        "#,
        shift.event_id,
        serde_json::json!({
            "shift_id": shift_id,
            "message": "Hay un turno sin cubrir que necesita atención"
        })
    )
    .execute(&state.pool)
    .await?;

    Ok(Json(serde_json::json!({
        "message": "Turno marcado como vacío sin resolver. Se notificó al grupo.",
        "shift_id": shift_id
    })))
}