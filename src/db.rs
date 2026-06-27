// "use" importa tipos específicos de una librería — como "import" en Python
use sqlx::postgres::{PgPool, PgPoolOptions};

// Esta función crea el pool de conexiones
// "async fn" significa que es asíncrona — puede pausarse mientras espera
// que la base de datos responda, sin bloquear el hilo del servidor
// "->" indica qué tipo devuelve la función
pub async fn create_pool(database_url: &str) -> PgPool {
    PgPoolOptions::new()
        // máximo 10 conexiones simultáneas a la base de datos
        // (el pool las reutiliza en vez de abrir y cerrar una por petición)
        .max_connections(10)
        .connect(database_url)
        .await
        // si no se puede conectar, termina con este mensaje de error
        .expect("No se pudo conectar a la base de datos")
}