use crate::models::{Supplier, SupplierProduct, SupplierSource, UpdateSupplierDto};
use sqlx::SqlitePool;

pub async fn upsert_supplier(pool: &SqlitePool, s: &Supplier) -> anyhow::Result<i64> {
    let id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO suppliers (
            canonical_key, company_name, normalized_name, website, domain,
            country, city, email, phone, category, description, verification_status
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        ON CONFLICT(canonical_key) DO UPDATE SET
            company_name = COALESCE(excluded.company_name, suppliers.company_name),
            website = COALESCE(excluded.website, suppliers.website),
            domain = COALESCE(excluded.domain, suppliers.domain),
            country = COALESCE(excluded.country, suppliers.country),
            city = COALESCE(excluded.city, suppliers.city),
            email = COALESCE(excluded.email, suppliers.email),
            phone = COALESCE(excluded.phone, suppliers.phone),
            category = COALESCE(excluded.category, suppliers.category),
            description = COALESCE(excluded.description, suppliers.description),
            updated_at = CURRENT_TIMESTAMP
        RETURNING id
        "#,
    )
    .bind(&s.canonical_key)
    .bind(&s.company_name)
    .bind(&s.normalized_name)
    .bind(&s.website)
    .bind(&s.domain)
    .bind(&s.country)
    .bind(&s.city)
    .bind(&s.email)
    .bind(&s.phone)
    .bind(&s.category)
    .bind(&s.description)
    .bind(&s.verification_status)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

pub async fn get_supplier_by_id(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<Supplier>> {
    let supplier = sqlx::query_as::<_, Supplier>(
        r#"
        SELECT id, canonical_key, company_name, normalized_name, website, domain,
               country, city, email, phone, category, description, verification_status,
               created_at, updated_at
        FROM suppliers WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(supplier)
}

pub async fn get_supplier_by_canonical_key(
    pool: &SqlitePool,
    key: &str,
) -> anyhow::Result<Option<Supplier>> {
    let supplier = sqlx::query_as::<_, Supplier>(
        r#"
        SELECT id, canonical_key, company_name, normalized_name, website, domain,
               country, city, email, phone, category, description, verification_status,
               created_at, updated_at
        FROM suppliers WHERE canonical_key = $1
        "#,
    )
    .bind(key)
    .fetch_optional(pool)
    .await?;

    Ok(supplier)
}

pub async fn list_suppliers(
    pool: &SqlitePool,
    limit: i64,
    offset: i64,
    country_filter: Option<&str>,
    status_filter: Option<&str>,
) -> anyhow::Result<Vec<Supplier>> {
    let mut query = String::from(
        r#"
        SELECT id, canonical_key, company_name, normalized_name, website, domain,
               country, city, email, phone, category, description, verification_status,
               created_at, updated_at
        FROM suppliers
        WHERE 1=1
        "#,
    );

    if country_filter.is_some() {
        query.push_str(" AND country = $3");
    }
    if status_filter.is_some() {
        query.push_str(" AND verification_status = $4");
    }

    query.push_str(" ORDER BY id DESC LIMIT $1 OFFSET $2");

    let mut q = sqlx::query_as::<_, Supplier>(&query)
        .bind(limit)
        .bind(offset);

    if let Some(cf) = country_filter {
        q = q.bind(cf);
    }
    if let Some(sf) = status_filter {
        q = q.bind(sf);
    }

    let list = q.fetch_all(pool).await?;
    Ok(list)
}

pub async fn update_supplier(
    pool: &SqlitePool,
    id: i64,
    dto: UpdateSupplierDto,
) -> anyhow::Result<Option<Supplier>> {
    let existing = match get_supplier_by_id(pool, id).await? {
        Some(s) => s,
        None => return Ok(None),
    };

    let company_name = dto.company_name.unwrap_or(existing.company_name);
    let website = dto.website.or(existing.website);
    let country = dto.country.or(existing.country);
    let city = dto.city.or(existing.city);
    let email = dto.email.or(existing.email);
    let phone = dto.phone.or(existing.phone);
    let category = dto.category.or(existing.category);
    let description = dto.description.or(existing.description);
    let verification_status = dto
        .verification_status
        .unwrap_or(existing.verification_status);

    let updated = sqlx::query_as::<_, Supplier>(
        r#"
        UPDATE suppliers
        SET company_name = $1, website = $2, country = $3, city = $4, email = $5,
            phone = $6, category = $7, description = $8, verification_status = $9,
            updated_at = CURRENT_TIMESTAMP
        WHERE id = $10
        RETURNING id, canonical_key, company_name, normalized_name, website, domain,
                  country, city, email, phone, category, description, verification_status,
                  created_at, updated_at
        "#,
    )
    .bind(company_name)
    .bind(website)
    .bind(country)
    .bind(city)
    .bind(email)
    .bind(phone)
    .bind(category)
    .bind(description)
    .bind(verification_status)
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(updated)
}

pub async fn insert_source(
    pool: &SqlitePool,
    supplier_id: i64,
    crawl_page_id: Option<i64>,
    source_url: &str,
    source_type: Option<&str>,
    evidence_json: Option<&str>,
) -> anyhow::Result<i64> {
    let id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO supplier_sources (
            supplier_id, crawl_page_id, source_url, source_type, evidence_json
        ) VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT(supplier_id, source_url) DO UPDATE SET
            evidence_json = COALESCE(excluded.evidence_json, supplier_sources.evidence_json)
        RETURNING id
        "#,
    )
    .bind(supplier_id)
    .bind(crawl_page_id)
    .bind(source_url)
    .bind(source_type)
    .bind(evidence_json)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

pub async fn get_sources(
    pool: &SqlitePool,
    supplier_id: i64,
) -> anyhow::Result<Vec<SupplierSource>> {
    let list = sqlx::query_as::<_, SupplierSource>(
        r#"
        SELECT id, supplier_id, crawl_page_id, source_url, source_type, evidence_json, extracted_at
        FROM supplier_sources
        WHERE supplier_id = $1
        ORDER BY id ASC
        "#,
    )
    .bind(supplier_id)
    .fetch_all(pool)
    .await?;

    Ok(list)
}

pub async fn insert_product(pool: &SqlitePool, p: &SupplierProduct) -> anyhow::Result<i64> {
    let id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO supplier_products (
            supplier_id, source_id, product_name, brand, model_or_specification,
            price_amount, price_currency, minimum_order_quantity, availability_note,
            product_url, price_observed_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING id
        "#,
    )
    .bind(p.supplier_id)
    .bind(p.source_id)
    .bind(&p.product_name)
    .bind(&p.brand)
    .bind(&p.model_or_specification)
    .bind(p.price_amount)
    .bind(&p.price_currency)
    .bind(&p.minimum_order_quantity)
    .bind(&p.availability_note)
    .bind(&p.product_url)
    .bind(&p.price_observed_at)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

pub async fn get_products(
    pool: &SqlitePool,
    supplier_id: i64,
) -> anyhow::Result<Vec<SupplierProduct>> {
    let list = sqlx::query_as::<_, SupplierProduct>(
        r#"
        SELECT id, supplier_id, source_id, product_name, brand, model_or_specification,
               price_amount, price_currency, minimum_order_quantity, availability_note,
               product_url, price_observed_at
        FROM supplier_products
        WHERE supplier_id = $1
        ORDER BY id ASC
        "#,
    )
    .bind(supplier_id)
    .fetch_all(pool)
    .await?;

    Ok(list)
}
