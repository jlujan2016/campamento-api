mod config;
mod db;
mod errors;
mod models;
mod auth;
mod routes;

use dotenvy::dotenv;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // Carga el .env antes de hacer cualquier otra cosa
    dotenv().ok();

    // Inicializa el sistema de logging
    // RUST_LOG=debug muestra logs detallados, =info muestra solo lo importante
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("campamento_api=debug".parse().unwrap()),
        )
        .init();

           // Lee la configuración del .env
    let config = config::Config::from_env();

     // Crea el pool de conexiones a la base de datos
    let pool = db::create_pool(&config.database_url).await;
    info!("✅ Conexión a la base de datos establecida");

    // Aplica las migraciones automáticamente al arrancar
    // (si ya están aplicadas, las salta — es idempotente)
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Error al aplicar migraciones");
    info!("✅ Migraciones aplicadas");

    // Construye el router con todas las rutas Pasamos también el jwt_secret al router
    let app = routes::create_router(pool, config.jwt_secret);

     // Arranca el servidor
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("No se pudo iniciar el servidor");

    info!("🚀 Servidor corriendo en http://{}", addr);

    axum::serve(listener, app)
        .await
        .expect("Error en el servidor");
}