use procure_ai::config::Config;
use procure_ai::database::init_pool;
use procure_ai::database::repositories::{search_jobs, users};
use procure_ai::domain::{EvidenceItem, Product, ProductRequirement, Supplier, SupplierOffer};
use procure_ai::models::UserRole;
use procure_ai::providers::search::{MockSearchProvider, SearchProvider};
use procure_ai::services::gemini::{ExtractedSupplierPayload, GeminiClient, PlannedQuery};
use procure_ai::services::{location, EntityResolution, QueryPlanner, RankingService};
use procure_ai::utils::text::{extract_phones_and_whatsapp, extract_whatsapp_from_href, normalize_company_name};
use procure_ai::utils::url_safety::{is_private_ip, validate_url_safety};
use procure_ai::workers::DiscoveryWorker;
use std::net::IpAddr;
use std::sync::Arc;

#[tokio::test]
async fn test_url_safety_anti_ssrf() {
    let localhost_ip: IpAddr = "127.0.0.1".parse().unwrap();
    assert!(is_private_ip(&localhost_ip));

    let metadata_ip: IpAddr = "169.254.169.254".parse().unwrap();
    assert!(is_private_ip(&metadata_ip));

    let private_10: IpAddr = "10.0.0.1".parse().unwrap();
    assert!(is_private_ip(&private_10));

    let private_192: IpAddr = "192.168.1.1".parse().unwrap();
    assert!(is_private_ip(&private_192));

    assert!(validate_url_safety("http://127.0.0.1:8080/secret").await.is_err());
    assert!(validate_url_safety("http://localhost:3000/").await.is_err());
    assert!(
        validate_url_safety("http://169.254.169.254/latest/meta-data/")
            .await
            .is_err()
    );
    assert!(validate_url_safety("ftp://example.com/file.txt").await.is_err());
    assert!(validate_url_safety("file:///etc/passwd").await.is_err());
}

#[tokio::test]
async fn test_requirement_analysis_query_planner_and_search_provider() {
    let gemini = GeminiClient::new(String::new(), "gemini-2.5-flash".to_string());
    let req_text = "SKF 6205 bearing supplier";

    let req = gemini.fallback_analyze_requirement(
        req_text,
        &["worldwide".to_string(), "Indonesia".to_string()],
        &["en".to_string(), "id".to_string()],
    );

    assert_eq!(req.category, "industrial_bearing");
    assert_eq!(req.part_number.as_deref(), Some("6205"));
    assert_eq!(req.brand.as_deref(), Some("SKF"));

    let planner = QueryPlanner::new(gemini);
    let queries = planner
        .generate_search_plan(&req, 4)
        .await
        .expect("Query planning failed");
    assert!(!queries.is_empty());
    assert!(queries.iter().any(|q| q.query_text.contains("6205")));

    let provider = MockSearchProvider;
    let page1 = provider.search(&queries[0].query_text, 1).await.expect("Page 1 failed");
    assert!(!page1.results.is_empty());
    assert!(page1.has_next_page);
}

#[tokio::test]
async fn test_entity_resolution_and_dual_ranking() {
    assert_eq!(normalize_company_name("SKF Bearings Group Inc."), "skf bearings");
    assert_eq!(
        normalize_company_name("PT. Mitra Maju Teknik Tbk"),
        "mitra maju teknik"
    );

    let sup: Supplier = EntityResolution::normalize_supplier(
        Some("SKF Group"),
        Some("https://www.skf.com"),
        Some("Sweden"),
        Some("sales@skf.com"),
        Some("+46 31 337 1000"),
        Some("manufacturer"),
        Some("industrial_bearing"),
    );

    assert_eq!(sup.normalized_domain.as_deref(), Some("skf.com"));
    assert_eq!(sup.country_code.as_deref(), Some("SE"));

    let prod = Product {
        id: 1,
        canonical_name: "SKF 6205 Deep Groove Ball Bearing".to_string(),
        brand: Some("SKF".to_string()),
        model: Some("6205".to_string()),
        part_number: Some("6205".to_string()),
        category_key: Some("industrial_bearing".to_string()),
        attributes_json: Some(
            r#"{"bearing_type":"deep groove","inner_diameter_mm":25,"outer_diameter_mm":52,"width_mm":15}"#.to_string(),
        ),
        created_at: String::new(),
        updated_at: String::new(),
    };

    let offer = SupplierOffer {
        id: 1,
        supplier_id: 1,
        product_id: 1,
        source_id: Some(1),
        product_url: Some("https://www.skf.com/6205".to_string()),
        description: Some("Original SKF 6205 bearing".to_string()),
        attributes_json: prod.attributes_json.clone(),
        price_amount: Some(18.5),
        currency: Some("USD".to_string()),
        price_basis: Some("unit".to_string()),
        moq: Some("10 pcs".to_string()),
        lead_time: Some("3 days".to_string()),
        availability_text: Some("In stock".to_string()),
        product_fit_score: 0.0,
        evidence_completeness_score: 0.0,
        extracted_at: String::new(),
    };

    let req = ProductRequirement {
        product_name: "Bearing SKF 6205".to_string(),
        category: "industrial_bearing".to_string(),
        brand: Some("SKF".to_string()),
        part_number: Some("6205".to_string()),
        synonyms: vec![],
        required_attributes: vec![
            "bearing type deep groove".to_string(),
            "inner diameter 25mm".to_string(),
            "outer diameter 52mm".to_string(),
            "width 15mm".to_string(),
        ],
        optional_attributes: vec![],
        supplier_types: vec!["manufacturer".to_string(), "authorized_distributor".to_string()],
        target_regions: vec!["worldwide".to_string()],
        search_languages: vec!["en".to_string()],
        pr_number: None,
    };

    let evidence = vec![
        EvidenceItem {
            id: 1,
            supplier_id: Some(1),
            offer_id: Some(1),
            field_path: "supplier.public_email".to_string(),
            field_value_json: Some(r#""sales@skf.com""#.to_string()),
            source_id: 1,
            evidence_quote: "Contact sales@skf.com".to_string(),
            confidence: 0.95,
            verification_status: "unverified".to_string(),
            verified_by_user_id: None,
            verified_at: None,
        },
        EvidenceItem {
            id: 2,
            supplier_id: Some(1),
            offer_id: Some(1),
            field_path: "offer.part_number".to_string(),
            field_value_json: Some(r#""6205""#.to_string()),
            source_id: 1,
            evidence_quote: "Bearing 6205 d=25mm D=52mm B=15mm".to_string(),
            confidence: 0.99,
            verification_status: "unverified".to_string(),
            verified_by_user_id: None,
            verified_at: None,
        },
    ];

    let scores = RankingService::calculate_scores(&sup, Some(&prod), Some(&offer), &req, &evidence);
    assert!(
        scores.product_fit_score >= 90.0,
        "Expected high product fit, got {}",
        scores.product_fit_score
    );
    assert!(
        scores.evidence_completeness_score >= 85.0,
        "Expected high evidence completeness, got {}",
        scores.evidence_completeness_score
    );
}

#[tokio::test]
async fn test_discovery_worker_v2_end_to_end() {
    let test_db = "instance/test_discovery.db";
    let _ = tokio::fs::remove_file(test_db).await;

    let mut config = Config::from_env();
    config.database_url = format!("sqlite://{}", test_db);
    config.search_provider = "mock".to_string();
    config.gemini_api_key = String::new();
    config.crawler_request_delay_ms = 0;
    config.request_timeout_seconds = 3;
    config.max_crawl_pages_per_job = 2;
    config.max_search_requests_per_job = 3;
    config.max_job_duration_seconds = 20;

    let pool = init_pool(&config.database_url)
        .await
        .expect("Failed to initialize test pool");

    let user_id = users::create_user(&pool, "worker_user", "hash", UserRole::Purchasing)
        .await
        .unwrap();

    let job_id = search_jobs::create_job(&pool, user_id, "SKF 6205 bearing supplier")
        .await
        .unwrap();

    let job = search_jobs::get_job(&pool, &job_id).await.unwrap().unwrap();
    assert_eq!(job.status, "queued");

    let worker = DiscoveryWorker::new(Arc::new(config), pool.clone());
    worker.process_job(job).await.expect("Worker job failed");

    let finished_job = search_jobs::get_job(&pool, &job_id).await.unwrap().unwrap();
    assert_eq!(finished_job.status, "completed");
    assert_eq!(finished_job.current_stage.as_deref(), Some("completed"));
    assert!(finished_job.round_count > 0);
    assert!(finished_job.requirement_json.is_some());

    let queries = search_jobs::get_queries(&pool, &job_id).await.unwrap();
    assert!(!queries.is_empty());

    let events = search_jobs::get_job_events(&pool, &job_id).await.unwrap();
    assert!(events.iter().any(|e| e.stage == "understand"));
    assert!(events.iter().any(|e| e.stage == "discover"));

    let _ = tokio::fs::remove_file(test_db).await;
}

#[tokio::test]
async fn test_location_query_planning_and_whatsapp_extraction() {
    let gemini = GeminiClient::new(String::new(), "gemini-2.5-flash".to_string());

    // 1. Foreign location (China) must NOT contain "indonesia" in queries
    let china_req = gemini.fallback_analyze_requirement(
        "Industrial servo motor 400W",
        &["China".to_string()],
        &["en".to_string()],
    );
    let china_queries = gemini.fallback_plan_queries(&china_req, 5);
    assert!(!china_queries.is_empty());
    assert!(china_queries.iter().all(|q| !q.query_text.to_lowercase().contains("indonesia")));
    assert!(china_queries.iter().any(|q| q.query_text.to_lowercase().contains("china")));

    // 2. Indonesia + specific region must include the chosen Indonesian region
    let id_req = gemini.fallback_analyze_requirement(
        "Bearing SKF 6205",
        &["Surabaya, Jawa Timur".to_string(), "Indonesia".to_string()],
        &["id".to_string(), "en".to_string()],
    );
    let id_queries = gemini.fallback_plan_queries(&id_req, 5);
    assert!(id_queries.iter().any(|q| q.query_text.to_lowercase().contains("surabaya")));

    // 3. WhatsApp & Indonesian phone extraction
    assert_eq!(
        extract_whatsapp_from_href("https://wa.me/6281234567890?text=Halo").as_deref(),
        Some("+6281234567890")
    );
    let phones = extract_phones_and_whatsapp(
        "Hubungi sales kami via WA 0812-3456-7890 atau Telp: (021) 555-1234",
    );
    assert!(phones.iter().any(|p| p.contains("0812-3456-7890")));
    assert!(phones.iter().any(|p| p.contains("555-1234")));

    // 4. Location ranking prefers matching country over mismatched country
    let cn_sup = EntityResolution::normalize_supplier(
        Some("Shenzhen Servo Co., Ltd"),
        Some("https://www.szservo.cn"),
        Some("China"),
        Some("sales@szservo.cn"),
        None,
        Some("manufacturer"),
        Some("general_industrial"),
    );
    let id_sup = EntityResolution::normalize_supplier(
        Some("PT Teknik Lokal"),
        Some("https://www.tekniklokal.co.id"),
        Some("Indonesia"),
        Some("sales@tekniklokal.co.id"),
        None,
        Some("manufacturer"),
        Some("general_industrial"),
    );
    let cn_score = score_proven_product(&cn_sup, &china_req);
    let id_score = score_proven_product(&id_sup, &china_req);
    assert!(
        cn_score.product_fit_score > id_score.product_fit_score,
        "Expected China supplier ({}) to outrank Indonesia supplier ({}) for China target",
        cn_score.product_fit_score,
        id_score.product_fit_score
    );
}

#[tokio::test]
async fn test_strict_location_enforcement_and_city_ranking() {
    let gemini = GeminiClient::new(String::new(), "gemini-2.5-flash".to_string());
    let sby_req = gemini.fallback_analyze_requirement(
        "Bearing SKF 6205-2RS",
        &[
            "Surabaya, Gresik, Sidoarjo, Jawa Timur".to_string(),
            "Indonesia".to_string(),
        ],
        &["id".to_string(), "en".to_string()],
    );

    // 1. Even if LLM returns unlocalized queries, enforce_query_regions anchors EVERY query to Indonesia / Surabaya
    let raw_llm_queries = vec![
        PlannedQuery {
            query_text: "SKF 6205-2RS bearing".to_string(),
            query_type: "exact".to_string(),
            language: "en".to_string(),
            rationale: "test".to_string(),
        },
        PlannedQuery {
            query_text: "SKF deep groove ball bearing 6205-2RS official".to_string(),
            query_type: "manufacturer".to_string(),
            language: "en".to_string(),
            rationale: "test".to_string(),
        },
    ];
    let enforced = location::enforce_query_regions(raw_llm_queries, &sby_req, 5);
    assert!(enforced.iter().all(|q| {
        let ql = q.query_text.to_lowercase();
        ql.contains("indonesia") || ql.contains("surabaya")
    }));

    // 2. Fallback queries must also never drop location
    let fb_queries = gemini.fallback_plan_queries(&sby_req, 5);
    assert!(fb_queries.iter().all(|q| {
        let ql = q.query_text.to_lowercase();
        ql.contains("indonesia") || ql.contains("surabaya")
    }));

    // 3. Phone-prefix country inference
    assert_eq!(location::country_code_from_phone("+1(707)236-0355"), Some("US"));
    assert_eq!(location::country_code_from_phone("+8618365833398"), Some("CN"));
    assert_eq!(location::country_code_from_phone("+628123456789"), Some("ID"));
    assert_eq!(location::country_code_from_phone("0812-3456-7890"), Some("ID"));
    assert_eq!(location::country_code_from_phone("(021) 555-1234"), Some("ID"));

    // 4. Foreign URL and snippet triage filtering for Indonesia target
    let id_targets = location::target_country_codes(&sby_req.target_regions);
    assert!(location::is_foreign_candidate_url(
        "amazon.com",
        "https://www.amazon.com/SKF-6205-2RS/dp/B00123",
        &id_targets
    ));
    assert!(location::is_foreign_candidate_url(
        "bearmaxusa.com",
        "https://bearmaxusa.com/products/skf-6205",
        &id_targets
    ));
    assert!(location::is_foreign_candidate_url(
        "us.misumi-ec.com",
        "https://us.misumi-ec.com/vona2/detail/123",
        &id_targets
    ));
    assert!(!location::is_foreign_candidate_url(
        "tokokdmp.com",
        "https://www.tokokdmp.com/product/bearing-skf-6205-2rs/",
        &id_targets
    ));

    // 5. Post-extraction location gate rejects US (+1) and China (+86) suppliers when targeting Indonesia
    let mut us_payload = ExtractedSupplierPayload {
        company_name: Some("Bearmax USA".to_string()),
        website: Some("https://bearmaxusa.com".to_string()),
        country_code: None,
        country_name: None,
        address: None,
        supplier_type: Some("distributor".to_string()),
        business_category: Some("industrial_bearing".to_string()),
        public_email: Some("sales@bearmaxusa.com".to_string()),
        public_phone: Some("+1(707)236-0355".to_string()),
        contact_page_url: None,
        offers: vec![],
        evidence_items: vec![],
    };
    location::enrich_extracted_location(
        &mut us_payload,
        "https://bearmaxusa.com",
        "Bearmax USA Bearing Distributor California +1(707)236-0355",
    );
    assert_eq!(us_payload.country_code.as_deref(), Some("US"));
    assert!(!location::is_supplier_allowed_for_targets(
        &us_payload,
        "https://bearmaxusa.com",
        "Bearmax USA Bearing Distributor California",
        &sby_req.target_regions
    ));

    let mut cn_payload = ExtractedSupplierPayload {
        company_name: Some("6205 Bearing Co".to_string()),
        website: Some("https://examplebearing.com".to_string()),
        country_code: None,
        country_name: None,
        address: None,
        supplier_type: Some("manufacturer".to_string()),
        business_category: Some("industrial_bearing".to_string()),
        public_email: None,
        public_phone: Some("+8618365833398".to_string()),
        contact_page_url: None,
        offers: vec![],
        evidence_items: vec![],
    };
    location::enrich_extracted_location(
        &mut cn_payload,
        "https://examplebearing.com",
        "SKF 6205 deep groove ball bearing contact +8618365833398",
    );
    assert_eq!(cn_payload.country_code.as_deref(), Some("CN"));
    assert!(!location::is_supplier_allowed_for_targets(
        &cn_payload,
        "https://examplebearing.com",
        "SKF 6205 deep groove ball bearing contact +8618365833398",
        &sby_req.target_regions
    ));

    // Indonesian .com vendor in Surabaya passes & gets Surabaya city populated into address
    let mut sby_payload = ExtractedSupplierPayload {
        company_name: Some("PT Karya Bearing".to_string()),
        website: Some("https://karyabearing.com".to_string()),
        country_code: None,
        country_name: None,
        address: None,
        supplier_type: Some("distributor".to_string()),
        business_category: Some("industrial_bearing".to_string()),
        public_email: Some("sales@karyabearing.com".to_string()),
        public_phone: Some("+6281234567890".to_string()),
        contact_page_url: None,
        offers: vec![],
        evidence_items: vec![],
    };
    let sby_text = "PT Karya Bearing distributor resmi SKF. Alamat: Surabaya Jawa Timur WA +6281234567890";
    location::enrich_extracted_location(&mut sby_payload, "https://karyabearing.com", sby_text);
    assert_eq!(sby_payload.country_code.as_deref(), Some("ID"));
    assert_eq!(sby_payload.address.as_deref(), Some("Surabaya, Jawa Timur"));
    assert!(location::is_supplier_allowed_for_targets(
        &sby_payload,
        "https://karyabearing.com",
        sby_text,
        &sby_req.target_regions
    ));

    // 6. Surabaya supplier outranks Jakarta supplier when Surabaya is selected
    let mut sby_sup = EntityResolution::normalize_supplier(
        Some("PT Karya Bearing"),
        Some("https://karyabearing.com"),
        Some("Indonesia"),
        Some("sales@karyabearing.com"),
        Some("+6281234567890"),
        Some("distributor"),
        Some("industrial_bearing"),
    );
    sby_sup.address = Some("Surabaya, Jawa Timur".to_string());

    let mut jkt_sup = EntityResolution::normalize_supplier(
        Some("PT Jakarta Bearing"),
        Some("https://jakartabearing.co.id"),
        Some("Indonesia"),
        Some("sales@jakartabearing.co.id"),
        Some("+62215551234"),
        Some("distributor"),
        Some("industrial_bearing"),
    );
    jkt_sup.address = Some("Jakarta".to_string());

    let sby_score = score_proven_product(&sby_sup, &sby_req);
    let jkt_score = score_proven_product(&jkt_sup, &sby_req);
    assert!(
        sby_score.product_fit_score > jkt_score.product_fit_score,
        "Expected Surabaya vendor ({}) to outrank Jakarta vendor ({}) for Surabaya target",
        sby_score.product_fit_score,
        jkt_score.product_fit_score
    );
}

fn score_proven_product(supplier: &procure_ai::domain::Supplier, req: &procure_ai::domain::ProductRequirement) -> procure_ai::services::ranking::DualScore {
 let product = Product { id:1, canonical_name:req.product_name.clone(), brand:req.brand.clone(), model:None, part_number:req.part_number.clone(), category_key:Some(req.category.clone()), attributes_json:None, created_at:String::new(), updated_at:String::new() };
 let evidence = EvidenceItem { id:1,supplier_id:Some(supplier.id),offer_id:None,field_path:"offer.product_name".into(),field_value_json:None,source_id:1,evidence_quote:req.product_name.clone(),confidence:1.0,verification_status:"unverified".into(),verified_by_user_id:None,verified_at:None };
 RankingService::calculate_scores(supplier,Some(&product),None,req,&[evidence])
}

#[test]
fn test_measurements_are_not_part_numbers_and_part_numbers_tolerate_spacing() {
    let gemini = GeminiClient::new(String::new(), "gemini-2.5-flash".to_string());

    // Sizes describe the product; treating them as a part number rejected most supplier pages.
    for text in ["pallet plastik HDPE 1200x1000", "pipa stainless 2inch", "plastik wrapping 25mm"] {
        let req = gemini.fallback_analyze_requirement(text, &["Indonesia".to_string()], &["id".to_string()]);
        assert_eq!(req.part_number, None, "'{}' should not produce a part number", text);
    }
    let bearing = gemini.fallback_analyze_requirement("Bearing SKF 6205-2RS", &["Indonesia".to_string()], &["id".to_string()]);
    assert_eq!(bearing.part_number.as_deref(), Some("6205-2RS"));

    // A real part number still matches when the page uses other separators, but not a different model.
    assert!(procure_ai::services::gemini::product_mentioned_on_page(&bearing, "Ready stock SKF 6205 2RS original"));
    assert!(procure_ai::services::gemini::product_mentioned_on_page(&bearing, "Bearing 6205-2RS C3"));
    assert!(!procure_ai::services::gemini::product_mentioned_on_page(&bearing, "Bearing 6206-2RS C3"));
}
