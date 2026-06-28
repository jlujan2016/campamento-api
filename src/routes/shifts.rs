use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension,
    Json,
};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::{
    auth::Claims,
    errors::AppError,
    models::shift::{
        ActivePresence, Checkin, CheckinRequest, CheckinResponse,
        CreateExtraShiftRequest, Shift, ShiftWithUser,
    },
    routes::AuthState,
};

// GET /events/:id/shifts — ver mis turnos en un evento
pub async fn list_my_shifts(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<Vec<Shift>>, AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    let shifts = sqlx::query_as!(
        Shift,
        r#"
        SELECT * FROM shifts
        WHERE event_id = $1 AND user_id = $2
        AND status NOT IN ('cancelled', 'rejected')
        ORDER BY scheduled_start ASC
        "#,
        event_id,
        user_id,
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(shifts))
}

// POST /events/:id/shifts — crear turno extra espontáneo
pub async fn create_extra_shift(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
    Json(req): Json<CreateExtraShiftRequest>,
) -> Result<(StatusCode, Json<Shift>), AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    // Verificar que es miembro activo del evento
    let member = sqlx::query!(
        "SELECT id FROM event_members WHERE event_id = $1 AND user_id = $2 AND status = 'active'",
        event_id, user_id
    )
    .fetch_optional(&state.pool)
    .await?;

    if member.is_none() {
        return Err(AppError::Unauthorized(
            "Debes ser miembro del evento para crear turnos extra".to_string()
        ));
    }

    // Validar duración mínima configurada en el evento
    let event = sqlx::query!(
        "SELECT min_shift_hours, max_shift_hours FROM events WHERE id = $1",
        event_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Evento no encontrado".to_string()))?;

    if req.scheduled_end <= req.scheduled_start {
        return Err(AppError::Validation(
            "La hora de fin debe ser posterior a la de inicio".to_string()
        ));
    }

    let duration_hours = (req.scheduled_end - req.scheduled_start)
        .num_minutes() as f64 / 60.0;

    if duration_hours < event.min_shift_hours {
        return Err(AppError::Validation(format!(
            "El turno debe durar al menos {} horas", event.min_shift_hours
        )));
    }

    if let Some(max) = event.max_shift_hours {
        if duration_hours > max {
            return Err(AppError::Validation(format!(
                "El turno no puede durar más de {} horas", max
            )));
        }
    }

    // Los turnos extra quedan en 'pending' hasta que el admin los apruebe
    let shift = sqlx::query_as!(
        Shift,
        r#"
        INSERT INTO shifts (
            event_id, user_id, shift_type,
            scheduled_start, scheduled_end,
            status, adjustment_reason
        )
        VALUES ($1, $2, 'extra', $3, $4, 'pending', $5)
        RETURNING *
        "#,
        event_id,
        user_id,
        req.scheduled_start,
        req.scheduled_end,
        req.notes,
    )
    .fetch_one(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(shift)))
}

// POST /shifts/:id/checkin — registrar entrada
pub async fn do_checkin(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(shift_id): Path<Uuid>,
    Json(req): Json<CheckinRequest>,
) -> Result<Json<CheckinResponse>, AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    // Verificar que el turno existe, es del usuario, y está aprobado
    let shift = sqlx::query!(
        r#"
        SELECT s.id, s.event_id, s.scheduled_start, s.scheduled_end, s.status,
               e.late_tolerance_minutes
        FROM shifts s
        JOIN events e ON e.id = s.event_id
        WHERE s.id = $1 AND s.user_id = $2
        "#,
        shift_id, user_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Turno no encontrado".to_string()))?;

    if shift.status != "approved" {
        return Err(AppError::Validation(format!(
            "El turno no está aprobado (estado actual: {})", shift.status
        )));
    }

    // Verificar que no haya un check-in previo sin check-out
    let existing_checkin = sqlx::query!(
        r#"
        SELECT id FROM checkins
        WHERE shift_id = $1 AND type = 'check_in'
        "#,
        shift_id
    )
    .fetch_optional(&state.pool)
    .await?;

    if existing_checkin.is_some() {
        return Err(AppError::Validation(
            "Ya hiciste check-in en este turno. Primero debes hacer check-out.".to_string()
        ));
    }

    let now = Utc::now();

    // Calcular effective_end: si llegó tarde, corremos el horario
    // Si llegó dentro del margen de tolerancia, no se corre nada
    let minutes_late = (now - shift.scheduled_start).num_minutes();
    let tolerance = shift.late_tolerance_minutes as i64;
    let duration = shift.scheduled_end - shift.scheduled_start;

    let effective_end = if minutes_late > tolerance {
        // Llegó tarde: el horario se corre para completar la duración comprometida
        now + duration
    } else {
        // Llegó a tiempo (dentro del margen): fin normal
        shift.scheduled_end
    };

    // Registramos el check-in
    let checkin = sqlx::query_as!(
        Checkin,
        r#"
        INSERT INTO checkins (shift_id, user_id, type, lat, lng, accuracy_m, photo_url)
        VALUES ($1, $2, 'check_in', $3, $4, $5, $6)
        RETURNING
            id, shift_id, user_id,
            type as checkin_type,
            lat, lng, accuracy_m, photo_url,
            timestamp
        "#,
        shift_id,
        user_id,
        req.lat,
        req.lng,
        req.accuracy_m,
        req.photo_url,
    )
    .fetch_one(&state.pool)
    .await?;

    // Actualizamos el turno a 'active'
    sqlx::query!(
        "UPDATE shifts SET status = 'active' WHERE id = $1",
        shift_id
    )
    .execute(&state.pool)
    .await?;

    // Mensaje personalizado según si llegó tarde o no
    let message = if minutes_late > tolerance {
        format!(
            "Check-in registrado. Llegaste {} minutos tarde — tu turno se extiende hasta las {}",
            minutes_late,
            effective_end.format("%H:%M UTC")
        )
    } else {
        format!(
            "Check-in registrado. Tu turno termina a las {}",
            effective_end.format("%H:%M UTC")
        )
    };

    Ok(Json(CheckinResponse {
        checkin,
        message,
        effective_end: Some(effective_end),
        hours_so_far: None,
    }))
}

// POST /shifts/:id/checkout — registrar salida
pub async fn do_checkout(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(shift_id): Path<Uuid>,
    Json(req): Json<CheckinRequest>,
) -> Result<Json<CheckinResponse>, AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    // Verificar que el turno está activo
    let shift = sqlx::query!(
        "SELECT id, scheduled_start, scheduled_end, status FROM shifts WHERE id = $1 AND user_id = $2",
        shift_id, user_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Turno no encontrado".to_string()))?;

    if shift.status != "active" {
        return Err(AppError::Validation(
            "Debes hacer check-in primero antes de hacer check-out".to_string()
        ));
    }

    // Buscar el check-in para calcular horas reales
    let checkin_record = sqlx::query!(
        "SELECT timestamp FROM checkins WHERE shift_id = $1 AND type = 'check_in'",
        shift_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::Validation("No se encontró el check-in de este turno".to_string()))?;

    let now = Utc::now();

    // Horas reales = desde check-in hasta ahora
    let hours_real = (now - checkin_record.timestamp).num_minutes() as f64 / 60.0;

    // Registramos el check-out
    let checkin = sqlx::query_as!(
        Checkin,
        r#"
        INSERT INTO checkins (shift_id, user_id, type, lat, lng, accuracy_m, photo_url)
        VALUES ($1, $2, 'check_out', $3, $4, $5, $6)
        RETURNING
            id, shift_id, user_id,
            type as checkin_type,
            lat, lng, accuracy_m, photo_url,
            timestamp
        "#,
        shift_id,
        user_id,
        req.lat,
        req.lng,
        req.accuracy_m,
        req.photo_url,
    )
    .fetch_one(&state.pool)
    .await?;

    // Marcamos el turno como completado
    sqlx::query!(
        "UPDATE shifts SET status = 'done' WHERE id = $1",
        shift_id
    )
    .execute(&state.pool)
    .await?;

    let message = format!(
        "Check-out registrado. Cumpliste {:.1} horas reales en este turno.",
        hours_real
    );

    Ok(Json(CheckinResponse {
        checkin,
        message,
        effective_end: None,
        hours_so_far: Some(hours_real),
    }))
}

// GET /events/:id/shifts/active — ver quién está presente ahora mismo
pub async fn active_presence(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<Vec<ActivePresence>>, AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    // Solo miembros o super admin pueden ver quién está presente
    let is_member = sqlx::query!(
        "SELECT id FROM event_members WHERE event_id = $1 AND user_id = $2 AND status = 'active'",
        event_id, user_id
    )
    .fetch_optional(&state.pool)
    .await?;

    if is_member.is_none() && !claims.is_super_admin {
        return Err(AppError::Unauthorized(
            "Solo los miembros del evento pueden ver la presencia activa".to_string()
        ));
    }

    // Traemos todos los turnos activos con el check-in más reciente
    // Un turno 'active' = alguien hizo check-in y todavía no hizo check-out
    let presence = sqlx::query_as!(
        ActivePresence,
        r#"
        SELECT
            s.user_id,
            u.name as user_name,
            s.id as shift_id,
            ci.timestamp as checkin_time,
            s.scheduled_end,
            CASE
                WHEN EXTRACT(EPOCH FROM (ci.timestamp - s.scheduled_start))/60
                     > e.late_tolerance_minutes
                THEN ci.timestamp + (s.scheduled_end - s.scheduled_start)
                ELSE s.scheduled_end
            END as "effective_end!: DateTime<Utc>",
            ci.lat,
            ci.lng
        FROM shifts s
        JOIN events e ON e.id = s.event_id
        JOIN users u ON u.id = s.user_id
        JOIN checkins ci ON ci.shift_id = s.id AND ci.type = 'check_in'
        WHERE s.event_id = $1 AND s.status = 'active'
        ORDER BY ci.timestamp ASC
        "#,
        event_id
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(presence))
}

// GET /events/:id/shifts/all — para el admin: ver todos los turnos del evento
pub async fn list_all_shifts(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<Vec<ShiftWithUser>>, AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    // Solo admins del evento o super admin
    crate::routes::events::verify_event_admin(
        &state.pool, event_id, user_id, claims.is_super_admin
    ).await?;

    let shifts = sqlx::query_as!(
        ShiftWithUser,
        r#"
        SELECT
            s.id, s.event_id, s.user_id,
            u.name as user_name,
            s.shift_type,
            s.scheduled_start,
            s.scheduled_end,
            s.status,
            ci_in.timestamp as checkin_time,
            ci_out.timestamp as checkout_time,
            CASE
                WHEN ci_in.timestamp IS NOT NULL AND
                     EXTRACT(EPOCH FROM (ci_in.timestamp - s.scheduled_start))/60
                     > e.late_tolerance_minutes
                THEN ci_in.timestamp + (s.scheduled_end - s.scheduled_start)
                ELSE s.scheduled_end
            END as "effective_end: DateTime<Utc>"
        FROM shifts s
        JOIN events e ON e.id = s.event_id
        JOIN users u ON u.id = s.user_id
        LEFT JOIN checkins ci_in ON ci_in.shift_id = s.id AND ci_in.type = 'check_in'
        LEFT JOIN checkins ci_out ON ci_out.shift_id = s.id AND ci_out.type = 'check_out'
        WHERE s.event_id = $1
        AND s.status NOT IN ('cancelled', 'rejected')
        ORDER BY s.scheduled_start ASC
        "#,
        event_id
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(shifts))
}