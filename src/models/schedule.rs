use chrono::{DateTime, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Un slot del cronograma — una franja horaria disponible
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ScheduleSlot {
    pub id: Uuid,
    pub event_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub capacity: i32,
    pub created_by: Uuid,
    pub origin: String,
    pub status: String,
    pub approved_by: Option<Uuid>,
}

// Slot con información extra: cuántos cupos quedan y si el usuario actual está anotado
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ScheduleSlotWithAvailability {
    pub id: Uuid,
    pub event_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub capacity: i32,
    pub origin: String,
    pub status: String,
    pub signups_count: i64,       // cuántos ya se anotaron
    pub available_spots: i64,     // cupos disponibles
}

// Para crear un slot
#[derive(Debug, Deserialize)]
pub struct CreateSlotRequest {
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub capacity: Option<i32>,    // default 1
}

// Para anotarse en uno o varios slots a la vez
#[derive(Debug, Deserialize)]
pub struct SignupRequest {
    pub slot_ids: Vec<Uuid>,      // permite elegir múltiples slots de una vez
}

// Una inscripción a un slot
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SlotSignup {
    pub id: Uuid,
    pub slot_id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub user_email: Option<String>,
    pub signed_up_at: DateTime<Utc>,
    pub status: String,
}

// Enlace temporal para que alguien se anote sin cuenta
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ScheduleLink {
    pub id: Uuid,
    pub event_id: Uuid,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

// Para crear un enlace temporal
#[derive(Debug, Deserialize)]
pub struct CreateScheduleLinkRequest {
    pub expires_in_hours: Option<i64>,  // por cuántas horas es válido, default 72
}

// Lo que ve alguien que entra al enlace público
#[derive(Debug, Serialize)]
pub struct PublicScheduleView {
    pub event_name: String,
    pub venue_name: String,
    pub expires_at: DateTime<Utc>,
    pub slots: Vec<ScheduleSlotWithAvailability>,
}

// Para anotarse sin cuenta via enlace temporal
#[derive(Debug, Deserialize)]
pub struct GuestSignupRequest {
    pub name: String,
    pub phone: String,
    pub slot_ids: Vec<Uuid>,    // puede elegir varios slots
}

// Resultado de anotarse via enlace temporal
#[derive(Debug, Serialize)]
pub struct GuestSignupResponse {
    pub message: String,
    pub user_id: Uuid,          // el ID del usuario invitado creado
    pub signups: Vec<Uuid>,     // IDs de los slot_signups creados
}