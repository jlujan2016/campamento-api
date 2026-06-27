use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension,
    Json,
};
use sqlx::PgPool;
use uuid::Uuid;
use crate::{
    auth::Claims,
    errors::AppError,
    models::event::{
        CreateEventRequest, Event, EventMember,
        JoinEventRequest, UpdateEventRequest,
    },
    routes::AuthState,
};

// POST /events — crear evento (solo super admin)
pub async fn create_event(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,  // claims viene del middleware require_auth
    Json(req): Json<CreateEventRequest>,
) -> Result<(StatusCode, Json<Event>), AppError> {

    // Solo el super admin puede crear eventos
    if !claims.is_super_admin {
        return Err(AppError::Unauthorized(
            "Solo el super admin puede crear eventos".to_string()
        ));
    }

    // Validaciones básicas
    if req.name.is_empty() || req.venue_name.is_empty() {
        return Err(AppError::Validation(
            "Nombre y lugar son requeridos".to_string()
        ));
    }

    if req.end_date <= req.start_date {
        return Err(AppError::Validation(
            "La fecha de fin debe ser posterior a la de inicio".to_string()
        ));
    }

    let creator_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    // Insertamos el evento
    let event = sqlx::query_as!(
        Event,
        r#"
        INSERT INTO events (
            name, venue_name, lat, lng, start_date, end_date,
            created_by, min_shift_hours, max_shift_hours,
            night_start_time, night_end_time, requires_night_shift,
            min_total_hours, late_tolerance_minutes
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        RETURNING *
        "#,
        req.name,
        req.venue_name,
        req.lat,
        req.lng,
        req.start_date,
        req.end_date,
        creator_id,
        req.min_shift_hours.unwrap_or(1.0),
        req.max_shift_hours,
        req.night_start_time,
        req.night_end_time,
        req.requires_night_shift.unwrap_or(false),
        req.min_total_hours,
        req.late_tolerance_minutes.unwrap_or(0.0),
    )
    .fetch_one(&state.pool)
    .await?;

    // El creador automáticamente se convierte en admin del evento
    sqlx::query!(
        r#"
        INSERT INTO event_members (event_id, user_id, role)
        VALUES ($1, $2, 'admin')
        "#,
        event.id,
        creator_id,
    )
    .execute(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(event)))
}

// GET /events — listar todos los eventos activos
pub async fn list_events(
    State(state): State<AuthState>,
) -> Result<Json<Vec<Event>>, AppError> {

    let events = sqlx::query_as!(
        Event,
        "SELECT * FROM events WHERE status = 'active' ORDER BY start_date ASC"
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(events))
}

// GET /events/:id — ver un evento específico
pub async fn get_event(
    State(state): State<AuthState>,
    Path(event_id): Path<Uuid>,   // Path extrae el :id de la URL automáticamente
) -> Result<Json<Event>, AppError> {

    let event = sqlx::query_as!(
        Event,
        "SELECT * FROM events WHERE id = $1",
        event_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Evento no encontrado".to_string()))?;

    Ok(Json(event))
}

// PUT /events/:id — editar evento (solo admin del evento o super admin)
pub async fn update_event(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
    Json(req): Json<UpdateEventRequest>,
) -> Result<Json<Event>, AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    // Verificamos que el usuario es admin del evento o super admin
    verify_event_admin(&state.pool, event_id, user_id, claims.is_super_admin).await?;

    // Actualizamos solo los campos que vienen en el request
    // COALESCE(valor_nuevo, valor_actual) = si el nuevo es NULL, mantiene el actual
    let event = sqlx::query_as!(
        Event,
        r#"
        UPDATE events SET
            name                  = COALESCE($1, name),
            venue_name            = COALESCE($2, venue_name),
            status                = COALESCE($3, status),
            min_shift_hours       = COALESCE($4, min_shift_hours),
            max_shift_hours       = COALESCE($5, max_shift_hours),
            night_start_time      = COALESCE($6, night_start_time),
            night_end_time        = COALESCE($7, night_end_time),
            requires_night_shift  = COALESCE($8, requires_night_shift),
            min_total_hours       = COALESCE($9, min_total_hours),
            late_tolerance_minutes = COALESCE($10, late_tolerance_minutes)
        WHERE id = $11
        RETURNING *
        "#,
        req.name,
        req.venue_name,
        req.status,
        req.min_shift_hours,
        req.max_shift_hours,
        req.night_start_time,
        req.night_end_time,
        req.requires_night_shift,
        req.min_total_hours,
        req.late_tolerance_minutes,
        event_id,
    )
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(event))
}

// POST /events/:id/join — unirse a un evento
pub async fn join_event(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
    Json(req): Json<JoinEventRequest>,
) -> Result<(StatusCode, Json<EventMember>), AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    // Verificamos que el evento existe
    let event_exists = sqlx::query!(
        "SELECT id FROM events WHERE id = $1 AND status = 'active'",
        event_id
    )
    .fetch_optional(&state.pool)
    .await?;

    if event_exists.is_none() {
        return Err(AppError::NotFound("Evento no encontrado o inactivo".to_string()));
    }

    // Verificamos que no esté ya en el evento
    let already_member = sqlx::query!(
        "SELECT id FROM event_members WHERE event_id = $1 AND user_id = $2",
        event_id,
        user_id
    )
    .fetch_optional(&state.pool)
    .await?;

    if already_member.is_some() {
        return Err(AppError::Validation("Ya eres miembro de este evento".to_string()));
    }

    // Solo super admin puede agregar admins de evento
    let role = match req.role.as_deref() {
        Some("admin") if claims.is_super_admin => "admin",
        Some("admin") => return Err(AppError::Unauthorized(
            "Solo el super admin puede asignar admins de evento".to_string()
        )),
        _ => "participant",
    };

    // Insertamos el miembro y devolvemos sus datos con join al usuario
    let member = sqlx::query_as!(
        EventMember,
        r#"
        WITH inserted AS (
            INSERT INTO event_members (event_id, user_id, role)
            VALUES ($1, $2, $3)
            RETURNING *
        )
        SELECT
            i.id, i.event_id, i.user_id,
            u.name as user_name, u.email as user_email,
            i.role, i.status, i.joined_at
        FROM inserted i
        JOIN users u ON u.id = i.user_id
        "#,
        event_id,
        user_id,
        role,
    )
    .fetch_one(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(member)))
}

// GET /events/:id/members — ver miembros del evento
pub async fn list_members(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<Vec<EventMember>>, AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    // Solo miembros del evento o super admin pueden ver la lista
    let is_member = sqlx::query!(
        "SELECT id FROM event_members WHERE event_id = $1 AND user_id = $2 AND status = 'active'",
        event_id,
        user_id
    )
    .fetch_optional(&state.pool)
    .await?;

    if is_member.is_none() && !claims.is_super_admin {
        return Err(AppError::Unauthorized(
            "Solo los miembros del evento pueden ver la lista".to_string()
        ));
    }

    let members = sqlx::query_as!(
        EventMember,
        r#"
        SELECT
            em.id, em.event_id, em.user_id,
            u.name as user_name, u.email as user_email,
            em.role, em.status, em.joined_at
        FROM event_members em
        JOIN users u ON u.id = em.user_id
        WHERE em.event_id = $1 AND em.status = 'active'
        ORDER BY em.joined_at ASC
        "#,
        event_id
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(members))
}

// Función auxiliar — verifica que el usuario es admin del evento o super admin
// La usamos en update_event y la vamos a reusar en muchos otros handlers
pub async fn verify_event_admin(
    pool: &PgPool,
    event_id: Uuid,
    user_id: Uuid,
    is_super_admin: bool,
) -> Result<(), AppError> {
    if is_super_admin {
        return Ok(());  // super admin puede todo
    }

    let is_admin = sqlx::query!(
        r#"
        SELECT id FROM event_members
        WHERE event_id = $1 AND user_id = $2
        AND role = 'admin' AND status = 'active'
        "#,
        event_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;

    if is_admin.is_none() {
        return Err(AppError::Unauthorized(
            "Se requiere ser admin del evento".to_string()
        ));
    }

    Ok(())
}