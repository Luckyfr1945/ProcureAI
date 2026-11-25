use sqlx::SqlitePool;
use crate::domain::EvidenceItem;

pub async fn add_evidence_item(
    pool: &SqlitePool,
    item: &EvidenceItem,
) -> anyhow::Result<i64> {
    let id = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO evidence_items (
            supplier_id, offer_id, field_path, field_value_json, source_id,
            evidence_quote, confidence, verification_status, verified_by_user_id, verified_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        RETURNING id
        "#,
    )
    .bind(item.supplier_id)
    .bind(item.offer_id)
    .bind(&item.field_path)
    .bind(&item.field_value_json)
    .bind(item.source_id)
    .bind(&item.evidence_quote)
    .bind(item.confidence)
    .bind(&item.verification_status)
    .bind(item.verified_by_user_id)
    .bind(&item.verified_at)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

pub async fn get_evidence_by_supplier(
    pool: &SqlitePool,
    supplier_id: i64,
) -> anyhow::Result<Vec<EvidenceItem>> {
    let items = sqlx::query_as::<_, EvidenceItem>(
        r#"
        SELECT id, supplier_id, offer_id, field_path, field_value_json, source_id,
               evidence_quote, confidence, verification_status, verified_by_user_id, verified_at
        FROM evidence_items
        WHERE supplier_id = ?
        ORDER BY id ASC
        "#,
    )
    .bind(supplier_id)
    .fetch_all(pool)
    .await?;

    Ok(items)
}

pub async fn get_evidence_by_offer(
    pool: &SqlitePool,
    offer_id: i64,
) -> anyhow::Result<Vec<EvidenceItem>> {
    let items = sqlx::query_as::<_, EvidenceItem>(
        r#"
        SELECT id, supplier_id, offer_id, field_path, field_value_json, source_id,
               evidence_quote, confidence, verification_status, verified_by_user_id, verified_at
        FROM evidence_items
        WHERE offer_id = ?
        ORDER BY id ASC
        "#,
    )
    .bind(offer_id)
    .fetch_all(pool)
    .await?;

    Ok(items)
}

pub async fn verify_evidence(
    pool: &SqlitePool,
    evidence_id: i64,
    user_id: i64,
    status: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE evidence_items
        SET verification_status = ?, verified_by_user_id = ?, verified_at = datetime('now')
        WHERE id = ?
        "#,
    )
    .bind(status)
    .bind(user_id)
    .bind(evidence_id)
    .execute(pool)
    .await?;

    Ok(())
}

