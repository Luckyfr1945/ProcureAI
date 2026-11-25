use crate::domain::{
    JobSupplierResult, SearchJob, SearchJobEvent, SearchQuery, Supplier, SupplierOffer,
};
use sqlx::SqlitePool;
use uuid::Uuid;

pub async fn create_job(
    pool: &SqlitePool,
    user_id: i64,
    input_text: &str,
) -> anyhow::Result<String> {
    let job_id = Uuid::new_v4().to_string();
    sqlx::query(
        r#"
        INSERT INTO search_jobs (id, user_id, input_text, status, current_stage, created_at)
        VALUES (?, ?, ?, 'queued', 'understand', datetime('now'))
        "#,
    )
    .bind(&job_id)
    .bind(user_id)
    .bind(input_text)
    .execute(pool)
    .await?;

    record_event(
        pool,
        &job_id,
        "understand",
        Some("Job created and queued for requirement analysis"),
        None,
    )
    .await?;

    Ok(job_id)
}

pub async fn get_job(pool: &SqlitePool, job_id: &str) -> anyhow::Result<Option<SearchJob>> {
    let job = sqlx::query_as::<_, SearchJob>(
        r#"
        SELECT id, user_id, input_text, requirement_json, status, current_stage,
               round_count, new_supplier_count, error_message, started_at, finished_at, created_at
        FROM search_jobs
        WHERE id = ?
        "#,
    )
    .bind(job_id)
    .fetch_optional(pool)
    .await?;

    Ok(job)
}

pub async fn list_user_jobs(pool: &SqlitePool, user_id: i64) -> anyhow::Result<Vec<SearchJob>> {
    let jobs = sqlx::query_as::<_, SearchJob>(
        r#"
        SELECT id, user_id, input_text, requirement_json, status, current_stage,
               round_count, new_supplier_count, error_message, started_at, finished_at, created_at
        FROM search_jobs
        WHERE user_id = ?
        ORDER BY created_at DESC
        LIMIT 50
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(jobs)
}

pub async fn update_job_status(
    pool: &SqlitePool,
    job_id: &str,
    status: &str,
    error: Option<&str>,
) -> anyhow::Result<()> {
    if status == "running" {
        sqlx::query(
            r#"
            UPDATE search_jobs
            SET status = ?, started_at = coalesce(started_at, datetime('now'))
            WHERE id = ?
            "#,
        )
        .bind(status)
        .bind(job_id)
        .execute(pool)
        .await?;
    } else if status == "completed"
        || status == "stopped"
        || status == "failed"
        || status == "limit_reached"
    {
        sqlx::query(
            r#"
            UPDATE search_jobs
            SET status = ?, error_message = ?, finished_at = datetime('now')
            WHERE id = ?
            "#,
        )
        .bind(status)
        .bind(error)
        .bind(job_id)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            r#"
            UPDATE search_jobs
            SET status = ?, error_message = ?
            WHERE id = ?
            "#,
        )
        .bind(status)
        .bind(error)
        .bind(job_id)
        .execute(pool)
        .await?;
    }

    Ok(())
}

pub async fn update_job_stage(
    pool: &SqlitePool,
    job_id: &str,
    stage: &str,
    message: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE search_jobs
        SET current_stage = ?
        WHERE id = ?
        "#,
    )
    .bind(stage)
    .bind(job_id)
    .execute(pool)
    .await?;

    record_event(pool, job_id, stage, message, None).await?;
    Ok(())
}

pub async fn set_job_requirement(
    pool: &SqlitePool,
    job_id: &str,
    req_json: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE search_jobs
        SET requirement_json = ?
        WHERE id = ?
        "#,
    )
    .bind(req_json)
    .bind(job_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn increment_job_counts(
    pool: &SqlitePool,
    job_id: &str,
    rounds: i64,
    new_suppliers: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        UPDATE search_jobs
        SET round_count = round_count + ?, new_supplier_count = new_supplier_count + ?
        WHERE id = ?
        "#,
    )
    .bind(rounds)
    .bind(new_suppliers)
    .bind(job_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn record_event(
    pool: &SqlitePool,
    job_id: &str,
    stage: &str,
    message: Option<&str>,
    metadata_json: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO search_job_events (job_id, stage, message, metadata_json, created_at)
        VALUES (?, ?, ?, ?, datetime('now'))
        "#,
    )
    .bind(job_id)
    .bind(stage)
    .bind(message)
    .bind(metadata_json)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_job_events(
    pool: &SqlitePool,
    job_id: &str,
) -> anyhow::Result<Vec<SearchJobEvent>> {
    let events = sqlx::query_as::<_, SearchJobEvent>(
        r#"
        SELECT id, job_id, stage, message, metadata_json, created_at
        FROM search_job_events
        WHERE job_id = ?
        ORDER BY id ASC
        "#,
    )
    .bind(job_id)
    .fetch_all(pool)
    .await?;

    Ok(events)
}

pub async fn save_query(
    pool: &SqlitePool,
    job_id: &str,
    query_text: &str,
    query_type: &str,
    language: &str,
    provider: &str,
    page_or_offset: i64,
    result_count: i64,
) -> anyhow::Result<i64> {
    let id = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO search_queries (job_id, query_text, query_type, language, provider, page_or_offset, result_count, created_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'))
        RETURNING id
        "#,
    )
    .bind(job_id)
    .bind(query_text)
    .bind(query_type)
    .bind(language)
    .bind(provider)
    .bind(page_or_offset)
    .bind(result_count)
    .fetch_one(pool)
    .await?;

    Ok(id)
}

pub async fn get_queries(pool: &SqlitePool, job_id: &str) -> anyhow::Result<Vec<SearchQuery>> {
    let queries = sqlx::query_as::<_, SearchQuery>(
        r#"
        SELECT id, job_id, query_text, query_type, language, provider, page_or_offset, result_count, created_at
        FROM search_queries
        WHERE job_id = ?
        ORDER BY id ASC
        "#,
    )
    .bind(job_id)
    .fetch_all(pool)
    .await?;

    Ok(queries)
}

pub async fn save_job_result(
    pool: &SqlitePool,
    job_id: &str,
    supplier_id: i64,
    product_fit_score: f64,
    evidence_completeness_score: f64,
    rank_position: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO job_supplier_results (job_id, supplier_id, product_fit_score, evidence_completeness_score, rank_position, created_at)
        VALUES (?, ?, ?, ?, ?, datetime('now'))
        ON CONFLICT(job_id, supplier_id) DO UPDATE SET
            product_fit_score = excluded.product_fit_score,
            evidence_completeness_score = excluded.evidence_completeness_score,
            rank_position = excluded.rank_position
        "#,
    )
    .bind(job_id)
    .bind(supplier_id)
    .bind(product_fit_score)
    .bind(evidence_completeness_score)
    .bind(rank_position)
    .execute(pool)
    .await?;

    Ok(())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DetailedJobResult {
    pub result_id: i64,
    pub job_id: String,
    pub supplier: Supplier,
    pub product_fit_score: f64,
    pub evidence_completeness_score: f64,
    pub rank_position: i64,
    pub offers: Vec<SupplierOffer>,
}

pub async fn get_job_results_detailed(
    pool: &SqlitePool,
    job_id: &str,
) -> anyhow::Result<Vec<DetailedJobResult>> {
    let rows = sqlx::query_as::<_, JobSupplierResult>(
        r#"
        SELECT id, job_id, supplier_id, product_fit_score, evidence_completeness_score, rank_position, created_at
        FROM job_supplier_results
        WHERE job_id = ?
        ORDER BY product_fit_score DESC, evidence_completeness_score DESC
        "#,
    )
    .bind(job_id)
    .fetch_all(pool)
    .await?;

    let requirement = get_job(pool, job_id)
        .await?
        .and_then(|j| j.requirement_json)
        .and_then(|raw| serde_json::from_str::<crate::domain::ProductRequirement>(&raw).ok())
        .filter(|req| req.category != "__PENDING__");
    let mut detailed = Vec::new();
    for r in rows {
        let sup_opt =
            crate::database::repositories::suppliers::get_supplier_by_id(pool, r.supplier_id)
                .await?;
        if let Some(sup) = sup_opt {
            if let Some(ref req) = requirement {
                if !crate::services::location::supplier_matches_targets(
                    sup.country_code.as_deref(),
                    sup.country_name.as_deref(),
                    sup.address.as_deref(),
                    &req.target_regions,
                ) || !crate::services::location::consistent_supplier_location(
                    sup.country_code.as_deref(),
                    sup.country_name.as_deref(),
                    sup.address.as_deref(),
                    sup.public_phone.as_deref(),
                ) {
                    continue;
                }
            }
            let mut offers =
                crate::database::repositories::products::get_supplier_offers(pool, sup.id).await?;
            if let Some(ref req) = requirement {
                let mut relevant = Vec::new();
                for offer in offers {
                    if let Some(product) =
                        crate::database::repositories::products::get_product_by_id(
                            pool,
                            offer.product_id,
                        )
                        .await?
                    {
                        if crate::services::stored_search::matches_product(
                            req,
                            &product,
                            offer.description.as_deref(),
                        ) {
                            relevant.push(offer);
                        }
                    }
                }
                offers = relevant;
            }
            if requirement.is_some() && offers.is_empty() {
                continue;
            }
            detailed.push(DetailedJobResult {
                result_id: r.id,
                job_id: r.job_id,
                supplier: sup,
                product_fit_score: r.product_fit_score,
                evidence_completeness_score: r.evidence_completeness_score,
                rank_position: detailed.len() as i64 + 1,
                offers,
            });
        }
    }

    Ok(detailed)
}

pub async fn claim_next_queued_job(pool: &SqlitePool) -> anyhow::Result<Option<SearchJob>> {
    let job = sqlx::query_as::<_, SearchJob>(
        r#"
        UPDATE search_jobs
        SET status = 'running', started_at = datetime('now')
        WHERE id = (
            SELECT id FROM search_jobs
            WHERE status = 'queued'
            ORDER BY created_at ASC
            LIMIT 1
        )
        RETURNING id, user_id, input_text, requirement_json, status, current_stage,
                  round_count, new_supplier_count, error_message, started_at, finished_at, created_at
        "#,
    )
    .fetch_optional(pool)
    .await?;

    Ok(job)
}

pub async fn fail_stale_running_jobs(pool: &SqlitePool) -> anyhow::Result<u64> {
    sqlx::query("UPDATE search_jobs SET status='stopped', finished_at=CURRENT_TIMESTAMP WHERE status='stopping'").execute(pool).await?;
    let res = sqlx::query(
        r#"
        UPDATE search_jobs
        SET status = 'failed',
            error_message = COALESCE(error_message, 'Worker restarted before job completion'),
            finished_at = datetime('now')
        WHERE status = 'running'
        "#,
    )
    .execute(pool)
    .await?;

    Ok(res.rows_affected())
}
