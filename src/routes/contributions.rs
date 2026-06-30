use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension,
    Json,
};

use uuid::Uuid;
use crate::{
    auth::Claims,
    errors::AppError,
    models::contribution::{
        ApproveContributionRequest, Contribution, ContributionType,
        ContributionWithDetails, CreateContributionRequest,
        CreateContributionTypeRequest, CreateFinalCheckpointRequest,
        FinalAttendance, FinalAttendanceRequest, FinalCheckpoint,
    },
    routes::AuthState,
};

// POST /events/:id/contribution-types — crear tipo de aporte (admin)
pub async fn create_contribution_type(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
    Json(req): Json<CreateContributionTypeRequest>,
) -> Result<(StatusCode, Json<ContributionType>), AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    crate::routes::events::verify_event_admin(
        &state.pool, event_id, user_id, claims.is_super_admin
    ).await?;

    if req.default_hour_bonus < 0.0 {
        return Err(AppError::Validation(
            "El bono de horas no puede ser negativo".to_string()
        ));
    }

    let ct = sqlx::query_as!(
        ContributionType,
        r#"
        INSERT INTO contribution_types (event_id, type_key, label, default_hour_bonus, created_by)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
        event_id,
        req.type_key,
        req.label,
        req.default_hour_bonus,
        user_id,
    )
    .fetch_one(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(ct)))
}

// GET /events/:id/contribution-types — listar tipos de aporte
pub async fn list_contribution_types(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<Vec<ContributionType>>, AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    let is_member = sqlx::query!(
        "SELECT id FROM event_members WHERE event_id = $1 AND user_id = $2 AND status = 'active'",
        event_id, user_id
    )
    .fetch_optional(&state.pool)
    .await?;

    if is_member.is_none() && !claims.is_super_admin {
        return Err(AppError::Unauthorized(
            "Solo los miembros pueden ver los tipos de aporte".to_string()
        ));
    }

    let types = sqlx::query_as!(
        ContributionType,
        "SELECT * FROM contribution_types WHERE event_id = $1 ORDER BY label ASC",
        event_id
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(types))
}

// POST /events/:id/contributions — registrar un aporte
pub async fn create_contribution(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
    Json(req): Json<CreateContributionRequest>,
) -> Result<(StatusCode, Json<ContributionWithDetails>), AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    // Verificar membresía
    let is_member = sqlx::query!(
        "SELECT id FROM event_members WHERE event_id = $1 AND user_id = $2 AND status = 'active'",
        event_id, user_id
    )
    .fetch_optional(&state.pool)
    .await?;

    if is_member.is_none() {
        return Err(AppError::Unauthorized(
            "Debes ser miembro del evento para registrar aportes".to_string()
        ));
    }

    // Traemos el tipo de aporte para obtener el bono por defecto
    let contribution_type = sqlx::query!(
        "SELECT id, label, default_hour_bonus FROM contribution_types WHERE id = $1 AND event_id = $2",
        req.contribution_type_id, event_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Tipo de aporte no encontrado".to_string()))?;

    // Usamos el override si viene, si no el default del tipo
    let hour_bonus = req.hour_bonus_override
        .unwrap_or(contribution_type.default_hour_bonus);

    let contribution = sqlx::query_as!(
        ContributionWithDetails,
        r#"
        WITH inserted AS (
            INSERT INTO contributions (
                event_id, user_id, contribution_type_id,
                description, hour_bonus
            )
            VALUES ($1, $2, $3, $4, $5)
            RETURNING *
        )
        SELECT
            i.id, i.event_id, i.user_id,
            u.name as user_name,
            i.contribution_type_id,
            $6::text as "type_label!",
            i.description,
            i.hour_bonus,
            i.approved_by,
            i.status,
            i.created_at
        FROM inserted i
        JOIN users u ON u.id = i.user_id
        "#,
        event_id,
        user_id,
        req.contribution_type_id,
        req.description,
        hour_bonus,
        contribution_type.label,
    )
    .fetch_one(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(contribution)))
}

// PUT /contributions/:id/approve — aprobar o rechazar aporte (admin)
pub async fn approve_contribution(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(contribution_id): Path<Uuid>,
    Json(req): Json<ApproveContributionRequest>,
) -> Result<Json<Contribution>, AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    if req.action != "approve" && req.action != "reject" {
        return Err(AppError::Validation(
            "La acción debe ser 'approve' o 'reject'".to_string()
        ));
    }

    // Traemos el aporte para verificar permisos
    let contribution = sqlx::query!(
        "SELECT id, event_id, status FROM contributions WHERE id = $1",
        contribution_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Aporte no encontrado".to_string()))?;

    if contribution.status != "pending" {
        return Err(AppError::Validation("Este aporte ya fue procesado".to_string()));
    }

    crate::routes::events::verify_event_admin(
        &state.pool, contribution.event_id, user_id, claims.is_super_admin
    ).await?;

    let new_status = if req.action == "approve" { "approved" } else { "rejected" };

    // Si el admin ajusta el bono al aprobar, lo actualizamos también
    let updated = if let Some(new_bonus) = req.hour_bonus_override {
        sqlx::query_as!(
            Contribution,
            r#"
            UPDATE contributions
            SET status = $1, approved_by = $2, hour_bonus = $3
            WHERE id = $4
            RETURNING *
            "#,
            new_status, user_id, new_bonus, contribution_id,
        )
        .fetch_one(&state.pool)
        .await?
    } else {
        sqlx::query_as!(
            Contribution,
            r#"
            UPDATE contributions
            SET status = $1, approved_by = $2
            WHERE id = $3
            RETURNING *
            "#,
            new_status, user_id, contribution_id,
        )
        .fetch_one(&state.pool)
        .await?
    };

    Ok(Json(updated))
}

// POST /events/:id/final-checkpoint — crear tramo final (admin)
pub async fn create_final_checkpoint(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
    Json(req): Json<CreateFinalCheckpointRequest>,
) -> Result<(StatusCode, Json<FinalCheckpoint>), AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    crate::routes::events::verify_event_admin(
        &state.pool, event_id, user_id, claims.is_super_admin
    ).await?;

    let checkpoint = sqlx::query_as!(
        FinalCheckpoint,
        r#"
        INSERT INTO final_checkpoints (event_id, opens_at, description)
        VALUES ($1, $2, $3)
        RETURNING *
        "#,
        event_id,
        req.opens_at,
        req.description,
    )
    .fetch_one(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(checkpoint)))
}

// POST /events/:id/final-checkpoint/attend — registrar presencia en tramo final
pub async fn attend_final_checkpoint(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
    Json(req): Json<FinalAttendanceRequest>,
) -> Result<(StatusCode, Json<FinalAttendance>), AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    // Buscar el checkpoint activo del evento
    let checkpoint = sqlx::query!(
        r#"
        SELECT id FROM final_checkpoints
        WHERE event_id = $1 AND opens_at <= NOW()
        ORDER BY opens_at DESC LIMIT 1
        "#,
        event_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound(
        "No hay un tramo final activo para este evento".to_string()
    ))?;

    // Verificar que cumple el mínimo de horas reales exigido
    let event = sqlx::query!(
        "SELECT min_total_hours FROM events WHERE id = $1",
        event_id
    )
    .fetch_one(&state.pool)
    .await?;

    if let Some(min_hours) = event.min_total_hours {
        // Calculamos las horas reales acumuladas (métrica 2)
        let real_hours: f64 = sqlx::query_scalar!(
            r#"
            SELECT COALESCE(
                SUM(
                    EXTRACT(EPOCH FROM (co.timestamp - ci.timestamp)) / 3600
                )::float8, 0.0
            )
            FROM shifts s
            JOIN checkins ci ON ci.shift_id = s.id AND ci.type = 'check_in'
            JOIN checkins co ON co.shift_id = s.id AND co.type = 'check_out'
            WHERE s.event_id = $1 AND s.user_id = $2 AND s.status = 'done'
            "#,
            event_id,
            user_id
        )
        .fetch_one(&state.pool)
        .await?
        .unwrap_or(0.0);
        // Después — casteamos a FLOAT8 para que SQLx lo mapee a f64
        let real_hours: f64 = sqlx::query_scalar!(
            r#"
            SELECT COALESCE(
                SUM(
                    EXTRACT(EPOCH FROM (co.timestamp - ci.timestamp)) / 3600
                )::float8, 0.0
            )
            FROM shifts s
            JOIN checkins ci ON ci.shift_id = s.id AND ci.type = 'check_in'
            JOIN checkins co ON co.shift_id = s.id AND co.type = 'check_out'
            WHERE s.event_id = $1 AND s.user_id = $2 AND s.status = 'done'
            "#,
            event_id, user_id
        )
        .fetch_one(&state.pool)
        .await?
        .unwrap_or(0.0);

        if real_hours < min_hours {
            return Err(AppError::Validation(format!(
                "No cumplís el mínimo de {:.1} horas reales exigido. Llevás {:.1} horas.",
                min_hours, real_hours
            )));
        }
    }

    // Verificar que no esté ya registrado
    let already = sqlx::query!(
        "SELECT id FROM final_attendance WHERE final_checkpoint_id = $1 AND user_id = $2",
        checkpoint.id, user_id
    )
    .fetch_optional(&state.pool)
    .await?;

    if already.is_some() {
        return Err(AppError::Validation(
            "Ya registraste tu presencia en el tramo final".to_string()
        ));
    }

    let attendance = sqlx::query_as!(
        FinalAttendance,
        r#"
        WITH inserted AS (
            INSERT INTO final_attendance (final_checkpoint_id, user_id, lat, lng)
            VALUES ($1, $2, $3, $4)
            RETURNING *
        )
        SELECT
            i.id, i.final_checkpoint_id, i.user_id,
            u.name as user_name,
            i.checkin_time, i.lat, i.lng, i.status
        FROM inserted i
        JOIN users u ON u.id = i.user_id
        "#,
        checkpoint.id,
        user_id,
        req.lat,
        req.lng,
    )
    .fetch_one(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(attendance)))
}

// GET /events/:id/contributions — listar aportes del evento (admin)
pub async fn list_contributions(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<Vec<ContributionWithDetails>>, AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    crate::routes::events::verify_event_admin(
        &state.pool, event_id, user_id, claims.is_super_admin
    ).await?;

    let contributions = sqlx::query_as!(
        ContributionWithDetails,
        r#"
        SELECT
            c.id, c.event_id, c.user_id,
            u.name as user_name,
            c.contribution_type_id,
            ct.label as type_label,
            c.description,
            c.hour_bonus,
            c.approved_by,
            c.status,
            c.created_at
        FROM contributions c
        JOIN users u ON u.id = c.user_id
        JOIN contribution_types ct ON ct.id = c.contribution_type_id
        WHERE c.event_id = $1
        ORDER BY c.created_at DESC
        "#,
        event_id
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(contributions))
}