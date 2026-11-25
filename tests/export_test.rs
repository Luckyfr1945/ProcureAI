use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use procure_ai::api::app_router;
use procure_ai::auth::session::create_session_token;
use procure_ai::config::Config;
use procure_ai::database::init_pool;
use procure_ai::database::repositories::{evidence, products, search_jobs, suppliers, users};
use procure_ai::domain::{EvidenceItem, Product, Supplier, SupplierOffer, SupplierSource};
use procure_ai::models::UserRole;
use procure_ai::state::AppState;
use tower::ServiceExt;

#[tokio::test]
async fn test_export_search_results_v2_and_human_verification() {
    let test_db = "instance/test_export.db";
    let _ = tokio::fs::remove_file(test_db).await;

    let mut config = Config::from_env();
    config.database_url = format!("sqlite://{}", test_db);
    config.session_secret = "test-secret-key-32-chars-long-procure-ai".to_string();

    let pool = init_pool(&config.database_url)
        .await
        .expect("Failed to initialize test pool");

    let user_id = users::create_user(&pool, "exporter", "hash", UserRole::Purchasing)
        .await
        .unwrap();

    let job_id = search_jobs::create_job(&pool, user_id, "SKF 6205 bearings")
        .await
        .unwrap();

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
        public_email: Some("sales@skf.com".to_string()),
        public_phone: Some("+46 31 337 1000".to_string()),
        contact_page_url: Some("https://www.skf.com/contact".to_string()),
        status: "discovered".to_string(),
        created_at: String::new(),
        updated_at: String::new(),
    };

    let supplier_id = suppliers::upsert_supplier(&pool, &supplier).await.unwrap();

    let source_id = suppliers::add_supplier_source(
        &pool,
        &SupplierSource {
            id: 0,
            supplier_id,
            url: "https://www.skf.com/6205".to_string(),
            canonical_url: "https://www.skf.com/6205".to_string(),
            page_type: Some("product_page".to_string()),
            page_title: Some("SKF 6205 Datasheet".to_string()),
            content_hash: Some("abc".to_string()),
            retrieved_at: String::new(),
            http_status: Some(200),
            source_type: Some("web".to_string()),
        },
    )
    .await
    .unwrap();

    let _ = evidence::add_evidence_item(
        &pool,
        &EvidenceItem {
            id: 0,
            supplier_id: Some(supplier_id),
            offer_id: None,
            field_path: "supplier.company_name".to_string(),
            field_value_json: Some(r#""SKF Group""#.to_string()),
            source_id,
            evidence_quote: "SKF Group Official Industrial Bearings".to_string(),
            confidence: 0.99,
            verification_status: "unverified".to_string(),
            verified_by_user_id: None,
            verified_at: None,
        },
    )
    .await
    .unwrap();

    let prod_id = products::upsert_product(
        &pool,
        &Product {
            id: 0,
            canonical_name: "SKF 6205 Bearing".to_string(),
            brand: Some("SKF".to_string()),
            model: Some("6205".to_string()),
            part_number: Some("6205".to_string()),
            category_key: Some("industrial_bearing".to_string()),
            attributes_json: Some(r#"{"inner_diameter_mm":25}"#.to_string()),
            created_at: String::new(),
            updated_at: String::new(),
        },
    )
    .await
    .unwrap();

    let _ = products::add_supplier_offer(
        &pool,
        &SupplierOffer {
            id: 0,
            supplier_id,
            product_id: prod_id,
            source_id: Some(source_id),
            product_url: Some("https://www.skf.com/6205".to_string()),
            description: Some("Original SKF 6205".to_string()),
            attributes_json: Some(r#"{"inner_diameter_mm":25}"#.to_string()),
            price_amount: None,
            currency: None,
            price_basis: Some("unknown".to_string()),
            moq: None,
            lead_time: None,
            availability_text: None,
            product_fit_score: 95.0,
            evidence_completeness_score: 80.0,
            extracted_at: String::new(),
        },
    )
    .await
    .unwrap();

    search_jobs::save_job_result(&pool, &job_id, supplier_id, 95.0, 80.0, 1)
        .await
        .unwrap();

    let token = create_session_token(
        user_id,
        "exporter",
        "purchasing",
        &config.session_secret,
        3600,
    )
    .unwrap();

    let state = AppState::new(config, pool);
    let app = app_router(state);

    // 1. Export Excel via /api/v1/search-jobs/{job_id}/export.xlsx
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/search-jobs/{}/export.xlsx", job_id))
                .method("GET")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap(),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    );

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(body_bytes.len() > 100);
    assert_eq!(&body_bytes[0..2], b"PK");

    // 2. Verify human verification endpoint POST /api/v1/suppliers/{id}/verify
    let verify_resp = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/suppliers/{}/verify", supplier_id))
                .method("POST")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(verify_resp.status(), StatusCode::OK);
    let vbytes = verify_resp.into_body().collect().await.unwrap().to_bytes();
    let vjson: serde_json::Value = serde_json::from_slice(&vbytes).unwrap();
    assert_eq!(vjson["status"], "verified_by_purchasing");

    let _ = tokio::fs::remove_file(test_db).await;
}
