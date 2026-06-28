use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Solicitud de reemplazo
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ShiftReplacement {
    pub id: Uuid,
    pub shift_id: Uuid,
    pub original_user_id: Uuid,
    pub replacement_user_id: Uuid,
    pub requested_by: String,
    pub status: String,
    pub covers_start: Option<DateTime<Utc>>,  // NULL = reemplazo total
    pub covers_end: Option<DateTime<Utc>>,    // NULL = reemplazo total
    pub created_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
}

// Vista enriquecida con nombres de usuarios
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ShiftReplacementWithUsers {
    pub id: Uuid,
    pub shift_id: Uuid,
    pub original_user_id: Uuid,
    pub original_user_name: String,
    pub replacement_user_id: Uuid,
    pub replacement_user_name: String,
    pub requested_by: String,
    pub status: String,
    pub covers_start: Option<DateTime<Utc>>,
    pub covers_end: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
}

// Para solicitar un reemplazo
#[derive(Debug, Deserialize)]
pub struct CreateReplacementRequest {
    // ID del usuario que va a cubrir el turno
    pub replacement_user_id: Uuid,
    // Si es None = reemplazo total del turno
    // Si tiene valor = reemplazo parcial (cubre solo ese tramo)
    pub covers_start: Option<DateTime<Utc>>,
    pub covers_end: Option<DateTime<Utc>>,
}

// Para confirmar o rechazar un reemplazo
#[derive(Debug, Deserialize)]
pub struct RespondReplacementRequest {
    pub action: String,   // "confirm" o "reject"
}

// Turno con vacío sin resolver — para el dashboard del admin
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct GapShift {
    pub shift_id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub scheduled_start: DateTime<Utc>,
    pub scheduled_end: DateTime<Utc>,
    pub gap_since: Option<DateTime<Utc>>,   // desde cuándo hay vacío
}