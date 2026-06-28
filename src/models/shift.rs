use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Un turno asignado a una persona
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Shift {
    pub id: Uuid,
    pub event_id: Uuid,
    pub user_id: Uuid,
    pub shift_type: String,
    pub slot_id: Option<Uuid>,
    pub scheduled_start: DateTime<Utc>,
    pub scheduled_end: DateTime<Utc>,
    pub status: String,
    pub original_scheduled_start: Option<DateTime<Utc>>,
    pub adjustment_reason: Option<String>,
    pub approved_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

// Turno con datos del usuario incluidos — para el admin
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ShiftWithUser {
    pub id: Uuid,
    pub event_id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub shift_type: String,
    pub scheduled_start: DateTime<Utc>,
    pub scheduled_end: DateTime<Utc>,
    pub status: String,
    pub checkin_time: Option<DateTime<Utc>>,    // hora real de entrada
    pub checkout_time: Option<DateTime<Utc>>,   // hora real de salida
    pub effective_end: Option<DateTime<Utc>>,   // hasta cuándo debe quedarse (corrido si llegó tarde)
}

// Para crear un turno extra (espontáneo)
#[derive(Debug, Deserialize)]
pub struct CreateExtraShiftRequest {
    pub scheduled_start: DateTime<Utc>,
    pub scheduled_end: DateTime<Utc>,
    pub notes: Option<String>,   // ej. "Voy porque tengo tiempo libre"
}

// Para hacer check-in o check-out
#[derive(Debug, Deserialize)]
pub struct CheckinRequest {
    pub lat: Option<f64>,        // GPS opcional, no bloquea
    pub lng: Option<f64>,
    pub accuracy_m: Option<f64>, // precisión del GPS en metros
    pub photo_url: Option<String>, // URL de la foto subida (opcional)
}

// Registro de check-in/out
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Checkin {
    pub id: Uuid,
    pub shift_id: Uuid,
    pub user_id: Uuid,
    pub checkin_type: String,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub accuracy_m: Option<f64>,
    pub photo_url: Option<String>,
    pub timestamp: DateTime<Utc>,
}

// Respuesta del check-in con info útil para el usuario
#[derive(Debug, Serialize)]
pub struct CheckinResponse {
    pub checkin: Checkin,
    pub message: String,
    pub effective_end: Option<DateTime<Utc>>,  // hasta cuándo debe quedarse
    pub hours_so_far: Option<f64>,             // horas acumuladas en este turno (en checkout)
}

// Vista de quién está presente ahora mismo
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ActivePresence {
    pub user_id: Uuid,
    pub user_name: String,
    pub shift_id: Uuid,
    pub checkin_time: DateTime<Utc>,
    pub scheduled_end: DateTime<Utc>,
    pub effective_end: DateTime<Utc>,    // hora real de salida esperada
    pub lat: Option<f64>,
    pub lng: Option<f64>,
}