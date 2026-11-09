use procure_ai::config::Config;
use procure_ai::database::init_pool;
use procure_ai::database::repositories::{searches, users};
use procure_ai::models::{SearchPreferences, Supplier, SupplierProduct, UserRole};
use procure_ai::providers::gemini::GeminiClient;
use procure_ai::providers::search::{MockSearchProvider, SearchProvider};
use procure_ai::services::{deduplication, normalization, RankingService};
use procure_ai::utils::text::normalize_company_name;
use procure_ai::utils::url_safety::{is_private_ip, validate_url_safety};
use procure_ai::workers::DiscoveryWorker;
use std::net::IpAddr;
use std::sync::Arc;

#[tokio::test]
async fn test_url_safety_anti_ssrf() {
    // 1. Private / loopback IPs must be blocked
    let localhost_ip: IpAddr = "127.0.0.1".parse().unwrap();
    assert!(is_private_ip(&localhost_ip));

    let metadata_ip: IpAddr = "169.254.169.254".parse().unwrap();
    assert!(is_private_ip(&metadata_ip));

    let private_10: IpAddr = "10.0.0.1".parse().unwrap();
    assert!(is_private_ip(&private_10));

    let private_192: IpAddr = "192.168.1.1".parse().unwrap();
    assert!(is_private_ip(&private_192));

    // 2. URLs to private / internal resources must be rejected
    assert!(validate_url_safety("http://127.0.0.1:8080/secret").await.is_err());
    assert!(validate_url_safety("http://localhost:3000/").await.is_err());
    assert!(validate_url_safety("http://169.254.169.254/latest/meta-data/").await.is_err());
    assert!(validate_url_safety("ftp://example.com/file.txt").await.is_err());
    assert!(validate_url_safety("file:///etc/passwd").await.is_err());
}

#[tokio::test]
async fn test_query_expansion_and_search_provider() {
    let gemini = GeminiClient::new(String::new(), "gemini-2.5-flash".to_string());
    let req = "SKF 6205 bearing supplier international";
    let exp = gemini.fallback_query_expansion(req, 4);

    assert!(!exp.search_queries.is_empty());
    assert!(exp.search_queries.iter().any(|q| q.contains("6205")));

    // Search provider pagination test
    let provider = MockSearchProvider;
    let page1 = provider.search(req, 1).await.expect("Page 1 failed");
    assert!(!page1.results.is_empty());
    assert!(page1.has_next_page);

    let page3 = provider.search(req, 3).await.expect("Page 3 failed");
    assert!(!page3.results.is_empty());
    assert!(!page3.has_next_page);
}

#[tokio::test]
async fn test_supplier_normalization_dedup_and_ranking() {
    // Normalization test
    assert_eq!(normalize_company_name("SKF Bearings Group Inc."), "skf bearings");
    assert_eq!(normalize_company_name("PT. Mitra Maju Teknik Tbk"), "mitra maju teknik");

    let s1 = Supplier {
        id: 1,
        canonical_key: "skf.com".to_string(),
        company_name: "SKF Group Inc".to_string(),
        normalized_name: "skf".to_string(),
        website: Some("https://www.skf.com".to_string()),
        domain: Some("skf.com".to_string()),
        country: Some("Sweden".to_string()),
        city: Some("Gothenburg".to_string()),
        email: Some("sales@skf.com".to_string()),
        phone: Some("+46 31 337 1000".to_string()),
        category: Some("Industrial Bearings".to_string()),
        description: Some("Global manufacturer of rolling bearings and units".to_string()),
        verification_status: "verified".to_string(),
        created_at: String::new(),
        updated_at: String::new(),
    };

    let norm_s1 = normalization::normalize_supplier(s1.clone());
    assert_eq!(norm_s1.canonical_key, "skf.com");

    // Deduplication test
    let mut s2 = s1.clone();
    s2.company_name = "SKF Official Distributor".to_string();
    assert!(deduplication::are_duplicate_suppliers(&s1, &s2));

    // Ranking test
    let products = vec![SupplierProduct {
        id: 1,
        supplier_id: 1,
        source_id: None,
        product_name: "SKF 6205 Deep Groove Ball Bearing".to_string(),
        brand: Some("SKF".to_string()),
        model_or_specification: Some("6205".to_string()),
        price_amount: Some(18.50),
        price_currency: Some("USD".to_string()),
        minimum_order_quantity: Some("10 pcs".to_string()),
        availability_note: Some("In stock".to_string()),
        product_url: Some("https://www.skf.com/bearing-6205".to_string()),
        price_observed_at: Some("2026-11-09".to_string()),
    }];

    let prefs = SearchPreferences {
        countries: vec!["Sweden".to_string()],
        exclude_marketplaces: true,
        prioritize_manufacturers: true,
    };

    let specs = vec!["SKF".to_string(), "6205".to_string()];
    let score = RankingService::calculate_score(&s1, &products, "SKF 6205 bearings", &specs, &prefs);

    assert!(score.total_score >= 80.0, "Expected high score, got {}", score.total_score);
    assert!(score.product_match_score > 30.0);
    assert_eq!(score.location_score, 20.0);
    assert_eq!(score.price_score, 15.0);
}

#[tokio::test]
async fn test_discovery_worker_end_to_end() {
    let test_db = "instance/test_discovery.db";
    let _ = tokio::fs::remove_file(test_db).await;

    let mut config = Config::from_env();
    config.database_url = format!("sqlite://{}", test_db);
    config.search_provider = "mock".to_string();
    config.crawler_request_delay_ms = 0;

    let pool = init_pool(&config.database_url)
        .await
        .expect("Failed to initialize test pool");

    let user_id = users::create_user(&pool, "worker_user", "hash", UserRole::Purchasing)
        .await
        .unwrap();

    let job_id = searches::create_job(
        &pool,
        user_id,
        "SKF 6205 bearing supplier",
        Some(r#"{"countries":[], "exclude_marketplaces":false, "prioritize_manufacturers":true}"#),
    )
    .await
    .unwrap();

    let job = searches::get_job(&pool, job_id).await.unwrap().unwrap();
    assert_eq!(job.status, "queued");

    let worker = DiscoveryWorker::new(Arc::new(config), pool.clone());
    worker.process_job(job).await.expect("Worker job failed");

    // Verify job transitioned to completed
    let finished_job = searches::get_job(&pool, job_id).await.unwrap().unwrap();
    assert_eq!(finished_job.status, "completed");
    assert!(finished_job.round_count > 0);

    // Verify queries were saved
    let queries = searches::get_queries(&pool, job_id).await.unwrap();
    assert!(!queries.is_empty());

    // Clean up
    let _ = tokio::fs::remove_file(test_db).await;
}
