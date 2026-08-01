#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub port: u16,
    pub host: String,
    pub cors_origins: Vec<String>,          // lista separada por comas en .env
    pub telegram_bot_token: Option<String>,
    pub frontend_dist: Option<String>,  // ← nuevo: ruta a dist/ del frontend
    pub public_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        // DATABASE_URL — obligatoria
        let database_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL debe estar definida en .env");

        // JWT_SECRET — obligatoria y debe ser estable
        // Si cambia, todos los tokens existentes quedan inválidos (logout masivo)
        let jwt_secret = std::env::var("JWT_SECRET")
            .expect("JWT_SECRET debe estar definida en .env");

        // APP_PORT — default 8090 para no chocar con otros servicios
        let port = std::env::var("APP_PORT")
            .unwrap_or_else(|_| "8090".to_string())
            .parse::<u16>()
            .expect("APP_PORT debe ser un número entre 1 y 65535");

        // APP_HOST — default 0.0.0.0 (escucha en todas las interfaces)
        // En producción con Cloudflare tunnel: 127.0.0.1 es más seguro
        let host = std::env::var("APP_HOST")
            .unwrap_or_else(|_| "0.0.0.0".to_string());

        // CORS_ORIGINS — lista separada por comas
        // En producción modo mismo-origen NO se necesita CORS
        // En desarrollo: http://localhost:5174,http://192.168.1.X:5174
        let cors_origins = std::env::var("CORS_ORIGINS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // TELEGRAM_BOT_TOKEN — opcional
        let telegram_bot_token = std::env::var("TELEGRAM_BOT_TOKEN").ok();

        // Ruta a la carpeta dist/ del frontend
        // En producción: ../campamento-web/dist o ruta absoluta
        // Si no está definida, no sirve archivos estáticos (solo API)
        let frontend_dist = std::env::var("FRONTEND_DIST").ok();
        let public_url = std::env::var("PUBLIC_URL")
            .unwrap_or_else(|_| "http://localhost:5174".to_string());

        Self {
            database_url,
            jwt_secret,
            port,
            host,
            cors_origins,
            telegram_bot_token,
            frontend_dist,
            public_url,
        }
    }
}