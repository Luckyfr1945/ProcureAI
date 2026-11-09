use sqlx::Row;
use sqlx::SqlitePool;
use crate::models::{
    CrawlPage, EnrichedSearchResultDto, ScoreBreakdown, SearchJob, SearchQuery, Supplier,
};

pub async fn create_job(
    pool: &SqlitePool,
    user_id: i64,
    request: &str,
    options_json: Option<&str>,
) -> anyhow::Result<i64> {
    let id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO search_jobs (user_id, original_request, options_json, status)
        VALUES ($1, $2, $3, 'queued')
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(request)
    .bind(options_json)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

pub async fn get_job(pool: &SqlitePool, job_id: i64) -> anyhow::Result<Option<SearchJob>> {
    let job = sqlx::query_as::<_, SearchJob>(
        r#"
        SELECT id, user_id, original_request, status, options_json, stop_requested,
               round_count, discovered_url_count, processed_url_count, error_message,
               created_at, started_at, finished_at
        FROM search_jobs
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .fetch_optional(pool)
    .await?;

    Ok(job)
}

pub async fn update_job_status(
    pool: &SqlitePool,
    job_id: i64,
    status: &str,
    error_message: Option<&str>,
) -> anyhow::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let is_terminal = matches!(status, "completed" | "stopped" | "failed");
    let is_starting = status == "running";

    if is_terminal {
        sqlx::query(
            r#"
            UPDATE search_jobs
            SET status = $1, error_message = $2, finished_at = $3
            WHERE id = $4
            "#,
        )
        .bind(status)
        .bind(error_message)
        .bind(now)
        .bind(job_id)
        .execute(pool)
        .await?;
    } else if is_starting {
        sqlx::query(
            r#"
            UPDATE search_jobs
            SET status = $1, started_at = COALESCE(started_at, $2)
            WHERE id = $3
            "#,
        )
        .bind(status)
        .bind(now)
        .bind(job_id)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            r#"
            UPDATE search_jobs
            SET status = $1, error_message = $2
            WHERE id = $3
            "#,
        )
        .bind(status)
        .bind(error_message)
        .bind(job_id)
        .execute(pool)
        .await?;
    }

    Ok(())
}

pub async fn request_job_stop(pool: &SqlitePool, job_id: i64) -> anyhow::Result<bool> {
    let result = sqlx::query(
        r#"
        UPDATE search_jobs
        SET stop_requested = 1, status = CASE WHEN status = 'running' THEN 'stopping' ELSE status END
        WHERE id = $1 AND status IN ('queued', 'running')
        "#,
    )
    .bind(job_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn is_stop_requested(pool: &SqlitePool, job_id: i64) -> anyhow::Result<bool> {
    let val: Option<i64> = sqlx::query_scalar(
        "SELECT stop_requested FROM search_jobs WHERE id = $1",
    )
    .bind(job_id)
    .fetch_optional(pool)
    .await?;

    Ok(val.map(|v| v == 1).unwrap_or(true))
}

pub async fn update_job_progress(
    pool: &SqlitePool,
    job_id: i64,
    round_count: i64,
    discovered_urls: i64,
    processed_urls: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE search_jobs
        SET round_count = $1, discovered_url_count = $2, processed_url_count = $3
        WHERE id = $4
        "#,
    )
    .bind(round_count)
    .bind(discovered_urls)
    .bind(processed_urls)
    .bind(job_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn save_query(
    pool: &SqlitePool,
    job_id: i64,
    query_text: &str,
    provider: &str,
    page_number: i64,
    result_count: i64,
    exhausted: bool,
) -> anyhow::Result<i64> {
    let exhausted_val: i64 = if exhausted { 1 } else { 0 };
    let id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO search_queries (job_id, query_text, provider, page_number, result_count, exhausted)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT(job_id, provider, query_text, page_number) DO UPDATE SET
            result_count = excluded.result_count,
            exhausted = excluded.exhausted
        RETURNING id
        "#,
    )
    .bind(job_id)
    .bind(query_text)
    .bind(provider)
    .bind(page_number)
    .bind(result_count)
    .bind(exhausted_val)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

pub async fn get_queries(pool: &SqlitePool, job_id: i64) -> anyhow::Result<Vec<SearchQuery>> {
    let list = sqlx::query_as::<_, SearchQuery>(
        r#"
        SELECT id, job_id, query_text, provider, page_number, result_count, exhausted, created_at
        FROM search_queries
        WHERE job_id = $1
        ORDER BY id ASC
        "#,
    )
    .bind(job_id)
    .fetch_all(pool)
    .await?;

    Ok(list)
}

pub async fn insert_crawl_page(
    pool: &SqlitePool,
    job_id: i64,
    url: &str,
    normalized_url: &str,
    domain: &str,
) -> anyhow::Result<Option<i64>> {
    let opt: Option<i64> = sqlx::query_scalar(
        r#"
        INSERT INTO crawl_pages (job_id, url, normalized_url, domain, status)
        VALUES ($1, $2, $3, $4, 'discovered')
        ON CONFLICT(job_id, normalized_url) DO NOTHING
        RETURNING id
        "#,
    )
    .bind(job_id)
    .bind(url)
    .bind(normalized_url)
    .bind(domain)
    .fetch_optional(pool)
    .await?;

    Ok(opt)
}

pub async fn get_crawl_pages(pool: &SqlitePool, job_id: i64) -> anyhow::Result<Vec<CrawlPage>> {
    let list = sqlx::query_as::<_, CrawlPage>(
        r#"
        SELECT id, job_id, url, normalized_url, domain, status, http_status,
               page_title, content_hash, error_message, discovered_at, fetched_at
        FROM crawl_pages
        WHERE job_id = $1
        ORDER BY id ASC
        "#,
    )
    .bind(job_id)
    .fetch_all(pool)
    .await?;

    Ok(list)
}

pub async fn update_crawl_page_status(
    pool: &SqlitePool,
    page_id: i64,
    status: &str,
    http_status: Option<i64>,
    page_title: Option<&str>,
    content_hash: Option<&str>,
    error_message: Option<&str>,
) -> anyhow::Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        r#"
        UPDATE crawl_pages
        SET status = $1, http_status = $2, page_title = $3,
            content_hash = $4, error_message = $5, fetched_at = $6
        WHERE id = $7
        "#,
    )
    .bind(status)
    .bind(http_status)
    .bind(page_title)
    .bind(content_hash)
    .bind(error_message)
    .bind(now)
    .bind(page_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn save_search_result(
    pool: &SqlitePool,
    job_id: i64,
    supplier_id: i64,
    relevance_score: f64,
    score_breakdown_json: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO search_results (job_id, supplier_id, relevance_score, score_breakdown_json)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT(job_id, supplier_id) DO UPDATE SET
            relevance_score = excluded.relevance_score,
            score_breakdown_json = excluded.score_breakdown_json
        "#,
    )
    .bind(job_id)
    .bind(supplier_id)
    .bind(relevance_score)
    .bind(score_breakdown_json)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_job_results(
    pool: &SqlitePool,
    job_id: i64,
) -> anyhow::Result<Vec<EnrichedSearchResultDto>> {
    let rows = sqlx::query(
        r#"
        SELECT
            sr.id AS result_id,
            sr.job_id,
            sr.relevance_score,
            sr.score_breakdown_json,
            sr.first_seen_at,
            s.id AS supplier_id,
            s.canonical_key,
            s.company_name,
            s.normalized_name,
            s.website,
            s.domain,
            s.country,
            s.city,
            s.email,
            s.phone,
            s.category,
            s.description,
            s.verification_status,
            s.created_at AS supplier_created_at,
            s.updated_at AS supplier_updated_at
        FROM search_results sr
        JOIN suppliers s ON sr.supplier_id = s.id
        WHERE sr.job_id = $1
        ORDER BY sr.relevance_score DESC
        "#,
    )
    .bind(job_id)
    .fetch_all(pool)
    .await?;

    let mut results = Vec::new();
    for r in rows {
        let score_json: Option<String> = r.try_get("score_breakdown_json").ok().flatten();
        let breakdown: Option<ScoreBreakdown> = score_json
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok());

        let supplier = Supplier {
            id: r.try_get("supplier_id")?,
            canonical_key: r.try_get("canonical_key")?,
            company_name: r.try_get("company_name")?,
            normalized_name: r.try_get("normalized_name")?,
            website: r.try_get("website")?,
            domain: r.try_get("domain")?,
            country: r.try_get("country")?,
            city: r.try_get("city")?,
            email: r.try_get("email")?,
            phone: r.try_get("phone")?,
            category: r.try_get("category")?,
            description: r.try_get("description")?,
            verification_status: r.try_get("verification_status")?,
            created_at: r.try_get("supplier_created_at")?,
            updated_at: r.try_get("supplier_updated_at")?,
        };

        results.push(EnrichedSearchResultDto {
            result_id: r.try_get("result_id")?,
            job_id: r.try_get("job_id")?,
            relevance_score: r.try_get("relevance_score")?,
            score_breakdown: breakdown,
            supplier,
            first_seen_at: r.try_get("first_seen_at")?,
        });
    }

    Ok(results)
}

pub async fn count_job_suppliers(pool: &SqlitePool, job_id: i64) -> anyhow::Result<i64> {
    let cnt: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM search_results WHERE job_id = $1",
    )
    .bind(job_id)
    .fetch_one(pool)
    .await?;

    Ok(cnt)
}

pub async fn list_recent_jobs(pool: &SqlitePool, limit: i64) -> anyhow::Result<Vec<SearchJob>> {
    let list = sqlx::query_as::<_, SearchJob>(
        r#"
        SELECT id, user_id, original_request, status, options_json, stop_requested,
               round_count, discovered_url_count, processed_url_count, error_message,
               created_at, started_at, finished_at
        FROM search_jobs
        ORDER BY id DESC
        LIMIT $1
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(list)
}
