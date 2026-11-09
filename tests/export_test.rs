use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use procure_ai::api::app_router;
use procure_ai::auth::session::create_session_token;
use procure_ai::config::Config;
use procure_ai::database::init_pool;
use procure_ai::database::repositories::{searches, suppliers, users};
use procure_ai::models::{Supplier, UserRole};
use procure_ai::state::AppState;
use tower::ServiceExt;

#[tokio::test]
async fn test_export_search_results_to_excel() {
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

    let job_id = searches::create_job(&pool, user_id, "SKF 6205 bearings", None)
        .await
        .unwrap();

    let supplier = Supplier {
        id: 0,
        canonical_key: "skf.com".to_string(),
        company_name: "SKF Group".to_string(),
        normalized_name: "skf".to_string(),
        website: Some("https://www.skf.com".to_string()),
        domain: Some("skf.com".to_string()),
        country: Some("Sweden".to_string()),
        city: Some("Gothenburg".to_string()),
        email: Some("sales@skf.com".to_string()),
        phone: Some("+46 31 337 1000".to_string()),
        category: Some("Bearings".to_string()),
        description: Some("Industrial bearings manufacturer".to_string()),
        verification_status: "verified".to_string(),
        created_at: String::new(),
        updated_at: String::new(),
    };

    let supplier_id = suppliers::upsert_supplier(&pool, &supplier).await.unwrap();

    searches::save_search_result(
        &pool,
        job_id,
        supplier_id,
        95.0,
        Some(r#"{"product_match_score":40.0,"location_score":20.0,"company_profile_score":15.0,"contact_score":10.0,"price_score":10.0,"total_score":95.0,"notes":["Verified supplier"]}"#),
    )
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

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/search/{}/export", job_id))
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
    // Excel xlsx files are zip archives starting with PK (0x50, 0x4B)
    assert!(body_bytes.len() > 100);
    assert_eq!(&body_bytes[0..2], b"PK");

    // Clean up
    let _ = tokio::fs::remove_file(test_db).await;
}
