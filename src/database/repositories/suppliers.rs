use crate::domain::{Supplier, SupplierSource};
use sqlx::SqlitePool;

pub async fn upsert_supplier(pool: &SqlitePool, supplier: &Supplier) -> anyhow::Result<i64> {
    // If normalized_domain exists, try matching by domain first
    if let Some(domain) = &supplier.normalized_domain {
        if !domain.is_empty() && !crate::services::entity_resolution::is_shared_supplier_platform(domain) {
            let existing_id = sqlx::query_scalar::<_, i64>(
                "SELECT id FROM suppliers WHERE normalized_domain = ? LIMIT 1",
            )
            .bind(domain)
            .fetch_optional(pool)
            .await?;

            if let Some(id) = existing_id {
                sqlx::query(
                    r#"
                    UPDATE suppliers
                    SET canonical_name = coalesce(nullif(canonical_name, ''), ?),
                        website = coalesce(website, ?),
                        country_code = CASE WHEN EXISTS(SELECT 1 FROM supplier_profile_corrections c WHERE c.supplier_id=suppliers.id) THEN country_code ELSE coalesce(?, country_code) END,
                        country_name = CASE WHEN EXISTS(SELECT 1 FROM supplier_profile_corrections c WHERE c.supplier_id=suppliers.id) THEN country_name ELSE coalesce(?, country_name) END,
                        address = CASE WHEN EXISTS(SELECT 1 FROM supplier_profile_corrections c WHERE c.supplier_id=suppliers.id) THEN address ELSE coalesce(?, address) END,
                        supplier_type = coalesce(nullif(?, 'unknown'), supplier_type),
                        business_category = coalesce(business_category, ?),
                        public_email = CASE WHEN EXISTS(SELECT 1 FROM supplier_profile_corrections c WHERE c.supplier_id=suppliers.id) THEN public_email ELSE coalesce(?, public_email) END,
                        public_phone = CASE WHEN EXISTS(SELECT 1 FROM supplier_profile_corrections c WHERE c.supplier_id=suppliers.id) THEN public_phone ELSE coalesce(?, public_phone) END,
                        contact_page_url = coalesce(contact_page_url, ?),
                        updated_at = datetime('now')
                    WHERE id = ?
                    "#,
                )
                .bind(&supplier.canonical_name)
                .bind(&supplier.website)
                .bind(&supplier.country_code)
                .bind(&supplier.country_name)
                .bind(&supplier.address)
                .bind(supplier.supplier_type.as_deref().unwrap_or("unknown"))
                .bind(&supplier.business_category)
                .bind(&supplier.public_email)
                .bind(&supplier.public_phone)
                .bind(&supplier.contact_page_url)
                .bind(id)
                .execute(pool)
                .await?;

                return Ok(id);
            }
        }
    }

    // Try matching by normalized_name if domain didn't match
    let existing_name_id = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM suppliers WHERE normalized_name = ? AND coalesce(country_code,'') = coalesce(?,'') AND (normalized_domain IS NULL OR ? IS NULL OR normalized_domain=?) LIMIT 1",
    )
    .bind(&supplier.normalized_name).bind(&supplier.country_code).bind(&supplier.normalized_domain).bind(&supplier.normalized_domain)
    .fetch_optional(pool)
    .await?;

    if let Some(id) = existing_name_id {
        return Ok(id);
    }

    let id = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO suppliers (
            canonical_name, normalized_name, website, normalized_domain,
            country_code, country_name, address, supplier_type,
            business_category, public_email, public_phone, contact_page_url,
            status, created_at, updated_at
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))
        RETURNING id
        "#,
    )
    .bind(&supplier.canonical_name)
    .bind(&supplier.normalized_name)
    .bind(&supplier.website)
    .bind(&supplier.normalized_domain)
    .bind(&supplier.country_code)
    .bind(&supplier.country_name)
    .bind(&supplier.address)
    .bind(supplier.supplier_type.as_deref().unwrap_or("unknown"))
    .bind(&supplier.business_category)
    .bind(&supplier.public_email)
    .bind(&supplier.public_phone)
    .bind(&supplier.contact_page_url)
    .bind(&supplier.status)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

pub async fn get_supplier_by_id(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<Supplier>> {
    let supplier = sqlx::query_as::<_, Supplier>(
        r#"
        SELECT id, canonical_name, normalized_name, website, normalized_domain,
               country_code, country_name, address, supplier_type, business_category,
               public_email, public_phone, contact_page_url, status, created_at, updated_at
        FROM suppliers
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(supplier)
}

pub async fn get_supplier_by_domain(
    pool: &SqlitePool,
    domain: &str,
) -> anyhow::Result<Option<Supplier>> {
    let supplier = sqlx::query_as::<_, Supplier>(
        r#"
        SELECT id, canonical_name, normalized_name, website, normalized_domain,
               country_code, country_name, address, supplier_type, business_category,
               public_email, public_phone, contact_page_url, status, created_at, updated_at
        FROM suppliers
        WHERE normalized_domain = ?
        LIMIT 1
        "#,
    )
    .bind(domain)
    .fetch_optional(pool)
    .await?;

    Ok(supplier)
}

pub async fn list_suppliers(
    pool: &SqlitePool,
    limit: i64,
    offset: i64,
    status_filter: Option<&str>,
    country_filter: Option<&str>,
) -> anyhow::Result<Vec<Supplier>> {
    let mut query_str = String::from(
        r#"
        SELECT id, canonical_name, normalized_name, website, normalized_domain,
               country_code, country_name, address, supplier_type, business_category,
               public_email, public_phone, contact_page_url, status, created_at, updated_at
        FROM suppliers
        WHERE 1=1
        "#,
    );

    if status_filter.is_some() {
        query_str.push_str(" AND status = ?");
    }
    if country_filter.is_some() {
        query_str.push_str(" AND (country_name LIKE ? OR country_code = ?)");
    }

    query_str.push_str(" ORDER BY updated_at DESC LIMIT ? OFFSET ?");

    let mut q = sqlx::query_as::<_, Supplier>(&query_str);

    if let Some(status) = status_filter {
        q = q.bind(status);
    }
    if let Some(country) = country_filter {
        let pattern = format!("%{}%", country);
        q = q.bind(pattern).bind(country);
    }

    q = q.bind(limit).bind(offset);

    let suppliers = q.fetch_all(pool).await?;
    Ok(suppliers)
}

pub async fn count_suppliers(pool: &SqlitePool) -> anyhow::Result<i64> {
    let count = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM suppliers")
        .fetch_one(pool)
        .await?;
    Ok(count)
}

pub async fn count_verified_suppliers(pool: &SqlitePool) -> anyhow::Result<i64> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM suppliers WHERE status = 'verified_by_purchasing'",
    )
    .fetch_one(pool)
    .await?;
    Ok(count)
}

pub async fn update_supplier_status(
    pool: &SqlitePool,
    id: i64,
    status: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE suppliers
        SET status = ?, updated_at = datetime('now')
        WHERE id = ?
        "#,
    )
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_supplier_profile(
    pool: &SqlitePool,
    id: i64,
    canonical_name: Option<&str>,
    public_email: Option<&str>,
    public_phone: Option<&str>,
    supplier_type: Option<&str>,
    address: Option<&str>,
    status: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE suppliers
        SET canonical_name = coalesce(?, canonical_name),
            public_email = coalesce(?, public_email),
            public_phone = coalesce(?, public_phone),
            supplier_type = coalesce(?, supplier_type),
            address = coalesce(?, address),
            status = coalesce(?, status),
            updated_at = datetime('now')
        WHERE id = ?
        "#,
    )
    .bind(canonical_name)
    .bind(public_email)
    .bind(public_phone)
    .bind(supplier_type)
    .bind(address)
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn add_supplier_source(
    pool: &SqlitePool,
    source: &SupplierSource,
) -> anyhow::Result<i64> {
    let id = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO supplier_sources (
            supplier_id, url, canonical_url, page_type, page_title,
            content_hash, retrieved_at, http_status, source_type
        )
        VALUES (?, ?, ?, ?, ?, ?, datetime('now'), ?, ?)
        ON CONFLICT(supplier_id, canonical_url) DO UPDATE SET
            page_title = coalesce(excluded.page_title, supplier_sources.page_title),
            retrieved_at = datetime('now'),
            http_status = excluded.http_status
        RETURNING id
        "#,
    )
    .bind(source.supplier_id)
    .bind(&source.url)
    .bind(&source.canonical_url)
    .bind(source.page_type.as_deref().unwrap_or("company"))
    .bind(&source.page_title)
    .bind(&source.content_hash)
    .bind(source.http_status.unwrap_or(200))
    .bind(source.source_type.as_deref().unwrap_or("web"))
    .fetch_one(pool)
    .await?;

    Ok(id)
}

pub async fn get_supplier_sources(
    pool: &SqlitePool,
    supplier_id: i64,
) -> anyhow::Result<Vec<SupplierSource>> {
    let sources = sqlx::query_as::<_, SupplierSource>(
        r#"
        SELECT id, supplier_id, url, canonical_url, page_type, page_title,
               content_hash, retrieved_at, http_status, source_type
        FROM supplier_sources
        WHERE supplier_id = ?
        ORDER BY retrieved_at DESC
        "#,
    )
    .bind(supplier_id)
    .fetch_all(pool)
    .await?;

    Ok(sources)
}
