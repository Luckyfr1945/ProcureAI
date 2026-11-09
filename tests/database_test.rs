use procure_ai::database::init_pool;
use procure_ai::database::repositories::{searches, suppliers, users};
use procure_ai::models::{Supplier, UserRole};

#[tokio::test]
async fn test_database_initialization_and_crud() {
    let test_db_path = "instance/test_procure_ai.db";
    let _ = tokio::fs::remove_file(test_db_path).await;

    let pool = init_pool(&format!("sqlite://{}", test_db_path))
        .await
        .expect("Failed to initialize test pool and run migrations");

    // 1. Verify user creation and retrieval
    let user_id = users::create_user(&pool, "test_admin", "hash123", UserRole::Admin)
        .await
        .expect("Failed to create user");
    assert!(user_id > 0);

    let user = users::get_user_by_username(&pool, "test_admin")
        .await
        .expect("Failed to get user")
        .expect("User not found");
    assert_eq!(user.username, "test_admin");
    assert_eq!(user.role, "admin");

    // 2. Verify job creation and retrieval
    let job_id = searches::create_job(&pool, user_id, "Test bearing SKF", None)
        .await
        .expect("Failed to create job");
    assert!(job_id > 0);

    let job = searches::get_job(&pool, job_id)
        .await
        .expect("Failed to get job")
        .expect("Job not found");
    assert_eq!(job.original_request, "Test bearing SKF");
    assert_eq!(job.status, "queued");

    // 3. Verify supplier creation and retrieval
    let supplier = Supplier {
        id: 0,
        canonical_key: "skf.com".to_string(),
        company_name: "SKF Group".to_string(),
        normalized_name: "skf group".to_string(),
        website: Some("https://www.skf.com".to_string()),
        domain: Some("skf.com".to_string()),
        country: Some("Sweden".to_string()),
        city: Some("Gothenburg".to_string()),
        email: Some("info@skf.com".to_string()),
        phone: None,
        category: Some("Industrial Bearings".to_string()),
        description: Some("Global manufacturer of bearings and seals".to_string()),
        verification_status: "unverified".to_string(),
        created_at: String::new(),
        updated_at: String::new(),
    };

    let supplier_id = suppliers::upsert_supplier(&pool, &supplier)
        .await
        .expect("Failed to upsert supplier");
    assert!(supplier_id > 0);

    let fetched_supplier = suppliers::get_supplier_by_id(&pool, supplier_id)
        .await
        .expect("Failed to fetch supplier")
        .expect("Supplier not found");
    assert_eq!(fetched_supplier.canonical_key, "skf.com");

    // 4. Verify search result linkage
    searches::save_search_result(&pool, job_id, supplier_id, 92.5, Some(r#"{"total_score":92.5}"#))
        .await
        .expect("Failed to save search result");

    let results = searches::get_job_results(&pool, job_id)
        .await
        .expect("Failed to get job results");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].supplier.company_name, "SKF Group");
    assert_eq!(results[0].relevance_score, 92.5);

    // Clean up test database
    let _ = tokio::fs::remove_file(test_db_path).await;
}

