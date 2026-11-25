CREATE TABLE purchasing_reviews (
    job_id TEXT NOT NULL,
    supplier_id INTEGER NOT NULL,
    data_json TEXT NOT NULL,
    updated_by INTEGER NOT NULL REFERENCES users(id),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (job_id, supplier_id),
    FOREIGN KEY (job_id, supplier_id) REFERENCES job_supplier_results(job_id, supplier_id)
);
CREATE TABLE purchasing_review_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id TEXT NOT NULL,
    supplier_id INTEGER NOT NULL,
    data_json TEXT NOT NULL,
    changed_by INTEGER NOT NULL REFERENCES users(id),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (job_id, supplier_id) REFERENCES job_supplier_results(job_id, supplier_id)
);
