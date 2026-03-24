use axum::{
    Json,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::Serialize;

use crate::QueryState;

#[derive(Serialize)]
struct RateLimitError {
    error: String,
}

/// Sliding-window rate limiter using Redis INCR + EXPIRE.
/// Limits requests per IP per window.
pub async fn rate_limit_middleware(
    state: axum::extract::State<QueryState>,
    request: Request,
    next: Next,
) -> Response {
    // Extract client IP from X-Forwarded-For header or fall back to "unknown"
    let client_ip = request
        .headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let window_secs: u64 = 60;
    let max_requests: i64 = 120; // 120 requests per minute per IP

    let key = format!(
        "fv:rl:{}:{}",
        client_ip,
        chrono::Utc::now().timestamp() / window_secs as i64
    );

    // Try to increment and check the rate limit in Redis
    match state
        .db
        .redis_client
        .get_multiplexed_async_connection()
        .await
    {
        Ok(mut conn) => {
            let count: i64 = match redis::pipe()
                .atomic()
                .incr(&key, 1i64)
                .expire(&key, window_secs as i64)
                .query_async::<Vec<i64>>(&mut conn)
                .await
            {
                Ok(results) => results.first().copied().unwrap_or(1),
                Err(_) => {
                    // Redis error — allow request through (fail-open)
                    return next.run(request).await;
                }
            };

            if count > max_requests {
                return (
                    StatusCode::TOO_MANY_REQUESTS,
                    Json(RateLimitError {
                        error: "rate limit exceeded, try again later".to_string(),
                    }),
                )
                    .into_response();
            }
        }
        Err(_) => {
            // Redis unavailable — fail-open, allow request through
        }
    }

    next.run(request).await
}
