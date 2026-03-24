use crate::error::ApiError;
use sqlx::PgPool;

pub async fn update_site_authority(pool: &PgPool) -> Result<usize, ApiError> {
    // 基于入链数量计算简化的 authority 分数
    // 使用对数缩放：authority = 0.5 + log10(inlink_count + 1) * 0.2
    let result = sqlx::query(
        r#"
        update documents
        set site_authority = 0.5 + log(greatest(inlink_count, 0) + 1) * 0.2
        where inlink_count > 0
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| ApiError::Internal(e.into()))?;

    Ok(result.rows_affected() as usize)
}
