use axum::{
    extract::{Request, State},
    http::{header::AUTHORIZATION, StatusCode},
    middleware::Next,
    response::Response,
    Json,
};
use sqlx::PgPool;
use uuid::Uuid;
use crate::{
    auth::{generate_token, hash_password, verify_password, verify_token},
    errors::AppError,
    models::user::{AuthResponse, LoginRequest, RegisterRequest, UserPublic},
};

// Estado compartido que los handlers necesitan
// Además del pool de DB, necesitan el JWT secret para firmar tokens
#[derive(Clone)]
pub struct AuthState {
    pub pool: PgPool,
    pub jwt_secret: String,
}

// POST /auth/register
// Crea una cuenta nueva y devuelve un JWT listo para usar
pub async fn register(
    State(state): State<AuthState>,
    Json(req): Json<RegisterRequest>,   // Json<> deserializa el body del request
) -> Result<(StatusCode, Json<AuthResponse>), AppError> {

    // Validación básica antes de tocar la DB
    if req.email.is_empty() || req.password.is_empty() || req.name.is_empty() {
        return Err(AppError::Validation("Email, contraseña y nombre son requeridos".to_string()));
    }

    if req.password.len() < 8 {
        return Err(AppError::Validation("La contraseña debe tener al menos 8 caracteres".to_string()));
    }

    // Verificamos que el email no esté ya registrado
    // El operador ? propaga el error automáticamente si algo falla
    let existing = sqlx::query!(
        "SELECT id FROM users WHERE email = $1",
        req.email
    )
    .fetch_optional(&state.pool)
    .await?;

    if existing.is_some() {
        return Err(AppError::Validation("El email ya está registrado".to_string()));
    }

    // Hasheamos la contraseña ANTES de guardarla
    let password_hash = hash_password(&req.password)?;

    // Insertamos el usuario y recuperamos sus datos completos
    let user = sqlx::query_as!(
        UserPublic,
        r#"
        INSERT INTO users (email, password_hash, name, phone, is_guest)
        VALUES ($1, $2, $3, $4, false)
        RETURNING id, email, name, phone, is_super_admin, is_guest, created_at
        "#,
        req.email,
        password_hash,
        req.name,
        req.phone
    )
    .fetch_one(&state.pool)
    .await?;

    // Generamos el JWT para que pueda usar el sistema de inmediato
    let token = generate_token(user.id, user.is_super_admin, &state.jwt_secret)?;

    Ok((StatusCode::CREATED, Json(AuthResponse { token, user })))
}

// POST /auth/login
pub async fn login(
    State(state): State<AuthState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {

    // Buscamos el usuario por email
    // fetch_optional devuelve None si no existe (sin tirar error)
    let user_record = sqlx::query!(
        r#"
        SELECT id, email, name, phone, password_hash,
               is_super_admin, is_guest, created_at
        FROM users
        WHERE email = $1 AND is_guest = false
        "#,
        req.email
    )
    .fetch_optional(&state.pool)
    .await?;

    // Usamos el mismo mensaje de error tanto si el email no existe
    // como si la contraseña es incorrecta — evita dar pistas a atacantes
    let record = user_record
        .ok_or_else(|| AppError::Unauthorized("Credenciales incorrectas".to_string()))?;

    let password_hash = record.password_hash
        .ok_or_else(|| AppError::Unauthorized("Credenciales incorrectas".to_string()))?;

    // Verificamos la contraseña contra el hash guardado
    if !verify_password(&req.password, &password_hash)? {
        return Err(AppError::Unauthorized("Credenciales incorrectas".to_string()));
    }

    let user = UserPublic {
        id: record.id,
        email: record.email,
        name: record.name,
        phone: record.phone,
        is_super_admin: record.is_super_admin,
        is_guest: record.is_guest,
        created_at: record.created_at,
    };

    let token = generate_token(user.id, user.is_super_admin, &state.jwt_secret)?;

    Ok(Json(AuthResponse { token, user }))
}

// GET /auth/me — ruta protegida, requiere JWT válido
pub async fn me(
    State(state): State<AuthState>,
    req: Request,                       // acceso al request completo para leer el header
) -> Result<Json<UserPublic>, AppError> {

    // Extraemos el token del header "Authorization: Bearer <token>"
    let token = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Unauthorized("Token no proporcionado".to_string()))?;

    // Verificamos el token y extraemos los claims (datos del usuario)
    let claims = verify_token(token, &state.jwt_secret)?;

    // Buscamos el usuario en la DB usando el ID del token
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::Unauthorized("Token inválido".to_string()))?;

    let user = sqlx::query_as!(
        UserPublic,
        r#"
        SELECT id, email, name, phone, is_super_admin, is_guest, created_at
        FROM users WHERE id = $1
        "#,
        user_id
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Usuario no encontrado".to_string()))?;

    Ok(Json(user))
}

// Middleware de autenticación — protege rutas que requieren login
// Se puede aplicar a grupos de rutas enteros, no solo handler por handler
pub async fn require_auth(
    State(state): State<AuthState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {

    let token = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Unauthorized("Token requerido".to_string()))?;

    // Verificamos el token y guardamos los claims en el request
    // para que los handlers que vengan después puedan leerlos
    let claims = verify_token(token, &state.jwt_secret)?;
    req.extensions_mut().insert(claims);

    // Pasamos el request al siguiente handler
    Ok(next.run(req).await)
}