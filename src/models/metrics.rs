use serde::Serialize;
use uuid::Uuid;

// Las 4 métricas de transparencia para una persona en un evento
#[derive(Debug, Serialize)]
pub struct PersonMetrics {
    pub user_id: Uuid,
    pub user_name: String,
    pub user_email: Option<String>,

    // Métrica 1: horas que debió cumplir según cronograma asignado
    pub hours_scheduled: f64,

    // Métrica 2: horas reales acumuladas (check-in/out reales)
    // Esta es la métrica que determina si cumple el mínimo exigido
    pub hours_real: f64,

    // Métrica 3: horas reales + tramo final (si se presentó)
    pub hours_with_final: f64,

    // Métrica 4: total con aportes — esta define el orden oficial de la fila
    pub hours_total: f64,

    // Información adicional
    pub contributions_bonus: f64,     // solo el bono de aportes
    pub final_checkpoint_present: bool, // si se presentó al tramo final
    pub night_shift_completed: bool,  // si cumplió al menos un turno noche
    pub meets_minimum: bool,          // si cumple el mínimo de horas reales exigido
}

// El ranking final ordenado por métrica 4
#[derive(Debug, Serialize)]
pub struct RankingEntry {
    pub position: i64,
    pub user_id: Uuid,
    pub user_name: String,
    pub hours_total: f64,         // métrica 4
    pub hours_real: f64,          // métrica 2
    pub meets_minimum: bool,
    pub night_shift_completed: bool,
    pub final_checkpoint_present: bool,
    pub is_eligible: bool,        // cumple mínimo Y está presente en tramo final (si aplica)
}