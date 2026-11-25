CREATE TABLE IF NOT EXISTS evidence_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    supplier_id INTEGER REFERENCES suppliers(id) ON DELETE CASCADE,
    offer_id INTEGER REFERENCES supplier_offers(id) ON DELETE CASCADE,
    field_path TEXT NOT NULL,
    field_value_json TEXT,
    source_id INTEGER NOT NULL REFERENCES supplier_sources(id) ON DELETE CASCADE,
    evidence_quote TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 1.0,
    verification_status TEXT NOT NULL DEFAULT 'unverified',
    verified_by_user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
    verified_at DATETIME
);

CREATE TABLE IF NOT EXISTS job_supplier_results (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id TEXT NOT NULL REFERENCES search_jobs(id) ON DELETE CASCADE,
    supplier_id INTEGER NOT NULL REFERENCES suppliers(id) ON DELETE CASCADE,
    product_fit_score REAL NOT NULL DEFAULT 0.0,
    evidence_completeness_score REAL NOT NULL DEFAULT 0.0,
    rank_position INTEGER DEFAULT 1,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(job_id, supplier_id)
);

CREATE INDEX IF NOT EXISTS idx_evidence_supplier ON evidence_items(supplier_id);
CREATE INDEX IF NOT EXISTS idx_evidence_offer ON evidence_items(offer_id);
CREATE INDEX IF NOT EXISTS idx_job_supplier_rank ON job_supplier_results(job_id, rank_position);
