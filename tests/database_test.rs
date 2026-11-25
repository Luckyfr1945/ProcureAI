use procure_ai::database::init_pool;
use procure_ai::database::repositories::{evidence, products, search_jobs, suppliers, users};
use procure_ai::domain::{EvidenceItem, Product, Supplier, SupplierOffer, SupplierSource};
use procure_ai::models::UserRole;

#[tokio::test]
async fn test_database_v2_schema_and_provenance() {
    let test_db_path = "instance/test_procure_ai.db";
    let _ = tokio::fs::remove_file(test_db_path).await;

    let pool = init_pool(&format!("sqlite://{}", test_db_path))
        .await
        .expect("Failed to initialize test pool and run v2 migrations");

    // 1. Verify user creation
    let user_id = users::create_user(&pool, "test_purchasing", "hash123", UserRole::Purchasing)
        .await
        .expect("Failed to create user");
    assert!(user_id > 0);

    // 2. Verify UUID search job creation & event log
    let job_id = search_jobs::create_job(&pool, user_id, "Bearing SKF 6205-2RS original")
        .await
        .expect("Failed to create search job");
    assert!(job_id.len() >= 32);

    let job = search_jobs::get_job(&pool, &job_id)
        .await
        .expect("Failed to fetch job")
        .expect("Job missing");
    assert_eq!(job.input_text, "Bearing SKF 6205-2RS original");
    assert_eq!(job.status, "queued");
    assert_eq!(job.current_stage.as_deref(), Some("understand"));

    let events = search_jobs::get_job_events(&pool, &job_id)
        .await
        .expect("Failed to fetch job events");
    assert!(!events.is_empty());

    // 3. Verify supplier upsert with domain deduplication
    let supplier = Supplier {
        id: 0,
        canonical_name: "SKF Group".to_string(),
        normalized_name: "skf group".to_string(),
        website: Some("https://www.skf.com".to_string()),
        normalized_domain: Some("skf.com".to_string()),
        country_code: Some("SE".to_string()),
        country_name: Some("Sweden".to_string()),
        address: Some("Gothenburg, Sweden".to_string()),
        supplier_type: Some("manufacturer".to_string()),
        business_category: Some("industrial_bearing".to_string()),
        public_email: Some("info@skf.com".to_string()),
        public_phone: Some("+46 31 337 1000".to_string()),
        contact_page_url: Some("https://www.skf.com/contact".to_string()),
        status: "discovered".to_string(),
        created_at: String::new(),
        updated_at: String::new(),
    };

    let supplier_id = suppliers::upsert_supplier(&pool, &supplier)
        .await
        .expect("Failed to upsert supplier");
    assert!(supplier_id > 0);

    // Duplicate domain should deduplicate to the same ID
    let dup_supplier_id = suppliers::upsert_supplier(&pool, &supplier)
        .await
        .expect("Failed to deduplicate supplier");
    assert_eq!(supplier_id, dup_supplier_id);

    // 4. Add source and evidence item
    let source = SupplierSource {
        id: 0,
        supplier_id,
        url: "https://www.skf.com/id/products/6205".to_string(),
        canonical_url: "https://www.skf.com/id/products/6205".to_string(),
        page_type: Some("product_page".to_string()),
        page_title: Some("SKF 6205 Deep Groove Ball Bearing".to_string()),
        content_hash: Some("sha256_demo".to_string()),
        retrieved_at: String::new(),
        http_status: Some(200),
        source_type: Some("web".to_string()),
    };
    let source_id = suppliers::add_supplier_source(&pool, &source)
        .await
        .expect("Failed to add source");

    let ev = EvidenceItem {
        id: 0,
        supplier_id: Some(supplier_id),
        offer_id: None,
        field_path: "offer.part_number".to_string(),
        field_value_json: Some(r#""6205""#.to_string()),
        source_id,
        evidence_quote: "SKF 6205 Deep groove ball bearing d=25mm D=52mm B=15mm".to_string(),
        confidence: 0.98,
        verification_status: "unverified".to_string(),
        verified_by_user_id: None,
        verified_at: None,
    };
    let ev_id = evidence::add_evidence_item(&pool, &ev)
        .await
        .expect("Failed to add evidence item");
    assert!(ev_id > 0);

    // 5. Add product + supplier offer + job result with dual scoring
    let product = Product {
        id: 0,
        canonical_name: "SKF 6205 Deep Groove Ball Bearing".to_string(),
        brand: Some("SKF".to_string()),
        model: Some("6205".to_string()),
        part_number: Some("6205".to_string()),
        category_key: Some("industrial_bearing".to_string()),
        attributes_json: Some(r#"{"inner_diameter_mm":25,"outer_diameter_mm":52,"width_mm":15}"#.to_string()),
        created_at: String::new(),
        updated_at: String::new(),
    };
    let product_id = products::upsert_product(&pool, &product)
        .await
        .expect("Failed to upsert product");

    let offer = SupplierOffer {
        id: 0,
        supplier_id,
        product_id,
        source_id: Some(source_id),
        product_url: Some("https://www.skf.com/id/products/6205".to_string()),
        description: Some("Original deep groove ball bearing 6205".to_string()),
        attributes_json: product.attributes_json.clone(),
        price_amount: None, // Not published! No hallucination!
        currency: None,
        price_basis: Some("unknown".to_string()),
        moq: None,
        lead_time: None,
        availability_text: None,
        product_fit_score: 92.5,
        evidence_completeness_score: 80.0,
        extracted_at: String::new(),
    };
    let offer_id = products::add_supplier_offer(&pool, &offer)
        .await
        .expect("Failed to add offer");
    assert!(offer_id > 0);

    search_jobs::save_job_result(&pool, &job_id, supplier_id, 92.5, 80.0, 1)
        .await
        .expect("Failed to save dual-scored job result");

    let results = search_jobs::get_job_results_detailed(&pool, &job_id)
        .await
        .expect("Failed to get detailed job results");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].supplier.canonical_name, "SKF Group");
    assert_eq!(results[0].product_fit_score, 92.5);
    assert_eq!(results[0].evidence_completeness_score, 80.0);

    // Existing products must be reusable without a search-provider request.
    let mut requirement = procure_ai::domain::ProductRequirement {
        product_name: "bearing SKF 6205".into(), category: "industrial_bearing".into(),
        brand: Some("SKF".into()), part_number: Some("6205".into()), synonyms: vec![],
        required_attributes: vec![], optional_attributes: vec![], supplier_types: vec![],
        target_regions: vec!["Sweden".into()], search_languages: vec![], pr_number: None,
    };
    let repeat = search_jobs::create_job(&pool, user_id, "same bearing").await.unwrap();
    let reused = procure_ai::services::stored_search::reuse_suppliers(&pool, &repeat, &requirement).await.unwrap();
    assert!(reused.contains_key(&supplier_id));
    requirement.target_regions = vec!["Surabaya".into(), "Indonesia".into()];
    let elsewhere = search_jobs::create_job(&pool, user_id, "same bearing elsewhere").await.unwrap();
    assert!(procure_ai::services::stored_search::reuse_suppliers(&pool, &elsewhere, &requirement).await.unwrap().is_empty());
    requirement.target_regions = vec!["Sweden".into()];
    requirement.part_number = Some("6305".into());
    assert!(procure_ai::services::stored_search::reuse_suppliers(&pool, &elsewhere, &requirement).await.unwrap().is_empty());

    // Historical foreign results must also be filtered when read or exported.
    requirement.target_regions = vec!["Indonesia".into()];
    let historical = search_jobs::create_job(&pool, user_id, "bearing Indonesia").await.unwrap();
    search_jobs::set_job_requirement(&pool, &historical, &serde_json::to_string(&requirement).unwrap()).await.unwrap();
    search_jobs::save_job_result(&pool, &historical, supplier_id, 99.0, 99.0, 1).await.unwrap();
    assert!(search_jobs::get_job_results_detailed(&pool, &historical).await.unwrap().is_empty());

    // Verify human verification flow
    evidence::verify_evidence(&pool, ev_id, user_id, "verified")
        .await
        .expect("Failed to verify evidence");
    suppliers::update_supplier_status(&pool, supplier_id, "verified_by_purchasing")
        .await
        .expect("Failed to verify supplier");

    let verified_count = suppliers::count_verified_suppliers(&pool).await.unwrap();
    assert_eq!(verified_count, 1);

    let _ = tokio::fs::remove_file(test_db_path).await;
}
