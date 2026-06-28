use axum::{
    middleware,
    routing::{get, post, put},
    Router,
};
use sqlx::PgPool;

mod health;
pub mod auth;
pub mod events;
pub mod schedule;
pub mod shifts;
pub mod replacements;

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
        // Eventos
        .route("/events", get(events::list_events).post(events::create_event))
        .route("/events/:id", get(events::get_event).put(events::update_event))
        .route("/events/:id/join", post(events::join_event))
        .route("/events/:id/members", get(events::list_members))
        // Cronograma
        .route("/events/:id/slots",
            get(schedule::list_slots).post(schedule::create_slot))
        .route("/events/:id/slots/:slot_id/signups",
            get(schedule::list_slot_signups))
        .route("/events/:id/signup-slots", post(schedule::signup_slots))
        .route("/events/:id/schedule-link", post(schedule::create_schedule_link))
        // Turnos
        .route("/events/:id/shifts",
            get(shifts::list_my_shifts).post(shifts::create_extra_shift))
        .route("/events/:id/shifts/active", get(shifts::active_presence))
        .route("/events/:id/shifts/all", get(shifts::list_all_shifts))
        .route("/events/:id/shifts/gaps", get(replacements::list_gaps))
        // Check-in / check-out
        .route("/shifts/:id/checkin", post(shifts::do_checkin))
        .route("/shifts/:id/checkout", post(shifts::do_checkout))
        // Reemplazos
        .route("/shifts/:id/replacement", post(replacements::create_replacement))
        .route("/shifts/:id/replacement/:rid",
            put(replacements::respond_replacement))
        .route("/shifts/:id/mark-gap", post(replacements::mark_gap))
        // Retiros
        .route("/events/:id/members/:uid/withdraw",
            post(replacements::withdraw_member))
        .layer(middleware::from_fn_with_state(
            auth_state.clone(),
            auth::require_auth,
        ))
        .with_state(auth_state);

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
}