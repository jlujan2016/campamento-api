use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Representa un evento tal como está en la base de datos
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Event {
    pub id: Uuid,
    pub name: String,
    pub venue_name: String,
    pub lat: f64,
    pub lng: f64,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub created_by: Uuid,
    pub status: String,
    pub min_shift_hours: f64,
    pub max_shift_hours: Option<f64>,
    pub night_start_time: Option<chrono::NaiveTime>,
    pub night_end_time: Option<chrono::NaiveTime>,
    pub requires_night_shift: bool,
    pub min_total_hours: Option<f64>,
    pub late_tolerance_minutes: f64,
    pub created_at: DateTime<Utc>,
}

// Lo que el cliente manda para crear un evento
#[derive(Debug, Deserialize)]
pub struct CreateEventRequest {
    pub name: String,
    pub venue_name: String,
    pub lat: f64,
    pub lng: f64,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub min_shift_hours: Option<f64>,
    pub max_shift_hours: Option<f64>,
    pub night_start_time: Option<chrono::NaiveTime>,
    pub night_end_time: Option<chrono::NaiveTime>,
    pub requires_night_shift: Option<bool>,
    pub min_total_hours: Option<f64>,
    pub late_tolerance_minutes: Option<f64>,
}

// Lo que el cliente manda para editar un evento
// Todos los campos son Option — solo se actualizan los que vienen
#[derive(Debug, Deserialize)]
pub struct UpdateEventRequest {
    pub name: Option<String>,
    pub venue_name: Option<String>,
    pub status: Option<String>,
    pub min_shift_hours: Option<f64>,
    pub max_shift_hours: Option<f64>,
    pub night_start_time: Option<chrono::NaiveTime>,
    pub night_end_time: Option<chrono::NaiveTime>,
    pub requires_night_shift: Option<bool>,
    pub min_total_hours: Option<f64>,
    pub late_tolerance_minutes: Option<f64>,
}

// Miembro de un evento con datos del usuario incluidos
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct EventMember {
    pub id: Uuid,
    pub event_id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub user_email: Option<String>,
    pub role: String,
    pub status: String,
    pub joined_at: DateTime<Utc>,
}

// Para unirse a un evento
#[derive(Debug, Deserialize)]
pub struct JoinEventRequest {
    pub role: Option<String>,   // 'admin' o 'participant', default 'participant'
}