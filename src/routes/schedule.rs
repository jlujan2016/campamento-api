use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension,
    Json,
};
use chrono::{Duration, Utc};
use uuid::Uuid;
use crate::{
    auth::Claims,
    errors::AppError,
    models::schedule::{
        CreateSlotRequest, CreateScheduleLinkRequest,
        GuestSignupRequest, GuestSignupResponse,
        PublicScheduleView, ScheduleLink,
        ScheduleSlot, ScheduleSlotWithAvailability,
        SignupRequest, SlotSignup,
    },
    routes::{AuthState, events::verify_event_admin},
};

// POST /events/:id/slots — crear slot (solo admin del evento)
pub async fn create_slot(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
    Json(req): Json<CreateSlotRequest>,
) -> Result<(StatusCode, Json<ScheduleSlot>), AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    // Verificamos que es admin del evento
    verify_event_admin(&state.pool, event_id, user_id, claims.is_super_admin).await?;

    // Validamos que el slot tenga sentido temporalmente
    if req.end_time <= req.start_time {
        return Err(AppError::Validation(
            "La hora de fin debe ser posterior a la de inicio".to_string()
        ));
    }

    // Verificamos duración mínima del evento
    let event = sqlx::query!(
        "SELECT min_shift_hours, max_shift_hours FROM events WHERE id = $1",
        event_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Evento no encontrado".to_string()))?;

    let duration_hours = (req.end_time - req.start_time).num_minutes() as f64 / 60.0;

    if duration_hours < event.min_shift_hours {
        return Err(AppError::Validation(format!(
            "El slot debe durar al menos {} horas", event.min_shift_hours
        )));
    }

    if let Some(max) = event.max_shift_hours {
        if duration_hours > max {
            return Err(AppError::Validation(format!(
                "El slot no puede durar más de {} horas", max
            )));
        }
    }

    let slot = sqlx::query_as!(
        ScheduleSlot,
        r#"
        INSERT INTO schedule_slots (event_id, start_time, end_time, capacity, created_by)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING *
        "#,
        event_id,
        req.start_time,
        req.end_time,
        req.capacity.unwrap_or(1),
        user_id,
    )
    .fetch_one(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(slot)))
}

// GET /events/:id/slots — listar slots del evento con disponibilidad
pub async fn list_slots(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<Vec<ScheduleSlotWithAvailability>>, AppError> {

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
            "Solo los miembros del evento pueden ver el cronograma".to_string()
        ));
    }

    // Casteamos capacity a INT8 para que coincida con COUNT que también es INT8
    let slots_raw = sqlx::query!(
        r#"
        SELECT
            s.id,
            s.event_id,
            s.start_time,
            s.end_time,
            s.capacity::int8 as "capacity!: i64",
            s.origin,
            s.status,
            COUNT(ss.id) as "signups_count!: i64"
        FROM schedule_slots s
        LEFT JOIN slot_signups ss ON ss.slot_id = s.id AND ss.status = 'confirmed'
        WHERE s.event_id = $1 AND s.status = 'approved'
        GROUP BY s.id
        ORDER BY s.start_time ASC
        "#,
        event_id
    )
    .fetch_all(&state.pool)
    .await?;

    let slots = slots_raw.into_iter().map(|r| {
        let available_spots = (r.capacity - r.signups_count).max(0);
        ScheduleSlotWithAvailability {
            id: r.id,
            event_id: r.event_id,
            start_time: r.start_time,
            end_time: r.end_time,
            capacity: r.capacity as i32,
            origin: r.origin,
            status: r.status,
            signups_count: r.signups_count,
            available_spots,
        }
    }).collect();

    Ok(Json(slots))
}

// POST /events/:id/slots/:sid/signup — anotarse en uno o varios slots
pub async fn signup_slots(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
    Json(req): Json<SignupRequest>,
) -> Result<(StatusCode, Json<Vec<SlotSignup>>), AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    // Verificar que es miembro del evento
    let is_member = sqlx::query!(
        "SELECT id FROM event_members WHERE event_id = $1 AND user_id = $2 AND status = 'active'",
        event_id, user_id
    )
    .fetch_optional(&state.pool)
    .await?;

    if is_member.is_none() && !claims.is_super_admin {
        return Err(AppError::Unauthorized(
            "Solo los miembros del evento pueden anotarse en slots".to_string()
        ));
    }

    if req.slot_ids.is_empty() {
        return Err(AppError::Validation("Debes elegir al menos un slot".to_string()));
    }

    let mut signups = Vec::new();

    // Procesamos cada slot elegido
    // Usamos una transacción para que si uno falla, ninguno se guarde
    // (atomicidad: todo o nada)
    let mut tx = state.pool.begin().await?;

    for slot_id in &req.slot_ids {
        // Verificamos que el slot pertenece al evento y está aprobado
        let slot = sqlx::query!(
            r#"
            SELECT id, capacity FROM schedule_slots
            WHERE id = $1 AND event_id = $2 AND status = 'approved'
            "#,
            slot_id, event_id
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Slot {} no encontrado", slot_id)))?;

        // Verificamos que hay cupo disponible
        let current_count = sqlx::query!(
            "SELECT COUNT(*) as count FROM slot_signups WHERE slot_id = $1 AND status = 'confirmed'",
            slot_id
        )
        .fetch_one(&mut *tx)
        .await?
        .count
        .unwrap_or(0);

        if current_count >= slot.capacity as i64 {
            return Err(AppError::Validation(
                format!("El slot {} ya no tiene cupos disponibles", slot_id)
            ));
        }

        // Verificamos que no esté ya anotado en este slot
        let already = sqlx::query!(
            "SELECT id FROM slot_signups WHERE slot_id = $1 AND user_id = $2",
            slot_id, user_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        if already.is_some() {
            return Err(AppError::Validation(
                format!("Ya estás anotado en el slot {}", slot_id)
            ));
        }

        // Insertamos el signup y creamos el shift correspondiente
        let signup = sqlx::query_as!(
            SlotSignup,
            r#"
            WITH inserted_signup AS (
                INSERT INTO slot_signups (slot_id, user_id)
                VALUES ($1, $2)
                RETURNING *
            ),
            inserted_shift AS (
                INSERT INTO shifts (event_id, user_id, shift_type, slot_id, scheduled_start, scheduled_end, status)
                SELECT $3, $2, 'scheduled', $1, s.start_time, s.end_time, 'approved'
                FROM schedule_slots s WHERE s.id = $1
            )
            SELECT
                is2.id, is2.slot_id, is2.user_id,
                u.name as user_name, u.email as user_email,
                is2.signed_up_at, is2.status
            FROM inserted_signup is2
            JOIN users u ON u.id = is2.user_id
            "#,
            slot_id,
            user_id,
            event_id,
        )
        .fetch_one(&mut *tx)
        .await?;

        signups.push(signup);
    }

    // Confirmamos la transacción — todos los signups se guardan juntos
    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(signups)))
}

// GET /events/:id/slots/:slot_id/signups — ver quién está anotado en un slot
pub async fn list_slot_signups(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path((event_id, slot_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<SlotSignup>>, AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    verify_event_admin(&state.pool, event_id, user_id, claims.is_super_admin).await?;

    let signups = sqlx::query_as!(
        SlotSignup,
        r#"
        SELECT
            ss.id, ss.slot_id, ss.user_id,
            u.name as user_name, u.email as user_email,
            ss.signed_up_at, ss.status
        FROM slot_signups ss
        JOIN users u ON u.id = ss.user_id
        WHERE ss.slot_id = $1
        ORDER BY ss.signed_up_at ASC
        "#,
        slot_id
    )
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(signups))
}

// POST /events/:id/schedule-link — generar enlace temporal
pub async fn create_schedule_link(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
    Json(req): Json<CreateScheduleLinkRequest>,
) -> Result<(StatusCode, Json<ScheduleLink>), AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    verify_event_admin(&state.pool, event_id, user_id, claims.is_super_admin).await?;

    // Generamos un token aleatorio seguro usando UUID v4
    // En producción podrías usar un token más corto y amigable
    let token = Uuid::new_v4().to_string().replace("-", "");

    let hours = req.expires_in_hours.unwrap_or(72);
    let expires_at = Utc::now() + Duration::hours(hours);

    let link = sqlx::query_as!(
        ScheduleLink,
        r#"
        INSERT INTO schedule_links (event_id, token, expires_at, created_by)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        "#,
        event_id,
        token,
        expires_at,
        user_id,
    )
    .fetch_one(&state.pool)
    .await?;

    Ok((StatusCode::CREATED, Json(link)))
}

// GET /schedule/:token — vista pública del cronograma (sin cuenta)
pub async fn public_schedule(
    State(state): State<AuthState>,
    Path(token): Path<String>,
) -> Result<Json<PublicScheduleView>, AppError> {

    let link = sqlx::query!(
        r#"
        SELECT sl.event_id, sl.expires_at, e.name as event_name, e.venue_name
        FROM schedule_links sl
        JOIN events e ON e.id = sl.event_id
        WHERE sl.token = $1
        "#,
        token
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Enlace no encontrado o inválido".to_string()))?;

    if link.expires_at < Utc::now() {
        return Err(AppError::Validation("Este enlace ha expirado".to_string()));
    }

    let slots_raw = sqlx::query!(
        r#"
        SELECT
            s.id,
            s.event_id,
            s.start_time,
            s.end_time,
            s.capacity::int8 as "capacity!: i64",
            s.origin,
            s.status,
            COUNT(ss.id) as "signups_count!: i64"
        FROM schedule_slots s
        LEFT JOIN slot_signups ss ON ss.slot_id = s.id AND ss.status = 'confirmed'
        WHERE s.event_id = $1 AND s.status = 'approved'
        GROUP BY s.id
        ORDER BY s.start_time ASC
        "#,
        link.event_id
    )
    .fetch_all(&state.pool)
    .await?;

    let slots = slots_raw.into_iter().map(|r| {
        let available_spots = (r.capacity - r.signups_count).max(0);
        ScheduleSlotWithAvailability {
            id: r.id,
            event_id: r.event_id,
            start_time: r.start_time,
            end_time: r.end_time,
            capacity: r.capacity as i32,
            origin: r.origin,
            status: r.status,
            signups_count: r.signups_count,
            available_spots,
        }
    }).collect();

    Ok(Json(PublicScheduleView {
        event_name: link.event_name,
        venue_name: link.venue_name,
        expires_at: link.expires_at,
        slots,
    }))
}

// POST /schedule/:token/signup — anotarse sin cuenta via enlace temporal
pub async fn guest_signup(
    State(state): State<AuthState>,
    Path(token): Path<String>,
    Json(req): Json<GuestSignupRequest>,
) -> Result<(StatusCode, Json<GuestSignupResponse>), AppError> {

    // Validaciones básicas
    if req.name.is_empty() || req.phone.is_empty() {
        return Err(AppError::Validation("Nombre y teléfono son requeridos".to_string()));
    }

    if req.slot_ids.is_empty() {
        return Err(AppError::Validation("Debes elegir al menos un slot".to_string()));
    }

    // Verificamos el enlace
    let link = sqlx::query!(
        "SELECT event_id, expires_at FROM schedule_links WHERE token = $1",
        token
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Enlace no encontrado".to_string()))?;

    if link.expires_at < Utc::now() {
        return Err(AppError::Validation("Este enlace ha expirado".to_string()));
    }

    let mut tx = state.pool.begin().await?;

    // Buscamos si ya existe un invitado con ese teléfono en este evento
    // (para no crear duplicados si vuelve a usar el enlace)
    let existing_user = sqlx::query!(
        r#"
        SELECT u.id FROM users u
        JOIN event_members em ON em.user_id = u.id
        WHERE u.phone = $1 AND u.is_guest = true AND em.event_id = $2
        "#,
        req.phone,
        link.event_id
    )
    .fetch_optional(&mut *tx)
    .await?;

    let user_id = if let Some(existing) = existing_user {
        // Ya existe, usamos el mismo usuario
        existing.id
    } else {
        // Creamos un usuario invitado nuevo
        let new_user = sqlx::query!(
            r#"
            INSERT INTO users (name, phone, is_guest)
            VALUES ($1, $2, true)
            RETURNING id
            "#,
            req.name,
            req.phone,
        )
        .fetch_one(&mut *tx)
        .await?;

        // Lo agregamos como participante del evento
        sqlx::query!(
            "INSERT INTO event_members (event_id, user_id, role) VALUES ($1, $2, 'participant')",
            link.event_id,
            new_user.id,
        )
        .execute(&mut *tx)
        .await?;

        new_user.id
    };

    // Procesamos cada slot elegido (misma lógica que signup_slots)
    let mut signup_ids = Vec::new();

    for slot_id in &req.slot_ids {
        let slot = sqlx::query!(
            r#"
            SELECT id, capacity FROM schedule_slots
            WHERE id = $1 AND event_id = $2 AND status = 'approved'
            "#,
            slot_id, link.event_id
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Slot {} no encontrado", slot_id)))?;

        let current_count = sqlx::query!(
            "SELECT COUNT(*) as count FROM slot_signups WHERE slot_id = $1 AND status = 'confirmed'",
            slot_id
        )
        .fetch_one(&mut *tx)
        .await?
        .count
        .unwrap_or(0);

        if current_count >= slot.capacity as i64 {
            return Err(AppError::Validation(
                format!("El slot {} ya no tiene cupos", slot_id)
            ));
        }

        let already = sqlx::query!(
            "SELECT id FROM slot_signups WHERE slot_id = $1 AND user_id = $2",
            slot_id, user_id
        )
        .fetch_optional(&mut *tx)
        .await?;

        if already.is_some() {
            continue; // si ya está anotado en este slot, lo saltamos silenciosamente
        }

        let signup = sqlx::query!(
            r#"
            WITH inserted_signup AS (
                INSERT INTO slot_signups (slot_id, user_id)
                VALUES ($1, $2)
                RETURNING id
            ),
            inserted_shift AS (
                INSERT INTO shifts (event_id, user_id, shift_type, slot_id, scheduled_start, scheduled_end, status)
                SELECT $3, $2, 'scheduled', $1, s.start_time, s.end_time, 'approved'
                FROM schedule_slots s WHERE s.id = $1
            )
            SELECT id FROM inserted_signup
            "#,
            slot_id,
            user_id,
            link.event_id,
        )
        .fetch_one(&mut *tx)
        .await?;

        signup_ids.push(signup.id);
    }

    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(GuestSignupResponse {
        message: format!(
            "Te anotaste en {} turno(s) exitosamente",
            signup_ids.len()
        ),
        user_id,
        signups: signup_ids,
    })))
}