PRAGMA foreign_keys = ON;

CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'purchasing',
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE search_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id INTEGER NOT NULL,
    original_request TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued',
    options_json TEXT,
    stop_requested INTEGER NOT NULL DEFAULT 0,
    round_count INTEGER NOT NULL DEFAULT 0,
    discovered_url_count INTEGER NOT NULL DEFAULT 0,
    processed_url_count INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    started_at TEXT,
    finished_at TEXT,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE TABLE search_queries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id INTEGER NOT NULL,
    query_text TEXT NOT NULL,
    provider TEXT NOT NULL,
    page_number INTEGER NOT NULL DEFAULT 1,
    result_count INTEGER NOT NULL DEFAULT 0,
    exhausted INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (job_id) REFERENCES search_jobs(id) ON DELETE CASCADE,
    UNIQUE(job_id, provider, query_text, page_number)
);

CREATE TABLE crawl_pages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id INTEGER NOT NULL,
    url TEXT NOT NULL,
    normalized_url TEXT NOT NULL,
    domain TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'discovered',
    http_status INTEGER,
    page_title TEXT,
    content_hash TEXT,
    error_message TEXT,
    discovered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    fetched_at TEXT,
    FOREIGN KEY (job_id) REFERENCES search_jobs(id) ON DELETE CASCADE,
    UNIQUE(job_id, normalized_url)
);

CREATE TABLE suppliers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    canonical_key TEXT NOT NULL UNIQUE,
    company_name TEXT NOT NULL,
    normalized_name TEXT NOT NULL,
    website TEXT,
    domain TEXT,
    country TEXT,
    city TEXT,
    email TEXT,
    phone TEXT,
    category TEXT,
    description TEXT,
    verification_status TEXT NOT NULL DEFAULT 'unverified',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE supplier_sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    supplier_id INTEGER NOT NULL,
    crawl_page_id INTEGER,
    source_url TEXT NOT NULL,
    source_type TEXT,
    evidence_json TEXT,
    extracted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (supplier_id) REFERENCES suppliers(id) ON DELETE CASCADE,
    FOREIGN KEY (crawl_page_id) REFERENCES crawl_pages(id) ON DELETE SET NULL,
    UNIQUE(supplier_id, source_url)
);

CREATE TABLE supplier_products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    supplier_id INTEGER NOT NULL,
    source_id INTEGER,
    product_name TEXT NOT NULL,
    brand TEXT,
    model_or_specification TEXT,
    price_amount REAL,
    price_currency TEXT,
    minimum_order_quantity TEXT,
    availability_note TEXT,
    product_url TEXT,
    price_observed_at TEXT,
    FOREIGN KEY (supplier_id) REFERENCES suppliers(id) ON DELETE CASCADE,
    FOREIGN KEY (source_id) REFERENCES supplier_sources(id) ON DELETE SET NULL
);

CREATE TABLE search_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id INTEGER NOT NULL,
    supplier_id INTEGER NOT NULL,
    relevance_score REAL NOT NULL DEFAULT 0,
    score_breakdown_json TEXT,
    first_seen_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (job_id) REFERENCES search_jobs(id) ON DELETE CASCADE,
    FOREIGN KEY (supplier_id) REFERENCES suppliers(id) ON DELETE CASCADE,
    UNIQUE(job_id, supplier_id)
);

CREATE INDEX idx_jobs_user_created ON search_jobs(user_id, created_at);
CREATE INDEX idx_jobs_status ON search_jobs(status);
CREATE INDEX idx_pages_job_status ON crawl_pages(job_id, status);
CREATE INDEX idx_pages_domain ON crawl_pages(domain);
CREATE INDEX idx_suppliers_name ON suppliers(normalized_name);
CREATE INDEX idx_suppliers_country ON suppliers(country);
CREATE INDEX idx_results_job_score ON search_results(job_id, relevance_score DESC);
CREATE INDEX idx_products_supplier ON supplier_products(supplier_id);

