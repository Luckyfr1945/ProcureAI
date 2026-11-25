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
    pub search_api_base_url: String,
    pub search_api_key: String,
    pub brave_search_api_key: String,
    pub tavily_api_key: String,
    pub session_secret: String,
    pub max_job_duration_seconds: u64,
    pub max_search_requests_per_job: u32,
    pub max_crawl_pages_per_job: u32,
    pub max_concurrent_fetches: usize,
    pub request_timeout_seconds: u64,
    pub max_page_bytes: usize,
    pub crawler_request_delay_ms: u64,
}

impl Config {
    pub fn from_env() -> Self {
        dotenvy::dotenv().ok();

        let max_duration = env::var("MAX_JOB_DURATION_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .or_else(|| {
                env::var("MAX_JOB_DURATION_MINUTES")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .map(|m| m * 60)
            })
            .unwrap_or(900);

        let max_search_req = env::var("MAX_SEARCH_REQUESTS_PER_JOB")
            .ok()
            .and_then(|v| v.parse().ok())
            .or_else(|| {
                env::var("MAX_SEARCH_ROUNDS")
                    .ok()
                    .and_then(|v| v.parse().ok())
            })
            .unwrap_or(40);

        let max_crawl_pages = env::var("MAX_CRAWL_PAGES_PER_JOB")
            .ok()
            .and_then(|v| v.parse().ok())
            .or_else(|| {
                env::var("MAX_URLS_PER_JOB")
                    .ok()
                    .and_then(|v| v.parse().ok())
            })
            .unwrap_or(80);

        let max_concurrency = env::var("MAX_CONCURRENT_FETCHES")
            .ok()
            .and_then(|v| v.parse().ok())
            .or_else(|| {
                env::var("CRAWLER_GLOBAL_CONCURRENCY")
                    .ok()
                    .and_then(|v| v.parse().ok())
            })
            .unwrap_or(5);

        let timeout = env::var("REQUEST_TIMEOUT_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .or_else(|| {
                env::var("CRAWLER_TIMEOUT_SECONDS")
                    .ok()
                    .and_then(|v| v.parse().ok())
            })
            .unwrap_or(20);

        let max_bytes = env::var("MAX_PAGE_BYTES")
            .ok()
            .and_then(|v| v.parse().ok())
            .or_else(|| {
                env::var("CRAWLER_MAX_RESPONSE_BYTES")
                    .ok()
                    .and_then(|v| v.parse().ok())
            })
            .unwrap_or(2_000_000);

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
            search_provider: env::var("SEARCH_PROVIDER").unwrap_or_else(|_| "live".to_string()),
            search_api_base_url: env::var("SEARCH_API_BASE_URL").unwrap_or_default(),
            search_api_key: env::var("SEARCH_API_KEY").unwrap_or_default(),
            brave_search_api_key: env::var("BRAVE_SEARCH_API_KEY")
                .or_else(|_| env::var("SEARCH_API_KEY"))
                .unwrap_or_default(),
            tavily_api_key: env::var("TAVILY_API_KEY").unwrap_or_default(),
            session_secret: env::var("SESSION_SECRET").unwrap_or_else(|_| {
                static DEVELOPMENT_SECRET: std::sync::OnceLock<String> = std::sync::OnceLock::new();
                DEVELOPMENT_SECRET
                    .get_or_init(|| format!("{}{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4()))
                    .clone()
            }),
            max_job_duration_seconds: max_duration,
            max_search_requests_per_job: max_search_req,
            max_crawl_pages_per_job: max_crawl_pages,
            max_concurrent_fetches: max_concurrency,
            request_timeout_seconds: timeout,
            max_page_bytes: max_bytes,
            crawler_request_delay_ms: env::var("CRAWLER_REQUEST_DELAY_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000),
        }
    }
}

impl Config {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.app_env == "production" {
            let secret = std::env::var("SESSION_SECRET").unwrap_or_default();
            anyhow::ensure!(
                secret.len() >= 32
                    && !secret.contains("insecure-default")
                    && !secret.contains("change-me"),
                "Production requires a unique SESSION_SECRET of at least 32 characters"
            );
        }
        anyhow::ensure!(
            self.max_job_duration_seconds > 0
                && self.max_search_requests_per_job > 0
                && self.max_crawl_pages_per_job > 0
                && self.max_concurrent_fetches > 0,
            "Search limits must be greater than zero"
        );
        Ok(())
    }
}
