use crate::domain::{Product, SupplierOffer};
use sqlx::SqlitePool;

pub async fn upsert_product(pool: &SqlitePool, product: &Product) -> anyhow::Result<i64> {
    // Never join different manufacturers just because their model numbers coincide.
    let candidates = sqlx::query_as::<_, Product>(
        "SELECT * FROM products WHERE lower(coalesce(brand,'')) = lower(?)",
    )
    .bind(product.brand.as_deref().unwrap_or("").trim())
    .fetch_all(pool)
    .await?;
    let identity = |s: &str| {
        s.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
    };
    for candidate in candidates {
        let same = match (
            product.part_number.as_deref().filter(|p| !p.is_empty()),
            candidate.part_number.as_deref().filter(|p| !p.is_empty()),
        ) {
            (Some(a), Some(b)) => identity(a) == identity(b),
            (None, None) => {
                identity(&product.canonical_name) == identity(&candidate.canonical_name)
            }
            _ => false,
        };
        if same {
            return Ok(candidate.id);
        }
    }

    let id = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO products (canonical_name, brand, model, part_number, category_key, attributes_json, created_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))
        RETURNING id
        "#,
    )
    .bind(&product.canonical_name)
    .bind(&product.brand)
    .bind(&product.model)
    .bind(&product.part_number)
    .bind(&product.category_key)
    .bind(&product.attributes_json)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

pub async fn get_product_by_id(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<Product>> {
    let prod = sqlx::query_as::<_, Product>(
        r#"
        SELECT id, canonical_name, brand, model, part_number, category_key, attributes_json, created_at, updated_at
        FROM products
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(prod)
}

pub async fn add_supplier_offer(pool: &SqlitePool, offer: &SupplierOffer) -> anyhow::Result<i64> {
    let id = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO supplier_offers (
            supplier_id, product_id, source_id, product_url, description,
            attributes_json, price_amount, currency, price_basis, moq,
            lead_time, availability_text, product_fit_score, evidence_completeness_score,
            extracted_at, needs_review
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), 0)
        RETURNING id
        "#,
    )
    .bind(offer.supplier_id)
    .bind(offer.product_id)
    .bind(offer.source_id)
    .bind(&offer.product_url)
    .bind(&offer.description)
    .bind(&offer.attributes_json)
    .bind(offer.price_amount)
    .bind(&offer.currency)
    .bind(offer.price_basis.as_deref().unwrap_or("unknown"))
    .bind(&offer.moq)
    .bind(&offer.lead_time)
    .bind(&offer.availability_text)
    .bind(offer.product_fit_score)
    .bind(offer.evidence_completeness_score)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

pub async fn get_supplier_offers(
    pool: &SqlitePool,
    supplier_id: i64,
) -> anyhow::Result<Vec<SupplierOffer>> {
    let offers = sqlx::query_as::<_, SupplierOffer>(
        r#"
        SELECT id, supplier_id, product_id, source_id, product_url, description,
               attributes_json, CAST(price_amount AS REAL) AS price_amount, currency, price_basis, moq,
               lead_time, availability_text, product_fit_score, evidence_completeness_score,
               extracted_at
        FROM supplier_offers
        WHERE supplier_id = ? AND needs_review = 0
        ORDER BY product_fit_score DESC, evidence_completeness_score DESC
        "#,
    )
    .bind(supplier_id)
    .fetch_all(pool)
    .await?;

    Ok(offers)
}

pub async fn get_offer_by_id(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<SupplierOffer>> {
    let offer = sqlx::query_as::<_, SupplierOffer>(
        r#"
        SELECT id, supplier_id, product_id, source_id, product_url, description,
               attributes_json, CAST(price_amount AS REAL) AS price_amount, currency, price_basis, moq,
               lead_time, availability_text, product_fit_score, evidence_completeness_score,
               extracted_at
        FROM supplier_offers
        WHERE id = ? AND needs_review = 0
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(offer)
}

pub async fn count_published_prices(pool: &SqlitePool) -> anyhow::Result<i64> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM supplier_offers WHERE needs_review=0 AND price_amount IS NOT NULL AND price_amount > 0",
    )
    .fetch_one(pool)
    .await?;
    Ok(count)
}
