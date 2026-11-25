CREATE TABLE IF NOT EXISTS search_jobs (
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    input_text TEXT NOT NULL,
    requirement_json TEXT,
    status TEXT NOT NULL DEFAULT 'queued',
    current_stage TEXT DEFAULT 'understand',
    round_count INTEGER NOT NULL DEFAULT 0,
    new_supplier_count INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    started_at DATETIME,
    finished_at DATETIME,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS search_queries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id TEXT NOT NULL REFERENCES search_jobs(id) ON DELETE CASCADE,
    query_text TEXT NOT NULL,
    query_type TEXT DEFAULT 'general',
    language TEXT DEFAULT 'en',
    provider TEXT DEFAULT 'live',
    page_or_offset INTEGER NOT NULL DEFAULT 0,
    result_count INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS search_job_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id TEXT NOT NULL REFERENCES search_jobs(id) ON DELETE CASCADE,
    stage TEXT NOT NULL,
    message TEXT,
    metadata_json TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_jobs_user_status ON search_jobs(user_id, status);
CREATE INDEX IF NOT EXISTS idx_queries_job ON search_queries(job_id);
CREATE INDEX IF NOT EXISTS idx_events_job ON search_job_events(job_id);
