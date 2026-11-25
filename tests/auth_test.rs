use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use procure_ai::api::app_router;
use procure_ai::config::Config;
use procure_ai::database::init_pool;
use procure_ai::state::AppState;
use tower::ServiceExt;

#[tokio::test]
async fn test_auth_protection_and_login_v2() {
    let test_db_path = "instance/test_auth.db";
    let _ = tokio::fs::remove_file(test_db_path).await;

    let mut config = Config::from_env();
    config.database_url = format!("sqlite://{}", test_db_path);
    config.session_secret = "test-secret-key-32-chars-long-procure-ai".to_string();

    let pool = init_pool(&config.database_url)
        .await
        .expect("Failed to initialize test pool");

    let state = AppState::new(config, pool);
    let app = app_router(state);

    // 1. Unauthenticated request to /api/v1/suppliers should be 401 Unauthorized
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/suppliers")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // 2. Unauthenticated request to /api/v1/search-jobs should be 401 Unauthorized
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/search-jobs")
                .method("POST")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"input_text": "test bearings SKF 6205"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    // 3. Login with default admin credentials
    let login_body = serde_json::json!({
        "username": "admin",
        "password": "AdminProcureAI2026!"
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/login")
                .method("POST")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&login_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let login_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    let token = login_json["token"]
        .as_str()
        .expect("Token should be in login response")
        .to_string();

    // 4. Authenticated request to /api/v1/auth/me
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/me")
                .method("GET")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let me_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(me_json["user"]["username"], "admin");
    assert_eq!(me_json["user"]["role"], "admin");

    // 5. Authenticated request to /api/v1/suppliers
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/suppliers")
                .method("GET")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // 6. Authenticated request to create search job via POST /api/v1/search-jobs
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/search-jobs")
                .method("POST")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"input_text": "SKF 6205 bearing supplier", "target_regions": ["Indonesia"]}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let job_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert!(job_json["job_id"].as_str().unwrap().len() > 20); // UUID
    assert_eq!(job_json["current_stage"], "understand");

    // Clean up
    let _ = tokio::fs::remove_file(test_db_path).await;
}
