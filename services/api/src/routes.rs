use axum::{
    Router,
    routing::{delete, get, patch, post, put},
};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use crate::{
    config::Config,
    handlers, rate_limit,
    state::{ControlState, QueryState, TaskState},
};

pub(crate) fn build_query_router(config: &Config, state: QueryState) -> Router {
    Router::new()
        .route("/healthz", get(handlers::search::healthz))
        .route("/readyz", get(handlers::search::readyz))
        .route("/v1/search", get(handlers::search::browser_search))
        .route("/v1/developer/search", get(handlers::search::search))
        .route("/v1/suggest", get(handlers::search::suggest))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            rate_limit::rate_limit_middleware,
        ))
        .layer(shared_cors(config))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub(crate) fn build_control_router(config: &Config, state: ControlState) -> Router {
    Router::new()
        .route("/healthz", get(handlers::search::healthz))
        .route("/readyz", get(handlers::search::readyz))
        .route(
            "/v1/users/register",
            post(handlers::developer::user_register),
        )
        .route(
            "/v1/users/session/login",
            post(handlers::developer::user_login),
        )
        .route("/v1/users/session/me", get(handlers::developer::user_me))
        .route(
            "/v1/users/session/logout",
            post(handlers::developer::user_logout),
        )
        .route(
            "/v1/users/keys",
            get(handlers::developer::user_list_keys).post(handlers::developer::user_create_key),
        )
        .route(
            "/v1/users/keys/{id}",
            delete(handlers::developer::user_revoke_key),
        )
        .route(
            "/v1/users/domains/inspect",
            get(handlers::developer::user_domain_insight),
        )
        .route(
            "/v1/users/domains/submit",
            post(handlers::developer::user_submit_domain),
        )
        .route(
            "/v1/admin/users/{user_id}/keys",
            get(handlers::admin::admin_list_user_keys).post(handlers::admin::admin_create_user_key),
        )
        .route(
            "/v1/admin/users/{user_id}/keys/{key_id}",
            delete(handlers::admin::admin_revoke_user_key),
        )
        .route(
            "/v1/admin/crawlers/{id}",
            patch(handlers::admin::admin_update_crawler)
                .delete(handlers::admin::admin_delete_crawler),
        )
        .route(
            "/v1/admin/frontier/seed",
            post(handlers::admin::admin_seed_frontier),
        )
        .route(
            "/v1/admin/crawl/overview",
            get(handlers::admin::admin_crawl_overview),
        )
        .route(
            "/v1/admin/crawl/rules",
            post(handlers::admin::admin_create_rule),
        )
        .route(
            "/v1/admin/crawl/rules/{id}",
            patch(handlers::admin::admin_update_rule).delete(handlers::admin::admin_delete_rule),
        )
        .route(
            "/v1/admin/documents",
            get(handlers::admin::admin_list_documents),
        )
        .route(
            "/v1/admin/documents/{id}",
            delete(handlers::admin::admin_delete_document),
        )
        .route(
            "/v1/admin/documents/purge-site",
            post(handlers::admin::admin_purge_site),
        )
        .route(
            "/v1/admin/system-config",
            get(handlers::admin::admin_list_system_config),
        )
        .route(
            "/v1/admin/system-config/{key}",
            put(handlers::admin::admin_set_system_config),
        )
        .route(
            "/v1/admin/crawl/jobs",
            get(handlers::admin::admin_list_jobs),
        )
        .route(
            "/v1/admin/crawl/jobs/stats",
            get(handlers::admin::admin_job_stats),
        )
        .route(
            "/v1/admin/crawl/origins",
            get(handlers::admin::admin_list_origins),
        )
        .route(
            "/v1/admin/domains/inspect",
            get(handlers::admin::admin_domain_insight),
        )
        .route(
            "/v1/admin/crawl/jobs/retry",
            post(handlers::admin::admin_retry_failed_jobs),
        )
        .route(
            "/v1/admin/crawl/jobs/stop",
            post(handlers::admin::admin_stop_all_jobs),
        )
        .route(
            "/v1/admin/crawl/jobs/completed",
            delete(handlers::admin::admin_cleanup_completed_jobs),
        )
        .route(
            "/v1/admin/crawl/jobs/failed",
            delete(handlers::admin::admin_cleanup_failed_jobs),
        )
        .route(
            "/v1/admin/users",
            get(handlers::admin::admin_list_users).post(handlers::admin::admin_create_user),
        )
        .route(
            "/v1/admin/users/{user_id}",
            patch(handlers::admin::admin_update_user).delete(handlers::admin::admin_delete_user),
        )
        .layer(shared_cors(config))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub(crate) fn build_task_router(state: TaskState) -> Router {
    Router::new()
        .route("/healthz", get(handlers::task::healthz))
        .route("/readyz", get(handlers::task::readyz))
        .route(
            "/internal/crawlers/claim",
            post(handlers::crawler::claim_jobs),
        )
        .route(
            "/internal/crawlers/report",
            post(handlers::crawler::submit_crawl_report),
        )
        .route(
            "/internal/crawlers/heartbeat",
            post(handlers::crawler::heartbeat_crawler),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

fn shared_cors(config: &Config) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(
            config
                .frontend_origin
                .split(',')
                .filter_map(|o| o.trim().parse::<axum::http::HeaderValue>().ok())
                .collect::<Vec<_>>(),
        )
        .allow_methods(Any)
        .allow_headers(Any)
}
