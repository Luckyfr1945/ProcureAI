use std::sync::Arc;
use std::time::Duration;
use sqlx::SqlitePool;
use tokio::time::sleep;
use crate::config::Config;
use crate::database::repositories::{searches, suppliers};
use crate::models::{SearchJob, SearchPreferences};
use crate::providers::gemini::GeminiClient;
use crate::providers::search::{create_search_provider, SearchProvider};
use crate::services::{
    AsyncCrawler, ExtractionService, QueryExpansionService, RankingService,
};
use crate::utils::url_safety::{extract_domain, normalize_url, validate_url_safety};

pub struct DiscoveryWorker {
    config: Arc<Config>,
    db: SqlitePool,
    search_provider: Arc<dyn SearchProvider>,
    crawler: AsyncCrawler,
    extraction: ExtractionService,
    query_expansion: QueryExpansionService,
}

impl DiscoveryWorker {
    pub fn new(config: Arc<Config>, db: SqlitePool) -> Self {
        let gemini = GeminiClient::new(
            config.gemini_api_key.clone(),
            config.gemini_model.clone(),
        );
        let search_provider = create_search_provider(&config);
        let crawler = AsyncCrawler::new(
            config.crawler_timeout_seconds,
            config.crawler_max_response_bytes,
        );
        let extraction = ExtractionService::new(gemini.clone());
        let query_expansion = QueryExpansionService::new(gemini.clone());

        Self {
            config,
            db,
            search_provider,
            crawler,
            extraction,
            query_expansion,
        }
    }

    pub async fn run_loop(self: Arc<Self>) {
        tracing::info!("Discovery background worker started");

        loop {
            match self.find_next_queued_job().await {
                Ok(Some(job)) => {
                    tracing::info!("Worker picked up search job #{}", job.id);
                    let worker = Arc::clone(&self);
                    // Process job asynchronously
                    tokio::spawn(async move {
                        if let Err(e) = worker.process_job(job).await {
                            tracing::error!("Error processing search job: {}", e);
                        }
                    });
                }
                Ok(None) => {
                    // No queued jobs, sleep briefly
                    sleep(Duration::from_millis(1500)).await;
                }
                Err(e) => {
                    tracing::error!("Worker error querying queued jobs: {}", e);
                    sleep(Duration::from_secs(3)).await;
                }
            }
        }
    }

    async fn find_next_queued_job(&self) -> anyhow::Result<Option<SearchJob>> {
        let job = sqlx::query_as::<_, SearchJob>(
            r#"
            SELECT id, user_id, original_request, status, options_json, stop_requested,
                   round_count, discovered_url_count, processed_url_count, error_message,
                   created_at, started_at, finished_at
            FROM search_jobs
            WHERE status = 'queued'
            ORDER BY id ASC
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.db)
        .await?;

        Ok(job)
    }

    pub async fn process_job(&self, job: SearchJob) -> anyhow::Result<()> {
        let job_id = job.id;
        searches::update_job_status(&self.db, job_id, "running", None).await?;

        let preferences: SearchPreferences = job
            .options_json
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok())
            .unwrap_or_default();

        // 1. Expand query via Gemini
        let expansion = match self
            .query_expansion
            .expand(&job.original_request, 5)
            .await
        {
            Ok(exp) => exp,
            Err(e) => {
                let err_msg = format!("Failed query expansion: {}", e);
                searches::update_job_status(&self.db, job_id, "failed", Some(&err_msg)).await?;
                return Err(anyhow::anyhow!(err_msg));
            }
        };

        tracing::info!(
            "Job #{}: Expanded into {} queries",
            job_id,
            expansion.search_queries.len()
        );

        let mut round_count = 0;
        let mut total_discovered_urls: i64 = 0;
        let mut total_processed_urls: i64 = 0;

        let max_rounds = self.config.max_search_rounds.min(10) as usize;
        let max_pages_per_query = self.config.max_pages_per_query.min(3);

        for query_str in &expansion.search_queries {
            round_count += 1;
            if round_count > max_rounds {
                break;
            }

            // Check if user requested stop
            if searches::is_stop_requested(&self.db, job_id).await.unwrap_or(false) {
                tracing::info!("Job #{}: Stop requested by user", job_id);
                searches::update_job_status(&self.db, job_id, "stopped", None).await?;
                return Ok(());
            }

            for page_num in 1..=max_pages_per_query {
                if searches::is_stop_requested(&self.db, job_id).await.unwrap_or(false) {
                    searches::update_job_status(&self.db, job_id, "stopped", None).await?;
                    return Ok(());
                }

                // Call SearchProvider
                let search_page = match self.search_provider.search(query_str, page_num).await {
                    Ok(sp) => sp,
                    Err(e) => {
                        tracing::warn!("Job #{}: Search provider error for '{}' p.{}: {}", job_id, query_str, page_num, e);
                        break;
                    }
                };

                let result_count = search_page.results.len() as i64;
                searches::save_query(
                    &self.db,
                    job_id,
                    query_str,
                    self.search_provider.provider_name(),
                    page_num as i64,
                    result_count,
                    !search_page.has_next_page,
                )
                .await?;

                // Process discovered URLs
                for item in search_page.results {
                    // Normalize URL
                    let norm_url = match normalize_url(&item.url) {
                        Ok(u) => u,
                        Err(_) => continue,
                    };

                    let domain = match extract_domain(&norm_url) {
                        Some(d) => d,
                        None => continue,
                    };

                    // Check SSRF before adding to queue
                    if validate_url_safety(&norm_url).await.is_err() {
                        continue;
                    }

                    // Insert into crawl_pages
                    if let Ok(Some(page_id)) = searches::insert_crawl_page(
                        &self.db,
                        job_id,
                        &item.url,
                        &norm_url,
                        &domain,
                    )
                    .await
                    {
                        total_discovered_urls += 1;

                        // Polite request delay
                        if self.config.crawler_request_delay_ms > 0 {
                            sleep(Duration::from_millis(self.config.crawler_request_delay_ms.min(100))).await;
                        }

                        // Crawl page
                        match self.crawler.fetch_page(&norm_url).await {
                            Ok(crawl_result) => {
                                total_processed_urls += 1;
                                searches::update_crawl_page_status(
                                    &self.db,
                                    page_id,
                                    "fetched",
                                    Some(crawl_result.http_status as i64),
                                    Some(&crawl_result.page_title),
                                    Some(&crawl_result.content_hash),
                                    None,
                                )
                                .await?;

                                // Extract supplier candidate
                                if let Ok((supplier, products)) =
                                    self.extraction.extract_candidate(&crawl_result).await
                                {
                                    // Upsert supplier
                                    if let Ok(supplier_id) =
                                        suppliers::upsert_supplier(&self.db, &supplier).await
                                    {
                                        // Save source evidence
                                        let evidence_json = serde_json::json!({
                                            "page_title": crawl_result.page_title,
                                            "emails": crawl_result.emails,
                                            "phones": crawl_result.phones
                                        })
                                        .to_string();

                                        let _ = suppliers::insert_source(
                                            &self.db,
                                            supplier_id,
                                            Some(page_id),
                                            &crawl_result.final_url,
                                            Some("website"),
                                            Some(&evidence_json),
                                        )
                                        .await;

                                        // Save products
                                        for mut prod in products.clone() {
                                            prod.supplier_id = supplier_id;
                                            let _ = suppliers::insert_product(&self.db, &prod).await;
                                        }

                                        // Calculate score
                                        let score_breakdown = RankingService::calculate_score(
                                            &supplier,
                                            &products,
                                            &job.original_request,
                                            &expansion.important_specs,
                                            &preferences,
                                        );

                                        let breakdown_json =
                                            serde_json::to_string(&score_breakdown).ok();

                                        let _ = searches::save_search_result(
                                            &self.db,
                                            job_id,
                                            supplier_id,
                                            score_breakdown.total_score,
                                            breakdown_json.as_deref(),
                                        )
                                        .await;
                                    }
                                }
                            }
                            Err(e) => {
                                total_processed_urls += 1;
                                let err_text = e.to_string();
                                let _ = searches::update_crawl_page_status(
                                    &self.db,
                                    page_id,
                                    "failed",
                                    None,
                                    None,
                                    None,
                                    Some(&err_text),
                                )
                                .await;
                            }
                        }
                    }
                }

                // Update job stats
                searches::update_job_progress(
                    &self.db,
                    job_id,
                    round_count as i64,
                    total_discovered_urls,
                    total_processed_urls,
                )
                .await?;

                if !search_page.has_next_page {
                    break;
                }
            }
        }

        searches::update_job_status(&self.db, job_id, "completed", None).await?;
        tracing::info!(
            "Job #{}: Discovery completed! Discovered: {}, Processed: {}",
            job_id,
            total_discovered_urls,
            total_processed_urls
        );

        Ok(())
    }
}

