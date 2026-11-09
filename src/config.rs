use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub app_host: String,
    pub app_port: u16,
    pub app_env: String,
    pub database_url: String,
    pub gemini_api_key: String,
    pub gemini_model: String,
    pub search_provider: String,
    pub search_api_key: String,
    pub search_api_base_url: String,
    pub session_secret: String,
    pub max_search_rounds: u32,
    pub max_pages_per_query: u32,
    pub max_urls_per_job: u32,
    pub max_job_duration_minutes: u32,
    pub crawler_global_concurrency: usize,
    pub crawler_per_domain_concurrency: usize,
    pub crawler_request_delay_ms: u64,
    pub crawler_timeout_seconds: u64,
    pub crawler_max_response_bytes: usize,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        Self {
            app_host: env::var("APP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            app_port: env::var("APP_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            app_env: env::var("APP_ENV").unwrap_or_else(|_| "development".to_string()),
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://instance/procure_ai.db".to_string()),
            gemini_api_key: env::var("GEMINI_API_KEY").unwrap_or_default(),
            gemini_model: env::var("GEMINI_MODEL")
                .unwrap_or_else(|_| "gemini-2.5-flash".to_string()),
            search_provider: env::var("SEARCH_PROVIDER").unwrap_or_else(|_| "mock".to_string()),
            search_api_key: env::var("SEARCH_API_KEY").unwrap_or_default(),
            search_api_base_url: env::var("SEARCH_API_BASE_URL").unwrap_or_default(),
            session_secret: env::var("SESSION_SECRET")
                .unwrap_or_else(|_| "insecure-default-change-me-in-production-12345".to_string()),
            max_search_rounds: env::var("MAX_SEARCH_ROUNDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20),
            max_pages_per_query: env::var("MAX_PAGES_PER_QUERY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10),
            max_urls_per_job: env::var("MAX_URLS_PER_JOB")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(500),
            max_job_duration_minutes: env::var("MAX_JOB_DURATION_MINUTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            crawler_global_concurrency: env::var("CRAWLER_GLOBAL_CONCURRENCY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5),
            crawler_per_domain_concurrency: env::var("CRAWLER_PER_DOMAIN_CONCURRENCY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1),
            crawler_request_delay_ms: env::var("CRAWLER_REQUEST_DELAY_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1500),
            crawler_timeout_seconds: env::var("CRAWLER_TIMEOUT_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(15),
            crawler_max_response_bytes: env::var("CRAWLER_MAX_RESPONSE_BYTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(2_000_000),
        }
    }
}

