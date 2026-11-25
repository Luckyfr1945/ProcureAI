use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use procure_ai::{
    api::app_router,
    auth::session::create_session_token,
    config::Config,
    database::repositories::{products, search_jobs, users},
    domain::{Product, ProductRequirement},
    models::UserRole,
    services::{EntityResolution, RankingService, gemini::GeminiClient},
    state::AppState,
};
use tower::ServiceExt;
async fn setup() -> (AppState, i64, String, String) {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    let owner = users::create_user(&pool, "owner", "hash", UserRole::Purchasing)
        .await
        .unwrap();
    let other = users::create_user(&pool, "other", "hash", UserRole::Purchasing)
        .await
        .unwrap();
    let config = Config::from_env();
    let token =
        create_session_token(other, "other", "purchasing", &config.session_secret, 3600).unwrap();
    let job = search_jobs::create_job(&pool, owner, "private request")
        .await
        .unwrap();
    (AppState::new(config, pool), other, token, job)
}
#[tokio::test]
async fn foreign_jobs_are_forbidden() {
    let (state, _, token, job) = setup().await;
    let app = app_router(state.clone());
    for (method, path) in [
        ("GET", format!("/api/v1/search-jobs/{job}")),
        ("POST", format!("/api/v1/search-jobs/{job}/stop")),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
    assert_eq!(
        search_jobs::get_job(&state.db, &job)
            .await
            .unwrap()
            .unwrap()
            .status,
        "queued"
    );
}
#[tokio::test]
async fn disabled_accounts_are_rejected() {
    let (state, other, token, _) = setup().await;
    sqlx::query("UPDATE users SET is_active=0 WHERE id=?")
        .bind(other)
        .execute(&state.db)
        .await
        .unwrap();
    let app = app_router(state);
    let logout = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/logout")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logout.status(), StatusCode::OK);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/auth/me")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
#[tokio::test]
async fn part_numbers_do_not_merge_brands() {
    let (state, _, _, _) = setup().await;
    let mut product = Product {
        id: 0,
        canonical_name: "SKF 6205".into(),
        brand: Some("SKF".into()),
        model: None,
        part_number: Some("6205".into()),
        category_key: None,
        attributes_json: None,
        created_at: String::new(),
        updated_at: String::new(),
    };
    let first = products::upsert_product(&state.db, &product).await.unwrap();
    product.canonical_name = "NSK 6205".into();
    product.brand = Some("NSK".into());
    let second = products::upsert_product(&state.db, &product).await.unwrap();
    assert_ne!(first, second);
    assert_eq!(
        products::get_product_by_id(&state.db, second)
            .await
            .unwrap()
            .unwrap()
            .brand
            .as_deref(),
        Some("NSK")
    );
}
#[test]
fn unrelated_page_cannot_become_requested_product() {
    let req:ProductRequirement=serde_json::from_value(serde_json::json!({"product_name":"SKF bearing 6205", "category":"bearing", "brand":"SKF", "part_number":"6205", "target_regions":["Indonesia"]})).unwrap();
    let client = GeminiClient::new(String::new(), "unused".into());
    let payload = client.fallback_extract_supplier(
        "https://example.co.id",
        "PT Office Paper",
        "Kami menjual kertas kantor. Alamat: Jakarta, Indonesia.",
        &req,
    );
    assert!(payload.offers.is_empty());
    let supplier = EntityResolution::normalize_supplier(
        Some("Office Paper"),
        Some("https://example.co.id"),
        Some("Indonesia"),
        None,
        None,
        Some("distributor"),
        Some("bearing"),
    );
    let score=RankingService::calculate_scores(&supplier,None,None,&serde_json::from_value(serde_json::json!({"product_name":"valve", "category":"valve", "target_regions":["Indonesia"]})).unwrap(),&[]);
    assert_eq!(score.product_fit_score, 0.0);
    println!(
        "No product proof still receives {} fit points",
        score.product_fit_score
    );
}
#[tokio::test]
async fn verification_rejects_unknown_supplier_and_status() {
    let (state, _, token, _) = setup().await;
    let response = app_router(state)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/suppliers/999999/verify")
                .header("Authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"verification_status":"anything"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn logout_revokes_bearer_and_cookie_token() {
    let (state, _, token, _) = setup().await;
    let app = app_router(state);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/logout")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    for header in ["Authorization", "Cookie"] {
        let value = if header == "Cookie" {
            format!("session={token}")
        } else {
            format!("Bearer {token}")
        };
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/auth/me")
                    .header(header, value)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}
#[test]
fn private_ipv4_mapped_ipv6_is_blocked() {
    use procure_ai::utils::url_safety::is_private_ip;
    assert!(is_private_ip(&"::ffff:127.0.0.1".parse().unwrap()));
    assert!(is_private_ip(&"::ffff:192.168.1.2".parse().unwrap()));
    assert!(!is_private_ip(&"2606:4700:4700::1111".parse().unwrap()));
}

#[tokio::test]
async fn every_job_endpoint_checks_ownership() {
    let (state, _, token, job) = setup().await;
    let app = app_router(state);
    for suffix in [
        "",
        "/events",
        "/queries",
        "/results",
        "/export.xlsx",
        "/purchasing",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/search-jobs/{job}{suffix}"))
                    .header("Authorization", format!("Bearer {token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{suffix}");
    }
}
#[tokio::test]
async fn verification_is_atomic_and_rejects_another_suppliers_evidence() {
    let (state, _, token, _) = setup().await;
    let first = sqlx::query(
        "INSERT INTO suppliers(canonical_name,normalized_name) VALUES('First','first')",
    )
    .execute(&state.db)
    .await
    .unwrap()
    .last_insert_rowid();
    let second = sqlx::query(
        "INSERT INTO suppliers(canonical_name,normalized_name) VALUES('Second','second')",
    )
    .execute(&state.db)
    .await
    .unwrap()
    .last_insert_rowid();
    let source=sqlx::query("INSERT INTO supplier_sources(supplier_id,url,canonical_url) VALUES(?, 'https://example.org','https://example.org')").bind(second).execute(&state.db).await.unwrap().last_insert_rowid();
    let evidence=sqlx::query("INSERT INTO evidence_items(supplier_id,source_id,field_path,evidence_quote) VALUES(?,?,'offer.price','100')").bind(second).bind(source).execute(&state.db).await.unwrap().last_insert_rowid();
    let app = app_router(state.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/suppliers/{first}/verify"))
                .header("Authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"evidence_ids":[evidence]}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let status: String = sqlx::query_scalar("SELECT status FROM suppliers WHERE id=?")
        .bind(first)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(status, "discovered");
    let status: String =
        sqlx::query_scalar("SELECT verification_status FROM evidence_items WHERE id=?")
            .bind(evidence)
            .fetch_one(&state.db)
            .await
            .unwrap();
    assert_eq!(status, "unverified");
}
#[tokio::test]
async fn directory_finds_supplier_beyond_first_hundred() {
    use http_body_util::BodyExt;
    let (state, _, token, _) = setup().await;
    sqlx::query("INSERT INTO suppliers(canonical_name,normalized_name) VALUES('Target vendor','target vendor')").execute(&state.db).await.unwrap();
    for i in 0..110 {
        sqlx::query("INSERT INTO suppliers(canonical_name,normalized_name) VALUES(?,?)")
            .bind(format!("Other {i}"))
            .bind(format!("other {i}"))
            .execute(&state.db)
            .await
            .unwrap();
    }
    let response = app_router(state)
        .oneshot(
            Request::builder()
                .uri("/api/v1/suppliers?limit=51&q=Target")
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let data: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(data.as_array().unwrap().len(), 1);
    assert_eq!(data[0]["canonical_name"], "Target vendor");
}
#[test]
fn country_table_covers_custom_countries() {
    use procure_ai::services::location::*;
    assert_eq!(country_code_from_name("Oman"), Some("OM"));
    assert_eq!(country_code_from_name("BR"), Some("BR"));
    assert_eq!(canonical_country_name("OM"), "Oman");
    assert!(supplier_matches_targets(
        Some("OM"),
        Some("Oman"),
        Some("Muscat"),
        &["Oman".into()]
    ));
    assert!(!supplier_matches_targets(
        Some("CN"),
        Some("China"),
        Some("Shenzhen"),
        &["Oman".into()]
    ));
    assert_ne!(country_code_from_name("Indianapolis"), Some("IN"));
}

#[tokio::test]
async fn invalid_profile_correction_is_not_partially_saved() {
    let (state, _, token, _) = setup().await;
    let id = sqlx::query(
        "INSERT INTO suppliers(canonical_name,normalized_name) VALUES('Original','original')",
    )
    .execute(&state.db)
    .await
    .unwrap()
    .last_insert_rowid();
    let response = app_router(state.clone())
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/v1/suppliers/{id}"))
                .header("Authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"canonical_name":"Changed","country_code":"NOT-A-COUNTRY"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let name: String = sqlx::query_scalar("SELECT canonical_name FROM suppliers WHERE id=?")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .unwrap();
    assert_eq!(name, "Original");
}
#[tokio::test]
async fn legacy_offers_are_preserved_but_not_reused() {
    let (state, _, _, _) = setup().await;
    let supplier = sqlx::query(
        "INSERT INTO suppliers(canonical_name,normalized_name) VALUES('Vendor','vendor')",
    )
    .execute(&state.db)
    .await
    .unwrap()
    .last_insert_rowid();
    let product = sqlx::query("INSERT INTO products(canonical_name) VALUES('Unproven bearing')")
        .execute(&state.db)
        .await
        .unwrap()
        .last_insert_rowid();
    sqlx::query("INSERT INTO supplier_offers(supplier_id,product_id) VALUES(?,?)")
        .bind(supplier)
        .bind(product)
        .execute(&state.db)
        .await
        .unwrap();
    assert!(
        products::get_supplier_offers(&state.db, supplier)
            .await
            .unwrap()
            .is_empty()
    );
    let preserved: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM supplier_offers WHERE supplier_id=? AND needs_review=1",
    )
    .bind(supplier)
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert_eq!(preserved, 1);
}
#[test]
fn part_number_evidence_accepts_formatting_without_accepting_other_models() {
    let req:ProductRequirement=serde_json::from_value(serde_json::json!({"product_name":"bearing", "category":"bearing", "part_number":"6205-2RS"})).unwrap();
    let client = GeminiClient::new(String::new(), "unused".into());
    let matched = client.fallback_extract_supplier(
        "https://example.org",
        "Bearings",
        "We supply bearing 6205 2RS. Contact sales@example.org.",
        &req,
    );
    assert_eq!(matched.offers.len(), 1);
    assert!(
        matched
            .evidence_items
            .iter()
            .any(|e| e.field_path == "offer.part_number")
    );
    let other = client.fallback_extract_supplier(
        "https://example.org",
        "Bearings",
        "We supply bearing 16205 2RS.",
        &req,
    );
    assert!(other.offers.is_empty());
}

#[tokio::test]
async fn repeated_login_attempts_are_rate_limited() {
 let(state,_,_,_)=setup().await; let app=app_router(state);
 for i in 0..6 {
  let response=app.clone().oneshot(Request::builder().method("POST").uri("/api/v1/auth/login").header("content-type","application/json").body(Body::from(r#"{"username":"missing","password":"wrong"}"#)).unwrap()).await.unwrap();
  assert_eq!(response.status(),if i<5 {StatusCode::UNAUTHORIZED}else{StatusCode::TOO_MANY_REQUESTS});
 }
}
#[tokio::test]
async fn cross_origin_mutations_are_rejected() {
 let(state,_,token,_)=setup().await;
 let response=app_router(state).oneshot(Request::builder().method("POST").uri("/api/v1/auth/logout").header("Host","procure.internal").header("Origin","https://other.example").header("Cookie",format!("session={token}")).body(Body::empty()).unwrap()).await.unwrap();
 assert_eq!(response.status(),StatusCode::FORBIDDEN);
}
