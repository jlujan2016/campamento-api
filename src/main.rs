mod config;
mod db;
mod errors;
mod models;
mod auth;
mod routes;
mod telegram;
mod worker;

use dotenvy::dotenv;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("campamento_api=debug".parse().unwrap()),
        )
        .init();

    let config = config::Config::from_env();
    let pool = db::create_pool(&config.database_url).await;
    info!("✅ Conexión a la base de datos establecida");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Error al aplicar migraciones");
    info!("✅ Migraciones aplicadas");

    // Iniciamos el worker de Telegram si hay token configurado
    if let Some(token) = &config.telegram_bot_token {
        let bot = telegram::TelegramBot::new(token.clone());
        let worker_pool = pool.clone();
        let worker_bot = bot.clone();

        // tokio::spawn lanza una tarea asíncrona en segundo plano
        // El servidor y el worker corren en paralelo sin bloquearse
        tokio::spawn(async move {
            worker::run_notification_worker(worker_pool, worker_bot).await;
        });

        info!("🤖 Worker de Telegram iniciado");
    } else {
        info!("⚠️  TELEGRAM_BOT_TOKEN no configurado — notificaciones desactivadas");
    }

    let app = routes::create_router(pool, config.jwt_secret);

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("No se pudo iniciar el servidor");

    info!("🚀 Servidor corriendo en http://{}", addr);

    axum::serve(listener, app)
        .await
        .expect("Error en el servidor");
}