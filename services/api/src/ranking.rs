use crate::error::ApiError;
use sqlx::PgPool;

pub async fn update_site_authority(pool: &PgPool) -> Result<usize, ApiError> {
    // Recompute inlink_count from link_edges table
    let _ = sqlx::query(
        "UPDATE documents d SET inlink_count = COALESCE(sub.cnt, 0)
         FROM (
             SELECT target_url, COUNT(DISTINCT source_url) as cnt
             FROM link_edges
             GROUP BY target_url
         ) sub
         WHERE d.canonical_url = sub.target_url
           AND d.inlink_count IS DISTINCT FROM COALESCE(sub.cnt, 0)::int",
    )
    .execute(pool)
    .await
    .map_err(|e| ApiError::Internal(e.into()))?;

    // Update authority scores using log scaling
    let result = sqlx::query(
        "UPDATE documents
         SET site_authority = 0.5 + log(greatest(inlink_count, 0) + 1) * 0.2
         WHERE inlink_count > 0",
    )
    .execute(pool)
    .await
    .map_err(|e| ApiError::Internal(e.into()))?;

    Ok(result.rows_affected() as usize)
}
