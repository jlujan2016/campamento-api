use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Usuario completo (uso interno, nunca se serializa directo al cliente)
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: Option<String>,
    pub name: String,
    pub phone: Option<String>,
    pub is_super_admin: bool,
    pub is_guest: bool,
    pub is_blocked: bool,
    pub created_at: DateTime<Utc>,
}

// Respuesta de auth (login/registro)
#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserPublic,
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
    pub is_blocked: bool,
    pub created_at: DateTime<Utc>,
}

// Para hacer login
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

// Para registrarse
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub name: String,
    pub phone: Option<String>,
}

// Para crear usuario desde el panel admin
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    pub name: String,
    pub phone: Option<String>,
    pub is_super_admin: Option<bool>,
}