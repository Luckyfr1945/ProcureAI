use axum::{body::Body, http::{Request, StatusCode}};
use procure_ai::{api::app_router, auth::session::create_session_token, config::Config, state::AppState, database::repositories::{users,search_jobs,products}, models::UserRole, domain::{Product,ProductRequirement}, services::{gemini::GeminiClient, RankingService, EntityResolution, stored_search}};
use tower::ServiceExt;
async fn setup() -> (AppState, i64, String, String) {
 let pool=sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
 sqlx::migrate!("./migrations").run(&pool).await.unwrap();
 let owner=users::create_user(&pool,"owner","hash",UserRole::Purchasing).await.unwrap();
 let other=users::create_user(&pool,"other","hash",UserRole::Purchasing).await.unwrap();
 let config=Config::from_env();
 let token=create_session_token(other,"other","purchasing",&config.session_secret,3600).unwrap();
 let job=search_jobs::create_job(&pool,owner,"private request").await.unwrap();
 (AppState::new(config,pool),other,token,job)
}
#[tokio::test]
async fn audit_foreign_job_read_and_stop_are_accepted() {
 let(state,_,token,job)=setup().await;
 let app=app_router(state.clone());
 for (method,path) in [("GET",format!("/api/v1/search-jobs/{job}")),("POST",format!("/api/v1/search-jobs/{job}/stop"))] {
  let response=app.clone().oneshot(Request::builder().method(method).uri(path).header("Authorization",format!("Bearer {token}")).body(Body::empty()).unwrap()).await.unwrap();
  assert_eq!(response.status(),StatusCode::OK);
 }
 assert_eq!(search_jobs::get_job(&state.db,&job).await.unwrap().unwrap().status,"stopping");
}
#[tokio::test]
async fn audit_disabled_account_and_logged_out_token_still_work() {
 let(state,other,token,_)=setup().await;
 sqlx::query("UPDATE users SET is_active=0 WHERE id=?").bind(other).execute(&state.db).await.unwrap();
 let app=app_router(state);
 let logout=app.clone().oneshot(Request::builder().method("POST").uri("/api/v1/auth/logout").body(Body::empty()).unwrap()).await.unwrap();
 assert_eq!(logout.status(),StatusCode::OK);
 let response=app.oneshot(Request::builder().uri("/api/v1/auth/me").header("Authorization",format!("Bearer {token}")).body(Body::empty()).unwrap()).await.unwrap();
 assert_eq!(response.status(),StatusCode::OK);
}
#[tokio::test]
async fn audit_part_number_merges_different_brands() {
 let(state,_,_,_)=setup().await;
 let mut product=Product { id:0, canonical_name:"SKF 6205".into(),brand:Some("SKF".into()),model:None,part_number:Some("6205".into()),category_key:None,attributes_json:None,created_at:String::new(),updated_at:String::new() };
 let first=products::upsert_product(&state.db,&product).await.unwrap();
 product.canonical_name="NSK 6205".into(); product.brand=Some("NSK".into());
 let second=products::upsert_product(&state.db,&product).await.unwrap();
 assert_eq!(first,second);
 assert_eq!(products::get_product_by_id(&state.db,second).await.unwrap().unwrap().brand.as_deref(),Some("SKF"));
}
#[test]
fn audit_unrelated_page_becomes_requested_product() {
 let req:ProductRequirement=serde_json::from_value(serde_json::json!({"product_name":"SKF bearing 6205", "category":"bearing", "brand":"SKF", "part_number":"6205", "target_regions":["Indonesia"]})).unwrap();
 let client=GeminiClient::new(String::new(),"unused".into());
 let payload=client.fallback_extract_supplier("https://example.co.id","PT Office Paper","Kami menjual kertas kantor. Alamat: Jakarta, Indonesia.",&req);
 assert_eq!(payload.offers[0].part_number.as_deref(),Some("6205"));
 assert_eq!(payload.offers[0].brand.as_deref(),Some("SKF"));
 let product=Product {id:0,canonical_name:payload.offers[0].product_name.clone(),brand:payload.offers[0].brand.clone(),part_number:payload.offers[0].part_number.clone(),model:None,category_key:None,attributes_json:None,created_at:String::new(),updated_at:String::new()};
 assert!(stored_search::matches_product(&req,&product,None));
 let supplier=EntityResolution::normalize_supplier(Some("Office Paper"),Some("https://example.co.id"),Some("Indonesia"),None,None,Some("distributor"),Some("bearing"));
 let score=RankingService::calculate_scores(&supplier,None,None,&serde_json::from_value(serde_json::json!({"product_name":"valve", "category":"valve", "target_regions":["Indonesia"]})).unwrap(),&[]);
 assert!(score.product_fit_score>=50.0);
 println!("No product proof still receives {} fit points",score.product_fit_score);
}
#[tokio::test]
async fn audit_verify_nonexistent_supplier_returns_success() {
 let(state,_,token,_)=setup().await;
 let response=app_router(state).oneshot(Request::builder().method("POST").uri("/api/v1/suppliers/999999/verify").header("Authorization",format!("Bearer {token}")).header("content-type","application/json").body(Body::from(r#"{"verification_status":"anything"}"#)).unwrap()).await.unwrap();
 assert_eq!(response.status(),StatusCode::OK);
}
