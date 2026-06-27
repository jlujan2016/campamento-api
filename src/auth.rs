use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{Utc, Duration};
use crate::errors::AppError;

// Claims son los datos que van dentro del JWT
// Cuando el servidor verifica el token, extrae estos datos
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,          // "subject" = ID del usuario (estándar JWT)
    pub is_super_admin: bool,
    pub exp: usize,           // "expiration" = cuándo expira (timestamp Unix)
}

// Genera un hash seguro de la contraseña
// Esta función es lenta a propósito — dificulta ataques de fuerza bruta
pub fn hash_password(password: &str) -> Result<String, AppError> {
    // SaltString genera un valor aleatorio único para cada hash
    // (dos usuarios con la misma contraseña tendrán hashes distintos)
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| AppError::Validation(format!("Error al hashear contraseña: {}", e)))
}

// Verifica si una contraseña coincide con su hash guardado en DB
pub fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| AppError::Validation(format!("Hash inválido: {}", e)))?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

// Genera un JWT firmado con el secret del .env
// El token expira en 7 días
pub fn generate_token(
    user_id: Uuid,
    is_super_admin: bool,
    jwt_secret: &str,
) -> Result<String, AppError> {
    let expiration = Utc::now()
        .checked_add_signed(Duration::days(7))
        .unwrap()
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        is_super_admin,
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
    .map_err(|e| AppError::Unauthorized(format!("Error al generar token: {}", e)))
}

// Verifica y decodifica un JWT
// Si el token es inválido, expiró, o fue firmado con otro secret → error
pub fn verify_token(token: &str, jwt_secret: &str) -> Result<Claims, AppError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|e| AppError::Unauthorized(format!("Token inválido: {}", e)))
}