use crate::config::Config;
use crate::database::repositories::{evidence, products, search_jobs, suppliers};
use crate::domain::{
    CreateSearchJobRequest, EvidenceItem, Product, ProductRequirement, SearchJob, SupplierOffer,
    SupplierSource,
};
use crate::providers::search::{SearchItem, SearchProvider, create_search_provider};
use crate::services::gemini::{ExtractedEvidencePayload, ExtractedSupplierPayload, GeminiClient};
use crate::services::{AsyncCrawler, EntityResolution, QueryPlanner, RankingService, location};
use crate::utils::text::calculate_content_hash;
use crate::utils::url_safety::{extract_domain, normalize_url, validate_url_safety};
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::sleep;

#[derive(Debug, Clone)]
struct CandidateUrl {
    original_url: String,
    normalized_url: String,
    title: String,
    snippet: String,
    triage_score: i32,
}

struct FetchedExtractedPage {
    canonical_url: String,
    raw_url: String,
    page_title: String,
    http_status: i64,
    content_hash: String,
    source_type: &'static str,
    strict_region_match: bool,
    payload: ExtractedSupplierPayload,
}

pub struct DiscoveryWorker {
    config: Arc<Config>,
    db: SqlitePool,
    search_provider: Arc<dyn SearchProvider>,
    crawler: AsyncCrawler,
    gemini: GeminiClient,
    planner: QueryPlanner,
    fetch_gate: Arc<tokio::sync::Mutex<Instant>>,
}

impl DiscoveryWorker {
    pub fn new(config: Arc<Config>, db: SqlitePool) -> Self {
        let gemini = GeminiClient::new(config.gemini_api_key.clone(), config.gemini_model.clone());
        let search_provider = create_search_provider(&config);
        let crawl_timeout = config.request_timeout_seconds;
        let crawler = AsyncCrawler::new(crawl_timeout, config.max_page_bytes);
        let planner = QueryPlanner::new(gemini.clone());

        Self {
            config,
            db,
            search_provider,
            crawler,
            gemini,
            planner,
            fetch_gate: Arc::new(tokio::sync::Mutex::new(Instant::now())),
        }
    }

    pub async fn run_loop(self: Arc<Self>) {
        tracing::info!("ProcureAI v2 Discovery Worker started");
        if let Err(e) = search_jobs::fail_stale_running_jobs(&self.db).await {
            tracing::warn!("Failed to clean up stale running jobs on startup: {}", e);
        }

        loop {
            match search_jobs::claim_next_queued_job(&self.db).await {
                Ok(Some(job)) => {
                    let job_id = job.id.clone();
                    tracing::info!("Worker picked up search job {}", job_id);
                    let worker = Arc::clone(&self);
                    // One active job per worker; fetch concurrency remains bounded inside it.
                    let outcome = tokio::spawn(async move { worker.process_job(job).await }).await;
                    let failure = match outcome {
                        Ok(Ok(())) => None,
                        Ok(Err(e)) => Some(e.to_string()),
                        Err(e) => Some(format!("Worker task failed: {}", e)),
                    };
                    if let Some(message) = failure {
                        let _ = search_jobs::update_job_status(
                            &self.db,
                            &job_id,
                            "failed",
                            Some(&message),
                        )
                        .await;
                    }
                }
                Ok(None) => {
                    sleep(Duration::from_millis(1200)).await;
                }
                Err(e) => {
                    tracing::error!("Worker error querying queued jobs: {}", e);
                    sleep(Duration::from_secs(3)).await;
                }
            }
        }
    }

    async fn should_stop(&self, job_id: &str, started: Instant) -> bool {
        if started.elapsed().as_secs() >= self.config.max_job_duration_seconds {
            let _ = search_jobs::update_job_status(
                &self.db,
                job_id,
                "limit_reached",
                Some("Max job duration reached"),
            )
            .await;
            return true;
        }

        if let Ok(Some(job)) = search_jobs::get_job(&self.db, job_id).await {
            if job.status == "stopped" || job.status == "stopping" {
                let _ = search_jobs::update_job_status(&self.db, job_id, "stopped", None).await;
                return true;
            }
        }

        false
    }

    pub async fn process_job(&self, job: SearchJob) -> anyhow::Result<()> {
        let job_id = job.id.clone();
        let start_time = Instant::now();
        if self.should_stop(&job_id, start_time).await {
            return Ok(());
        }
        search_jobs::update_job_status(&self.db, &job_id, "running", None).await?;

        let pending_seed: Option<ProductRequirement> = job
            .requirement_json
            .as_deref()
            .and_then(|s| serde_json::from_str::<ProductRequirement>(s).ok());

        let (input_text, target_regions, preferred_langs, user_req_attrs, user_pr_number) =
            if let Some(ref seed) = pending_seed {
                if seed.category == "__PENDING__" {
                    (
                        job.input_text.clone(),
                        seed.target_regions.clone(),
                        seed.search_languages.clone(),
                        seed.required_attributes.clone(),
                        seed.pr_number.clone(),
                    )
                } else {
                    (
                        job.input_text.clone(),
                        seed.target_regions.clone(),
                        seed.search_languages.clone(),
                        Vec::new(),
                        seed.pr_number.clone(),
                    )
                }
            } else if let Ok(req_payload) =
                serde_json::from_str::<CreateSearchJobRequest>(&job.input_text)
            {
                (
                    req_payload.input_text,
                    req_payload.target_regions,
                    req_payload.preferred_languages,
                    req_payload.required_attributes,
                    req_payload.pr_number,
                )
            } else {
                (
                    job.input_text.clone(),
                    vec!["Indonesia".to_string()],
                    vec!["id".to_string(), "en".to_string()],
                    Vec::new(),
                    None,
                )
            };

        // ============================================================
        // STAGE 1: UNDERSTAND (Requirement Analyzer)
        // ============================================================
        search_jobs::update_job_stage(
            &self.db,
            &job_id,
            "understand",
            Some("Analyzing product specification and category attributes"),
        )
        .await?;

        let requirement: ProductRequirement =
            if let Some(seed) = pending_seed.filter(|s| s.category != "__PENDING__") {
                seed
            } else {
                let mut req = self
                    .gemini
                    .analyze_requirement(&input_text, &target_regions, &preferred_langs)
                    .await
                    .unwrap_or_else(|_| {
                        self.gemini.fallback_analyze_requirement(
                            &input_text,
                            &target_regions,
                            &preferred_langs,
                        )
                    });

                if !target_regions.is_empty() {
                    req.target_regions = target_regions.clone();
                }
                for attr in user_req_attrs {
                    if !req.required_attributes.contains(&attr) {
                        req.required_attributes.push(attr);
                    }
                }
                if req.pr_number.is_none() {
                    req.pr_number = user_pr_number;
                }

                let req_json = serde_json::to_string(&req)?;
                search_jobs::set_job_requirement(&self.db, &job_id, &req_json).await?;
                req
            };

        let stored_scores =
            crate::services::stored_search::reuse_suppliers(&self.db, &job_id, &requirement)
                .await?;
        if !stored_scores.is_empty() {
            search_jobs::record_event(
                &self.db,
                &job_id,
                "understand",
                Some(&format!(
                    "Ditemukan {} supplier tersimpan dengan produk dan lokasi sesuai",
                    stored_scores.len()
                )),
                None,
            )
            .await?;
            search_jobs::increment_job_counts(&self.db, &job_id, 0, stored_scores.len() as i64)
                .await?;
        }

        if self.should_stop(&job_id, start_time).await {
            return Ok(());
        }

        // ============================================================
        // STAGE 2: DISCOVER (Query Planning & Multi-Query Search)
        // ============================================================
        search_jobs::update_job_stage(
            &self.db,
            &job_id,
            "discover",
            Some(&format!(
                "Planning targeted queries for {} (category: {})",
                requirement.product_name, requirement.category
            )),
        )
        .await?;

        let planned_queries = self
            .planner
            .generate_search_plan(&requirement, 6)
            .await
            .unwrap_or_else(|_| self.gemini.fallback_plan_queries(&requirement, 6));

        let mut search_requests_used: u32 = 0;
        let mut raw_candidates: Vec<SearchItem> = Vec::new();
        let max_search_req = self.config.max_search_requests_per_job;
        let discover_budget = if max_search_req > 6 {
            (max_search_req * 3 / 4).max(1)
        } else {
            max_search_req
        };

        for pq in &planned_queries {
            if search_requests_used >= discover_budget || self.should_stop(&job_id, start_time).await
            {
                break;
            }

            let max_p = if raw_candidates.len() < 20 { 2 } else { 1 };
            for page in 1..=max_p {
                if search_requests_used >= discover_budget
                    || self.should_stop(&job_id, start_time).await
                {
                    break;
                }

                search_requests_used += 1;
                search_jobs::increment_job_counts(&self.db, &job_id, 1, 0).await?;

                match self.search_provider.search(&pq.query_text, page).await {
                    Ok(search_page) => {
                        let count = search_page.results.len() as i64;
                        let _ = search_jobs::save_query(
                            &self.db,
                            &job_id,
                            &pq.query_text,
                            &pq.query_type,
                            &pq.language,
                            self.search_provider.provider_name(),
                            page as i64,
                            count,
                        )
                        .await;

                        let has_more = search_page.has_next_page;
                        raw_candidates.extend(search_page.results);

                        if !has_more || count == 0 {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Job {}: Search provider error on '{}' p.{}: {}",
                            job_id,
                            pq.query_text,
                            page,
                            e
                        );
                        break;
                    }
                }
            }
        }

        // ============================================================
        // STAGE 3: TRIAGE (Filter & Prioritize Candidate URLs)
        // ============================================================
        search_jobs::update_job_stage(
            &self.db,
            &job_id,
            "triage",
            Some(&format!(
                "Filtering and prioritizing {} discovered search hits",
                raw_candidates.len()
            )),
        )
        .await?;

        let triaged_urls = self.triage_candidates(raw_candidates, &requirement).await;
        let mut crawled_urls: HashSet<String> = HashSet::new();
        let mut crawl_count: u32 = 0;
        // Safety ceiling on top of the configured budget (the .env allows up to 500).
        let max_crawl = self.config.max_crawl_pages_per_job.min(60);

        let mut discovered_supplier_ids: Vec<i64> = stored_scores.keys().copied().collect();
        let mut supplier_best_scores: HashMap<i64, (f64, f64)> = stored_scores;

        // ============================================================
        // STAGE 4 & 5: EXTRACT, RESOLVE & VALIDATE (Bounded Concurrency)
        // ============================================================
        search_jobs::update_job_stage(
            &self.db,
            &job_id,
            "extract",
            Some(&format!(
                "Crawling and extracting structured evidence across top {} triaged candidate pages",
                triaged_urls.len().min(max_crawl as usize)
            )),
        )
        .await?;

        let (first_pass, leftover_candidates) = self
            .process_candidate_batch(
                &job_id,
                triaged_urls,
                &mut crawled_urls,
                max_crawl.saturating_sub(max_crawl / 4) as usize,
                &requirement,
                &mut discovered_supplier_ids,
                &mut supplier_best_scores,
                start_time,
            )
            .await?;
        crawl_count += first_pass;

        self.recompute_job_rankings(&job_id, &supplier_best_scores)
            .await?;

        if self.should_stop(&job_id, start_time).await {
            return Ok(());
        }

        // ============================================================
        // STAGE 6: GAP-DRIVEN TARGETED DISCOVERY
        // Also runs when results are still thin, so unopened candidates
        // from the first pass are not thrown away.
        // ============================================================
        const FOLLOW_UP_BELOW_SUPPLIERS: usize = 25;
        let gaps = self.detect_high_value_gaps(&job_id, &requirement).await;
        let thin_results = discovered_supplier_ids.len() < FOLLOW_UP_BELOW_SUPPLIERS
            && !leftover_candidates.is_empty();
        if (!gaps.is_empty() || thin_results) && crawl_count < max_crawl {
            let stage_msg = if gaps.is_empty() {
                format!(
                    "Hasil baru {} supplier, melanjutkan ke {} kandidat berikutnya.",
                    discovered_supplier_ids.len(),
                    leftover_candidates.len()
                )
            } else {
                format!(
                    "Detected information gaps ({:?}). Launching targeted follow-up research.",
                    gaps
                )
            };
            search_jobs::update_job_stage(&self.db, &job_id, "gap_discovery", Some(&stage_msg))
                .await?;

            let mut gap_queries = self.planner.generate_gap_queries(&requirement, &gaps, 3);
            if discovered_supplier_ids.len() < 8 {
                let term = requirement
                    .part_number
                    .as_deref()
                    .filter(|s| !s.trim().is_empty())
                    .map(|pn| {
                        format!(
                            "{} {}",
                            requirement.brand.as_deref().unwrap_or(""),
                            pn
                        )
                        .trim()
                        .to_string()
                    })
                    .unwrap_or_else(|| requirement.product_name.clone());
                gap_queries.push(crate::services::gemini::PlannedQuery {
                    query_text: format!("{} supplier distributor", term),
                    query_type: "broadened".to_string(),
                    language: "id".to_string(),
                    rationale: "Broadened regional/global fallback due to scarce local results"
                        .to_string(),
                });
                gap_queries.push(crate::services::gemini::PlannedQuery {
                    query_text: format!("{} manufacturer wholesale", term),
                    query_type: "broadened".to_string(),
                    language: "en".to_string(),
                    rationale: "Broadened international manufacturer fallback".to_string(),
                });
            }
            let mut gap_raw_items = Vec::new();

            for gq in gap_queries {
                if search_requests_used >= max_search_req
                    || self.should_stop(&job_id, start_time).await
                {
                    break;
                }
                search_requests_used += 1;
                search_jobs::increment_job_counts(&self.db, &job_id, 1, 0).await?;

                if let Ok(sp) = self.search_provider.search(&gq.query_text, 1).await {
                    let count = sp.results.len() as i64;
                    let _ = search_jobs::save_query(
                        &self.db,
                        &job_id,
                        &gq.query_text,
                        &gq.query_type,
                        &gq.language,
                        self.search_provider.provider_name(),
                        1,
                        count,
                    )
                    .await;
                    gap_raw_items.extend(sp.results);
                }
            }

            let mut follow_up = self.triage_candidates(gap_raw_items, &requirement).await;
            follow_up.extend(leftover_candidates);
            follow_up.sort_by(|a, b| b.triage_score.cmp(&a.triage_score));
            let remaining_budget = (max_crawl - crawl_count) as usize;
            if remaining_budget > 0 && !follow_up.is_empty() {
                let (second_pass, _) = self
                    .process_candidate_batch(
                        &job_id,
                        follow_up,
                        &mut crawled_urls,
                        remaining_budget,
                        &requirement,
                        &mut discovered_supplier_ids,
                        &mut supplier_best_scores,
                        start_time,
                    )
                    .await?;
                crawl_count += second_pass;
            }

            self.recompute_job_rankings(&job_id, &supplier_best_scores)
                .await?;
        }

        if self.should_stop(&job_id, start_time).await {
            return Ok(());
        }
        // Complete Job
        search_jobs::update_job_stage(
            &self.db,
            &job_id,
            "completed",
            Some(&format!(
                "Deep research complete. Discovered {} supplier entities across {} pages.",
                discovered_supplier_ids.len(),
                crawl_count
            )),
        )
        .await?;
        search_jobs::update_job_status(&self.db, &job_id, "completed", None).await?;

        tracing::info!(
            "Job {}: Completed cleanly in {:?} with {} suppliers",
            job_id,
            start_time.elapsed(),
            discovered_supplier_ids.len()
        );

        Ok(())
    }

    /// Crawls up to `limit` candidates and returns how many were processed plus the
    /// candidates left for a later pass. `crawled` holds crawled URLs and
    /// `supplier:`-prefixed keys, so each pass favours suppliers not crawled yet;
    /// extra pages of an already-crawled supplier are returned at the back of the queue.
    async fn process_candidate_batch(
        &self,
        job_id: &str,
        candidates: Vec<CandidateUrl>,
        crawled: &mut HashSet<String>,
        limit: usize,
        requirement: &ProductRequirement,
        discovered_supplier_ids: &mut Vec<i64>,
        supplier_best_scores: &mut HashMap<i64, (f64, f64)>,
        start_time: Instant,
    ) -> anyhow::Result<(u32, Vec<CandidateUrl>)> {
        let mut selected = Vec::new();
        let mut leftover = Vec::new();
        let mut same_supplier = Vec::new();
        for cand in candidates {
            if crawled.contains(&cand.normalized_url) {
                continue;
            }
            if selected.len() >= limit {
                leftover.push(cand);
                continue;
            }
            let key = format!("supplier:{}", supplier_dedup_key(&cand));
            if crawled.contains(&key) {
                same_supplier.push(cand);
                continue;
            }
            crawled.insert(cand.normalized_url.clone());
            crawled.insert(key);
            selected.push(cand);
        }
        leftover.extend(same_supplier);

        if selected.is_empty() {
            return Ok((0, leftover));
        }

        let processed_count = selected.len() as u32;
        let concurrency = self.config.max_concurrent_fetches.clamp(1, 5);
        let semaphore = Arc::new(Semaphore::new(concurrency));
        let mut join_set = JoinSet::new();

        for cand in selected {
            let sem = Arc::clone(&semaphore);
            let crawler = self.crawler.clone();
            let gemini = self.gemini.clone();
            let req_clone = requirement.clone();
            let gate = Arc::clone(&self.fetch_gate);
            let delay = Duration::from_millis(self.config.crawler_request_delay_ms);
            let timeout = Duration::from_secs(self.config.request_timeout_seconds.max(1));

            join_set.spawn(async move {
                let _permit = sem.acquire_owned().await.ok()?;

                {
                    let mut next = gate.lock().await;
                    if *next > Instant::now() {
                        sleep(next.duration_since(Instant::now())).await;
                    }
                    *next = Instant::now() + delay;
                }
                let crawl_res =
                    tokio::time::timeout(timeout, crawler.fetch_page(&cand.normalized_url)).await;

                let (
                    page_title,
                    site_name,
                    clean_text,
                    http_status,
                    content_hash,
                    source_type,
                    crawl_emails,
                    crawl_phones,
                    resolved_url,
                ) = match crawl_res {
                    Ok(Ok(res)) => (
                        res.page_title,
                        res.site_name,
                        res.clean_text,
                        res.http_status as i64,
                        res.content_hash,
                        "web",
                        res.emails,
                        res.phones,
                        res.final_url,
                    ),
                    _ => {
                        if cand.title.is_empty() && cand.snippet.is_empty() {
                            return None;
                        }
                        let text = format!("{}. {}", cand.title, cand.snippet);
                        let hash = calculate_content_hash(&text);
                        (
                            cand.title.clone(),
                            None,
                            text,
                            200,
                            hash,
                            "search_snippet",
                            Vec::new(),
                            Vec::new(),
                            cand.normalized_url.clone(),
                        )
                    }
                };

                let mut payload = gemini
                    .extract_supplier_and_offers(
                        &resolved_url,
                        &page_title,
                        &clean_text,
                        &req_clone,
                    )
                    .await
                    .unwrap_or_else(|_| {
                        gemini.fallback_extract_supplier(
                            &resolved_url,
                            &page_title,
                            &clean_text,
                            &req_clone,
                        )
                    });

                // Prefer og:site_name if extracted company name looks noisy or empty
                if let Some(sname) = site_name.filter(|s| s.len() >= 3) {
                    let current_noisy = payload
                        .company_name
                        .as_deref()
                        .map(|n| {
                            let nl = n.to_lowercase();
                            nl.starts_with("jual ")
                                || nl.starts_with("harga ")
                                || nl.starts_with("beli ")
                                || nl.len() > 60
                        })
                        .unwrap_or(true);
                    let has_legal_prefix = payload
                        .company_name
                        .as_deref()
                        .map(|n| {
                            let nl = n.to_lowercase();
                            nl.starts_with("pt ")
                                || nl.starts_with("pt.")
                                || nl.starts_with("cv ")
                                || nl.starts_with("cv.")
                                || nl.contains("ltd")
                        })
                        .unwrap_or(false);
                    if current_noisy && !has_legal_prefix {
                        payload.company_name = Some(sname);
                    }
                }

                if payload.public_email.is_none() {
                    if let Some(em) = crawl_emails.into_iter().next() {
                        payload.evidence_items.push(ExtractedEvidencePayload {
                            field_path: "supplier.public_email".to_string(),
                            field_value: serde_json::json!(em),
                            evidence_quote: em.clone(),
                            confidence: 0.95,
                        });
                        payload.public_email = Some(em);
                    }
                }
                if payload.public_phone.is_none() {
                    if let Some(ph) = crawl_phones.into_iter().find(|p| {
                        let lower = p.to_lowercase();
                        !lower.contains('x') && !lower.contains('*')
                    }) {
                        payload.evidence_items.push(ExtractedEvidencePayload {
                            field_path: "supplier.public_phone".to_string(),
                            field_value: serde_json::json!(ph),
                            evidence_quote: ph.clone(),
                            confidence: 0.95,
                        });
                        payload.public_phone = Some(ph);
                    }
                }

                location::enrich_extracted_location(
                    &mut payload,
                    &cand.normalized_url,
                    &clean_text,
                );
                let strict_region_match = location::is_supplier_allowed_for_targets(
                    &payload,
                    &cand.normalized_url,
                    &clean_text,
                    &req_clone.target_regions,
                );

                Some(FetchedExtractedPage {
                    canonical_url: cand.normalized_url,
                    raw_url: cand.original_url,
                    page_title,
                    http_status,
                    content_hash,
                    source_type,
                    strict_region_match,
                    payload,
                })
            });
        }

        let mut deferred_broad: Vec<FetchedExtractedPage> = Vec::new();
        // Funnel counters so a job's events explain where candidate pages were dropped.
        let suppliers_before = discovered_supplier_ids.len();
        let (mut unreadable, mut snippet_only, mut no_product, mut out_of_region, mut duplicate) =
            (0u32, 0u32, 0u32, 0u32, 0u32);

        while !join_set.is_empty() {
            let next = tokio::time::timeout(Duration::from_secs(1), join_set.join_next()).await;
            if self.should_stop(job_id, start_time).await {
                join_set.abort_all();
                break;
            }

            let Ok(Some(res)) = next else {
                continue;
            };
            let Ok(Some(fetched)) = res else {
                unreadable += 1;
                continue;
            };
            {
                if fetched.source_type == "search_snippet" {
                    snippet_only += 1;
                }
                if fetched.payload.offers.is_empty() {
                    no_product += 1;
                    continue;
                }
                if !fetched.strict_region_match {
                    out_of_region += 1;
                    deferred_broad.push(fetched);
                    continue;
                }

                let (sup_id, fit_score, comp_score) = self
                    .persist_extracted_payload(job_id, fetched, requirement, false)
                    .await?;

                if sup_id.is_none() {
                    out_of_region += 1;
                }
                if let Some(sid) = sup_id {
                    if !discovered_supplier_ids.contains(&sid) {
                        discovered_supplier_ids.push(sid);
                        search_jobs::increment_job_counts(&self.db, job_id, 0, 1).await?;
                    } else {
                        duplicate += 1;
                    }
                    let existing = supplier_best_scores
                        .get(&sid)
                        .copied()
                        .unwrap_or((0.0, 0.0));
                    supplier_best_scores
                        .insert(sid, (fit_score.max(existing.0), comp_score.max(existing.1)));
                    self.recompute_job_rankings(job_id, supplier_best_scores)
                        .await?;
                }
            }
        }

        // Auto-broaden: if strict target-region results are scarce (< 12), include deferred global/unspecified suppliers
        if discovered_supplier_ids.len() < 12 && !deferred_broad.is_empty() {
            tracing::info!(
                "Job {}: Strict region yielded {} suppliers; broadening with {} additional candidates",
                job_id,
                discovered_supplier_ids.len(),
                deferred_broad.len()
            );
            for fetched in deferred_broad {
                if discovered_supplier_ids.len() >= 15 || self.should_stop(job_id, start_time).await
                {
                    break;
                }
                let (sup_id, fit_score, comp_score) = self
                    .persist_extracted_payload(job_id, fetched, requirement, true)
                    .await?;

                if let Some(sid) = sup_id {
                    if !discovered_supplier_ids.contains(&sid) {
                        discovered_supplier_ids.push(sid);
                        search_jobs::increment_job_counts(&self.db, job_id, 0, 1).await?;
                    }
                    let existing = supplier_best_scores
                        .get(&sid)
                        .copied()
                        .unwrap_or((0.0, 0.0));
                    supplier_best_scores
                        .insert(sid, (fit_score.max(existing.0), comp_score.max(existing.1)));
                    self.recompute_job_rankings(job_id, supplier_best_scores)
                        .await?;
                }
            }
        }

        let added = discovered_supplier_ids.len().saturating_sub(suppliers_before);
        let summary = format!(
            "{} halaman diproses: {} gagal dibuka, {} hanya dari cuplikan pencarian, {} tanpa produk yang cocok, {} di luar lokasi target, {} duplikat, {} supplier baru",
            processed_count, unreadable, snippet_only, no_product, out_of_region, duplicate, added
        );
        tracing::info!("Job {}: {}", job_id, summary);
        let _ = search_jobs::record_event(&self.db, job_id, "extract", Some(&summary), None).await;

        Ok((processed_count, leftover))
    }

    async fn triage_candidates(
        &self,
        raw_items: Vec<SearchItem>,
        req: &ProductRequirement,
    ) -> Vec<CandidateUrl> {
        let mut seen_urls = HashSet::new();
        let mut candidates = Vec::new();

        let prod_lower = req.product_name.to_lowercase();
        let part_lower = req.part_number.as_deref().unwrap_or("").to_lowercase();
        let target_codes = location::target_country_codes(&req.target_regions);
        let subregions = location::target_subregions(&req.target_regions);
        let targets_id = target_codes.iter().any(|c| c == "ID");
        let targets_cn = target_codes.iter().any(|c| c == "CN");

        for item in raw_items {
            let norm = match normalize_url(&item.url) {
                Ok(u) => u,
                Err(_) => continue,
            };
            if !seen_urls.insert(norm.clone()) {
                continue;
            }
            let domain = match extract_domain(&norm) {
                Some(d) => d,
                None => continue,
            };

            if is_excluded_domain(&domain) {
                continue;
            }

            let foreign_domain = location::is_foreign_candidate_url(&domain, &norm, &target_codes);

            let text_blob = format!("{} {} {}", item.title, item.snippet, norm).to_lowercase();
            let foreign_snippet = location::is_foreign_snippet(&text_blob, &target_codes);

            if validate_url_safety(&norm).await.is_err() {
                continue;
            }

            let mut score = if foreign_domain || foreign_snippet {
                -25
            } else {
                15
            };

            if !part_lower.is_empty() && text_blob.contains(&part_lower) {
                score += 40;
            }
            if text_blob.contains(&prod_lower) {
                score += 25;
            }
            if text_blob.contains("distributor")
                || text_blob.contains("manufacturer")
                || text_blob.contains("pabrik")
                || text_blob.contains("supplier")
            {
                score += 20;
            }
            if text_blob.contains("official")
                || text_blob.contains("resmi")
                || text_blob.contains("catalog")
            {
                score += 15;
            }

            if subregions
                .iter()
                .any(|sr| location::contains_ascii_word(&text_blob, sr))
            {
                score += 25;
            } else if targets_id
                && (domain.ends_with(".co.id")
                    || domain.ends_with(".id")
                    || location::contains_ascii_word(&text_blob, "indonesia"))
            {
                score += 15;
            } else if targets_cn
                && (domain.ends_with(".cn")
                    || domain.contains("made-in-china")
                    || domain.contains("alibaba")
                    || text_blob.contains("shenzhen")
                    || text_blob.contains("shanghai")
                    || text_blob.contains("guangzhou"))
            {
                score += 15;
            }

            // Deprioritize C2C retail marketplaces when official distributors exist
            if is_retail_marketplace(&domain) {
                score -= 25;
            }

            candidates.push(CandidateUrl {
                original_url: item.url,
                normalized_url: norm,
                title: item.title,
                snippet: item.snippet,
                triage_score: score,
            });
        }

        candidates.sort_by(|a, b| b.triage_score.cmp(&a.triage_score));
        candidates
    }

    async fn persist_extracted_payload(
        &self,
        job_id: &str,
        fetched: FetchedExtractedPage,
        req: &ProductRequirement,
        allow_broad: bool,
    ) -> anyhow::Result<(Option<i64>, f64, f64)> {
        let payload = fetched.payload;
        if payload.offers.is_empty() {
            return Ok((None, 0.0, 0.0));
        }
        let source_domain = extract_domain(&fetched.canonical_url).unwrap_or_default();
        let is_shared_platform =
            crate::services::entity_resolution::is_shared_supplier_platform(&source_domain);
        let mut override_type: Option<String> = None;
        if is_shared_platform {
            let identified = payload.company_name.as_deref().is_some_and(|name| {
                let lower = name.to_lowercase();
                !source_domain.contains(&lower)
                    && payload.evidence_items.iter().any(|e| {
                        e.field_path.contains("company_name")
                            && e.evidence_quote.to_lowercase().contains(&lower)
                    })
            });
            if !identified {
                if payload.offers.is_empty() {
                    return Ok((None, 0.0, 0.0));
                }
                override_type = Some("marketplace_directory".to_string());
            }
        }

        let mut supplier = EntityResolution::normalize_supplier(
            payload.company_name.as_deref(),
            payload.website.as_deref().or(Some(&fetched.canonical_url)),
            payload.country_name.as_deref(),
            payload.public_email.as_deref(),
            payload.public_phone.as_deref(),
            override_type
                .as_deref()
                .or(payload.supplier_type.as_deref()),
            payload.business_category.as_deref().or(Some(&req.category)),
        );
        supplier.address = payload.address.clone();
        supplier.contact_page_url = payload.contact_page_url.clone();
        if supplier.country_code.is_none() {
            supplier.country_code = payload.country_code.clone();
        }

        if !allow_broad {
            let target_codes = location::target_country_codes(&req.target_regions);
            if !target_codes.is_empty() {
                if let Some(ref sc) = supplier.country_code {
                    if !target_codes.iter().any(|tc| tc.eq_ignore_ascii_case(sc)) {
                        return Ok((None, 0.0, 0.0));
                    }
                }
            }
        }

        let supplier_id = suppliers::upsert_supplier(&self.db, &supplier).await?;
        supplier.id = supplier_id;

        let source = SupplierSource {
            id: 0,
            supplier_id,
            url: fetched.raw_url,
            canonical_url: fetched.canonical_url.clone(),
            page_type: Some("product_or_company".to_string()),
            page_title: Some(fetched.page_title),
            content_hash: Some(fetched.content_hash),
            retrieved_at: String::new(),
            http_status: Some(fetched.http_status),
            source_type: Some(fetched.source_type.to_string()),
        };
        let source_id = suppliers::add_supplier_source(&self.db, &source).await?;

        let mut saved_evidence = Vec::new();
        for ev_pay in &payload.evidence_items {
            let item = EvidenceItem {
                id: 0,
                supplier_id: Some(supplier_id),
                offer_id: None,
                field_path: ev_pay.field_path.clone(),
                field_value_json: serde_json::to_string(&ev_pay.field_value).ok(),
                source_id,
                evidence_quote: ev_pay.evidence_quote.clone(),
                confidence: ev_pay.confidence,
                verification_status: "unverified".to_string(),
                verified_by_user_id: None,
                verified_at: None,
            };
            let ev_id = evidence::add_evidence_item(&self.db, &item).await?;
            let mut item_saved = item;
            item_saved.id = ev_id;
            saved_evidence.push(item_saved);
        }

        let mut best_fit = 0.0;
        let mut best_comp = 0.0;

        if payload.offers.is_empty() {
            let dual =
                RankingService::calculate_scores(&supplier, None, None, req, &saved_evidence);
            best_fit = dual.product_fit_score;
            best_comp = dual.evidence_completeness_score;
        } else {
            for off_pay in &payload.offers {
                let prod = Product {
                    id: 0,
                    canonical_name: off_pay.product_name.clone(),
                    brand: off_pay.brand.clone(),
                    model: off_pay.model.clone(),
                    part_number: off_pay.part_number.clone(),
                    category_key: Some(req.category.clone()),
                    attributes_json: serde_json::to_string(&off_pay.attributes).ok(),
                    created_at: String::new(),
                    updated_at: String::new(),
                };
                let product_id = products::upsert_product(&self.db, &prod).await?;

                let mut offer = SupplierOffer {
                    id: 0,
                    supplier_id,
                    product_id,
                    source_id: Some(source_id),
                    product_url: Some(fetched.canonical_url.clone()),
                    description: off_pay.description.clone(),
                    attributes_json: serde_json::to_string(&off_pay.attributes).ok(),
                    price_amount: off_pay.price_amount,
                    currency: off_pay.currency.clone(),
                    price_basis: off_pay.price_basis.clone(),
                    moq: off_pay.moq.clone(),
                    lead_time: off_pay.lead_time.clone(),
                    availability_text: off_pay.availability_text.clone(),
                    product_fit_score: 0.0,
                    evidence_completeness_score: 0.0,
                    extracted_at: String::new(),
                };

                let offer_evidence: Vec<_> = saved_evidence
                    .iter()
                    .filter(|ev| {
                        if !ev.field_path.starts_with("offer")
                            && !ev.field_path.starts_with("product")
                        {
                            return true;
                        }
                        payload.offers.len() == 1
                            || ev
                                .evidence_quote
                                .to_lowercase()
                                .contains(&prod.canonical_name.to_lowercase())
                            || prod.part_number.as_deref().is_some_and(|pn| {
                                ev.evidence_quote
                                    .to_lowercase()
                                    .contains(&pn.to_lowercase())
                            })
                    })
                    .cloned()
                    .collect();
                let dual = RankingService::calculate_scores(
                    &supplier,
                    Some(&prod),
                    Some(&offer),
                    req,
                    &offer_evidence,
                );
                offer.product_fit_score = dual.product_fit_score;
                offer.evidence_completeness_score = dual.evidence_completeness_score;

                let offer_id = products::add_supplier_offer(&self.db, &offer).await?;
                for ev in offer_evidence.iter().filter(|e| {
                    e.field_path.starts_with("offer") || e.field_path.starts_with("product")
                }) {
                    sqlx::query(
                        "UPDATE evidence_items SET offer_id=? WHERE id=? AND offer_id IS NULL",
                    )
                    .bind(offer_id)
                    .bind(ev.id)
                    .execute(&self.db)
                    .await?;
                }

                if dual.product_fit_score > best_fit {
                    best_fit = dual.product_fit_score;
                }
                if dual.evidence_completeness_score > best_comp {
                    best_comp = dual.evidence_completeness_score;
                }
            }
        }

        if best_fit <= 0.0 {
            return Ok((None, 0.0, 0.0));
        }
        search_jobs::save_job_result(&self.db, job_id, supplier_id, best_fit, best_comp, 1).await?;
        Ok((Some(supplier_id), best_fit, best_comp))
    }

    async fn recompute_job_rankings(
        &self,
        job_id: &str,
        scores: &HashMap<i64, (f64, f64)>,
    ) -> anyhow::Result<()> {
        let mut sorted: Vec<(i64, f64, f64)> = scores
            .iter()
            .map(|(id, (fit, comp))| (*id, *fit, *comp))
            .collect();

        sorted.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal))
        });

        for (idx, (sid, fit, comp)) in sorted.into_iter().enumerate() {
            search_jobs::save_job_result(&self.db, job_id, sid, fit, comp, (idx + 1) as i64)
                .await?;
        }
        Ok(())
    }

    async fn detect_high_value_gaps(&self, job_id: &str, _req: &ProductRequirement) -> Vec<String> {
        let mut gaps = Vec::new();
        if let Ok(results) = search_jobs::get_job_results_detailed(&self.db, job_id).await {
            if results.is_empty() {
                gaps.push("contact".to_string());
                gaps.push("specifications".to_string());
                return gaps;
            }

            let has_any_price = results.iter().any(|r| {
                r.offers
                    .iter()
                    .any(|o| o.price_amount.is_some() && o.price_amount.unwrap_or(0.0) > 0.0)
            });
            if !has_any_price {
                gaps.push("pricing".to_string());
            }

            let has_any_contact = results
                .iter()
                .any(|r| r.supplier.public_email.is_some() || r.supplier.public_phone.is_some());
            if !has_any_contact {
                gaps.push("contact".to_string());
            }

            let avg_fit = results.iter().map(|r| r.product_fit_score).sum::<f64>()
                / results.len().max(1) as f64;
            if avg_fit < 60.0 {
                gaps.push("specifications".to_string());
            }
        }
        gaps
    }
}

/// Groups candidate pages that belong to the same supplier. A normal company site is
/// one supplier per domain; shared marketplaces/directories host many suppliers, so
/// they are keyed per page, except Indonetwork whose snippet starts with the seller name.
fn supplier_dedup_key(cand: &CandidateUrl) -> String {
    let domain = extract_domain(&cand.normalized_url).unwrap_or_default();
    if !crate::services::entity_resolution::is_shared_supplier_platform(&domain) {
        return domain;
    }
    if domain.ends_with("indonetwork.co.id") {
        if let Some(seller) = cand
            .snippet
            .split(" | ")
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return format!("{}#{}", domain, seller.to_lowercase());
        }
    }
    cand.normalized_url.clone()
}

fn is_retail_marketplace(domain: &str) -> bool {
    let d = domain.to_lowercase();
    [
        "tokopedia.com",
        "shopee.co.id",
        "bukalapak.com",
        "lazada.co.id",
        "blibli.com",
        "amazon.com",
        "ebay.com",
    ]
    .iter()
    .any(|m| d == *m || d.ends_with(&format!(".{}", m)))
}

fn is_excluded_domain(domain: &str) -> bool {
    let d = domain.to_lowercase();
    let blocked_hosts = [
        "facebook.com",
        "instagram.com",
        "tiktok.com",
        "twitter.com",
        "x.com",
        "youtube.com",
        "pinterest.com",
        "wikipedia.org",
        "wiktionary.org",
        "brainly.co.id",
        "kompas.com",
        "detik.com",
        "tribunnews.com",
        "cnnindonesia.com",
        "liputan6.com",
        "kbbi.web.id",
    ];
    if blocked_hosts
        .iter()
        .any(|b| d == *b || d.ends_with(&format!(".{}", b)))
    {
        return true;
    }
    if d.ends_with(".go.id") || d.ends_with(".gov") || d.ends_with(".ac.id") || d.ends_with(".edu")
    {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(url: &str, snippet: &str) -> CandidateUrl {
        CandidateUrl {
            original_url: url.to_string(),
            normalized_url: url.to_string(),
            title: String::new(),
            snippet: snippet.to_string(),
            triage_score: 0,
        }
    }

    #[test]
    fn supplier_dedup_key_groups_pages_of_the_same_supplier() {
        // Same company site -> one supplier.
        assert_eq!(
            supplier_dedup_key(&cand("https://pamanpack.co.id/produk/a", "")),
            supplier_dedup_key(&cand("https://pamanpack.co.id/produk/b", ""))
        );
        // Indonetwork listings from the same seller -> one supplier.
        assert_eq!(
            supplier_dedup_key(&cand(
                "https://www.indonetwork.co.id/product/kardus-1",
                "SAM Putramandiri | Semarang | Indonesia | Min. order 100"
            )),
            supplier_dedup_key(&cand(
                "https://www.indonetwork.co.id/product/kardus-2",
                "SAM Putramandiri | Semarang | Indonesia | Min. order 500"
            ))
        );
        // Different Indonetwork sellers stay separate.
        assert_ne!(
            supplier_dedup_key(&cand(
                "https://www.indonetwork.co.id/product/kardus-1",
                "SAM Putramandiri | Semarang | Indonesia"
            )),
            supplier_dedup_key(&cand(
                "https://www.indonetwork.co.id/product/kardus-3",
                "HALIM BOXINDO | Bandung | Indonesia"
            ))
        );
        // Other marketplaces without a known seller format are keyed per page.
        assert_ne!(
            supplier_dedup_key(&cand("https://www.tokopedia.com/a/kardus", "")),
            supplier_dedup_key(&cand("https://www.tokopedia.com/b/kardus", ""))
        );
    }
}
