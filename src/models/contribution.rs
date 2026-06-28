use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Tipo de aporte definido por el admin del evento
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ContributionType {
    pub id: Uuid,
    pub event_id: Uuid,
    pub type_key: String,
    pub label: String,
    pub default_hour_bonus: f64,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

// Para crear un tipo de aporte
#[derive(Debug, Deserialize)]
pub struct CreateContributionTypeRequest {
    pub type_key: String,            // ej. "tent", "mattress", "food"
    pub label: String,               // ej. "Carpa", "Colchón", "Comida"
    pub default_hour_bonus: f64,     // ej. 5.0 = vale 5 horas
}

// Un aporte registrado por un participante
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Contribution {
    pub id: Uuid,
    pub event_id: Uuid,
    pub user_id: Uuid,
    pub contribution_type_id: Uuid,
    pub description: Option<String>,
    pub hour_bonus: f64,
    pub approved_by: Option<Uuid>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

// Vista enriquecida del aporte con nombres
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ContributionWithDetails {
    pub id: Uuid,
    pub event_id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub contribution_type_id: Uuid,
    pub type_label: String,
    pub description: Option<String>,
    pub hour_bonus: f64,
    pub approved_by: Option<Uuid>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

// Para registrar un aporte
#[derive(Debug, Deserialize)]
pub struct CreateContributionRequest {
    pub contribution_type_id: Uuid,
    pub description: Option<String>,
    // Si no viene, se usa el default_hour_bonus del tipo
    pub hour_bonus_override: Option<f64>,
}

// Para aprobar o rechazar un aporte
#[derive(Debug, Deserialize)]
pub struct ApproveContributionRequest {
    pub action: String,              // "approve" o "reject"
    pub hour_bonus_override: Option<f64>, // el admin puede ajustar el bono al aprobar
}

// Para el tramo final
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct FinalCheckpoint {
    pub id: Uuid,
    pub event_id: Uuid,
    pub opens_at: DateTime<Utc>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFinalCheckpointRequest {
    pub opens_at: DateTime<Utc>,
    pub description: Option<String>,
}

// Registro de presencia en el tramo final
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct FinalAttendance {
    pub id: Uuid,
    pub final_checkpoint_id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub checkin_time: DateTime<Utc>,
    pub lat: Option<f64>,
    pub lng: Option<f64>,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct FinalAttendanceRequest {
    pub lat: Option<f64>,
    pub lng: Option<f64>,
}