use axum::{
    middleware,
    routing::{get, post, put},
    Router,
};
use axum::http::{header, Method, HeaderValue}; // 1. Módulos HTTP para configurar las cabeceras
use tower_http::cors::CorsLayer;                // 2. Middleware de CORS de tower-http
use sqlx::PgPool;

mod health;
pub mod auth;
pub mod events;
pub mod schedule;
pub mod shifts;
pub mod replacements;
pub mod contributions;
pub mod metrics;
pub mod telegram;

pub use auth::AuthState;

pub fn create_router(pool: PgPool, jwt_secret: String) -> Router {
    let auth_state = AuthState {
        pool: pool.clone(),
        jwt_secret,
    };

    let public_routes = Router::new()
        .route("/health", get(health::health_handler))
        .route("/auth/register", post(auth::register))
        .route("/auth/login", post(auth::login))
        .route("/schedule/:token", get(schedule::public_schedule))
        .route("/schedule/:token/signup", post(schedule::guest_signup))
        .with_state(auth_state.clone());

    let protected_routes = Router::new()
        .route("/auth/me", get(auth::me))
        .route("/events", get(events::list_events).post(events::create_event))
        .route("/events/:id", get(events::get_event).put(events::update_event))
        .route("/events/:id/join", post(events::join_event))
        .route("/events/:id/members", get(events::list_members))
        .route("/events/:id/slots",
            get(schedule::list_slots).post(schedule::create_slot))
        .route("/events/:id/slots/:slot_id/signups",
            get(schedule::list_slot_signups))
        .route("/events/:id/signup-slots", post(schedule::signup_slots))
        .route("/events/:id/schedule-link", post(schedule::create_schedule_link))
        .route("/events/:id/shifts",
            get(shifts::list_my_shifts).post(shifts::create_extra_shift))
        .route("/events/:id/shifts/active", get(shifts::active_presence))
        .route("/events/:id/shifts/all", get(shifts::list_all_shifts))
        .route("/events/:id/shifts/gaps", get(replacements::list_gaps))
        .route("/shifts/:id/checkin", post(shifts::do_checkin))
        .route("/shifts/:id/checkout", post(shifts::do_checkout))
        .route("/shifts/:id/replacement", post(replacements::create_replacement))
        .route("/shifts/:id/replacement/:rid",
            put(replacements::respond_replacement))
        .route("/shifts/:id/mark-gap", post(replacements::mark_gap))
        .route("/events/:id/members/:uid/withdraw",
            post(replacements::withdraw_member))
        .route("/events/:id/contribution-types",
            get(contributions::list_contribution_types)
            .post(contributions::create_contribution_type))
        .route("/events/:id/contributions",
            post(contributions::create_contribution))
        .route("/contributions/:id/approve",
            put(contributions::approve_contribution))
        .route("/events/:id/final-checkpoint",
            post(contributions::create_final_checkpoint))
        .route("/events/:id/final-checkpoint/attend",
            post(contributions::attend_final_checkpoint))
        .route("/events/:id/metrics", get(metrics::get_metrics))
        .route("/events/:id/ranking", get(metrics::get_ranking))
        // Telegram
        .route("/events/:id/telegram/group", post(telegram::link_group))
        .route("/telegram/link-account", post(telegram::link_account))
        .layer(middleware::from_fn_with_state(
            auth_state.clone(),
            auth::require_auth,
        ))
        .with_state(auth_state);

    // 3. Configuración estricta de la capa CORS para interactuar con React
    let cors = CorsLayer::new()
        .allow_origin([
            "http://localhost:5173".parse::<HeaderValue>().unwrap(),
            "http://192.168.100.95:5173".parse::<HeaderValue>().unwrap(),
        ])
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

    // 4. Se fusionan las rutas y se aplica el middleware globalmente al final
    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(cors) 
}
