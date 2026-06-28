use axum::{
    extract::{Path, State},
    Extension,
    Json,
};
use uuid::Uuid;
use crate::{
    auth::Claims,
    errors::AppError,
    models::metrics::{PersonMetrics, RankingEntry},
    routes::AuthState,
};

pub async fn get_metrics(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<Vec<PersonMetrics>>, AppError> {

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    let is_admin = sqlx::query!(
        r#"
        SELECT id FROM event_members
        WHERE event_id = $1 AND user_id = $2
        AND role = 'admin' AND status = 'active'
        "#,
        event_id, user_id
    )
    .fetch_optional(&state.pool)
    .await?
    .is_some() || claims.is_super_admin;

    let event = sqlx::query!(
        r#"
        SELECT min_total_hours, night_start_time, night_end_time,
               requires_night_shift
        FROM events WHERE id = $1
        "#,
        event_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Evento no encontrado".to_string()))?;

    let rows = sqlx::query!(
        r#"
        SELECT
            em.user_id,
            u.name as user_name,
            u.email as user_email,

            COALESCE((
                SELECT SUM(
                    EXTRACT(EPOCH FROM (s.scheduled_end - s.scheduled_start))::float8 / 3600.0
                )
                FROM shifts s
                WHERE s.event_id = $1
                AND s.user_id = em.user_id
                AND s.shift_type = 'scheduled'
                AND s.status NOT IN ('cancelled', 'rejected')
            ), 0.0)::float8 as "hours_scheduled!: f64",

            COALESCE((
                SELECT SUM(
                    EXTRACT(EPOCH FROM (co.timestamp - ci.timestamp))::float8 / 3600.0
                )
                FROM shifts s
                JOIN checkins ci ON ci.shift_id = s.id AND ci.type = 'check_in'
                JOIN checkins co ON co.shift_id = s.id AND co.type = 'check_out'
                WHERE s.event_id = $1
                AND s.user_id = em.user_id
                AND s.status = 'done'
            ), 0.0)::float8 as "hours_real!: f64",

            COALESCE((
                SELECT SUM(c.hour_bonus)::float8
                FROM contributions c
                WHERE c.event_id = $1
                AND c.user_id = em.user_id
                AND c.status = 'approved'
            ), 0.0)::float8 as "contributions_bonus!: f64",

            COALESCE((
                SELECT EXTRACT(EPOCH FROM (
                    fa.checkin_time - fc.opens_at
                ))::float8 / 3600.0
                FROM final_attendance fa
                JOIN final_checkpoints fc ON fc.id = fa.final_checkpoint_id
                WHERE fc.event_id = $1
                AND fa.user_id = em.user_id
                AND fa.status = 'present'
                LIMIT 1
            ), 0.0)::float8 as "final_hours!: f64",

            EXISTS(
                SELECT 1 FROM final_attendance fa
                JOIN final_checkpoints fc ON fc.id = fa.final_checkpoint_id
                WHERE fc.event_id = $1
                AND fa.user_id = em.user_id
                AND fa.status = 'present'
            ) as "final_checkpoint_present!: bool",

            EXISTS(
                SELECT 1 FROM shifts s
                JOIN checkins ci ON ci.shift_id = s.id AND ci.type = 'check_in'
                JOIN checkins co ON co.shift_id = s.id AND co.type = 'check_out'
                WHERE s.event_id = $1
                AND s.user_id = em.user_id
                AND s.status = 'done'
                AND (
                    ($2::time IS NULL OR $3::time IS NULL)
                    OR (
                        s.scheduled_start::time <= $3::time
                        AND s.scheduled_end::time >= $2::time
                    )
                )
            ) as "night_shift_completed!: bool"

        FROM event_members em
        JOIN users u ON u.id = em.user_id
        WHERE em.event_id = $1 AND em.status = 'active'
        AND ($4::bool OR em.user_id = $5)
        ORDER BY u.name ASC
        "#,
        event_id,
        event.night_start_time,
        event.night_end_time,
        is_admin,
        user_id,
    )
    .fetch_all(&state.pool)
    .await?;

    let min_hours = event.min_total_hours.unwrap_or(0.0);

    let metrics = rows.into_iter().map(|r| {
        let hours_real = r.hours_real;
        let hours_with_final = hours_real + r.final_hours;
        let hours_total = hours_with_final + r.contributions_bonus;
        let meets_minimum = min_hours == 0.0 || hours_real >= min_hours;

        PersonMetrics {
            user_id: r.user_id,
            user_name: r.user_name,
            user_email: r.user_email,
            hours_scheduled: r.hours_scheduled,
            hours_real,
            hours_with_final,
            hours_total,
            contributions_bonus: r.contributions_bonus,
            final_checkpoint_present: r.final_checkpoint_present,
            night_shift_completed: r.night_shift_completed,
            meets_minimum,
        }
    }).collect();

    Ok(Json(metrics))
}

pub async fn get_ranking(
    State(state): State<AuthState>,
    Extension(claims): Extension<Claims>,
    Path(event_id): Path<Uuid>,
) -> Result<Json<Vec<RankingEntry>>, AppError> {

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
            "Solo los miembros pueden ver el ranking".to_string()
        ));
    }

    let event = sqlx::query!(
        "SELECT min_total_hours FROM events WHERE id = $1",
        event_id
    )
    .fetch_one(&state.pool)
    .await?;

    let rows = sqlx::query!(
        r#"
        SELECT
            em.user_id,
            u.name as user_name,

            COALESCE((
                SELECT SUM(
                    EXTRACT(EPOCH FROM (co.timestamp - ci.timestamp))::float8 / 3600.0
                )
                FROM shifts s
                JOIN checkins ci ON ci.shift_id = s.id AND ci.type = 'check_in'
                JOIN checkins co ON co.shift_id = s.id AND co.type = 'check_out'
                WHERE s.event_id = $1 AND s.user_id = em.user_id AND s.status = 'done'
            ), 0.0)::float8 as "hours_real!: f64",

            COALESCE((
                SELECT SUM(c.hour_bonus)::float8
                FROM contributions c
                WHERE c.event_id = $1
                AND c.user_id = em.user_id
                AND c.status = 'approved'
            ), 0.0)::float8 as "contributions_bonus!: f64",

            COALESCE((
                SELECT EXTRACT(EPOCH FROM (fa.checkin_time - fc.opens_at))::float8 / 3600.0
                FROM final_attendance fa
                JOIN final_checkpoints fc ON fc.id = fa.final_checkpoint_id
                WHERE fc.event_id = $1
                AND fa.user_id = em.user_id
                AND fa.status = 'present'
                LIMIT 1
            ), 0.0)::float8 as "final_hours!: f64",

            EXISTS(
                SELECT 1 FROM final_attendance fa
                JOIN final_checkpoints fc ON fc.id = fa.final_checkpoint_id
                WHERE fc.event_id = $1
                AND fa.user_id = em.user_id
                AND fa.status = 'present'
            ) as "final_checkpoint_present!: bool",

            EXISTS(
                SELECT 1 FROM shifts s
                JOIN checkins ci ON ci.shift_id = s.id AND ci.type = 'check_in'
                JOIN checkins co ON co.shift_id = s.id AND co.type = 'check_out'
                WHERE s.event_id = $1
                AND s.user_id = em.user_id
                AND s.status = 'done'
            ) as "night_shift_completed!: bool"

        FROM event_members em
        JOIN users u ON u.id = em.user_id
        WHERE em.event_id = $1 AND em.status = 'active'
        "#,
        event_id,
    )
    .fetch_all(&state.pool)
    .await?;

    let min_hours = event.min_total_hours.unwrap_or(0.0);

    let mut entries: Vec<RankingEntry> = rows.into_iter().map(|r| {
        let hours_real = r.hours_real;
        let hours_total = hours_real + r.final_hours + r.contributions_bonus;
        let meets_minimum = min_hours == 0.0 || hours_real >= min_hours;

        RankingEntry {
            position: 0,
            user_id: r.user_id,
            user_name: r.user_name,
            hours_total,
            hours_real,
            meets_minimum,
            night_shift_completed: r.night_shift_completed,
            final_checkpoint_present: r.final_checkpoint_present,
            is_eligible: meets_minimum,
        }
    }).collect();

    entries.sort_by(|a, b| b.hours_total.partial_cmp(&a.hours_total).unwrap());

    let mut position = 1i64;
    for entry in entries.iter_mut() {
        if entry.meets_minimum {
            entry.position = position;
            position += 1;
        }
    }

    Ok(Json(entries))
}