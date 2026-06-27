// Una "struct" en Rust es como un objeto con campos tipados
// #[derive(Clone)] permite copiar/clonar esta estructura cuando sea necesario
// (Axum necesita poder clonarla para pasarla a cada handler)
#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub port: u16,
}

impl Config {
    // "impl" define los métodos de una estructura
    // Este método "new" lee las variables de entorno y construye el Config
    // Si falta alguna variable obligatoria, el programa termina con un error claro
    pub fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .expect("DATABASE_URL no está definida en .env"),
            jwt_secret: std::env::var("JWT_SECRET")
                .expect("JWT_SECRET no está definida en .env"),
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())  // si no existe, usa 3000
                .parse()                                   // convierte String a número
                .expect("PORT debe ser un número válido"),
        }
    }
}