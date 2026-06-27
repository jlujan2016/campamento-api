use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Esta struct representa un usuario tal como está en la base de datos
// #[derive(sqlx::FromRow)] le dice a SQLx cómo convertir una fila de DB
// automáticamente a esta struct — mapea columna por columna por nombre
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: Option<String>,     // Option = puede ser NULL (usuarios invitados)
    pub name: String,
    pub phone: Option<String>,
    pub is_super_admin: bool,
    pub is_guest: bool,
    pub created_at: DateTime<Utc>,
}

// Esta struct es lo que el cliente manda para registrarse
// #[derive(Deserialize)] permite convertir JSON → esta struct automáticamente
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub name: String,
    pub phone: Option<String>,
}

// Esta struct es lo que el cliente manda para hacer login
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

// Esta struct es lo que el servidor devuelve después de login/registro exitoso
// Nunca incluye el password_hash — solo datos seguros de mostrar
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,      // el JWT que el cliente debe guardar
    pub user: UserPublic,   // datos del usuario sin información sensible
}

// Vista pública del usuario (sin password_hash)
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct UserPublic {
    pub id: Uuid,
    pub email: Option<String>,
    pub name: String,
    pub phone: Option<String>,
    pub is_super_admin: bool,
    pub is_guest: bool,
    pub created_at: DateTime<Utc>,
}