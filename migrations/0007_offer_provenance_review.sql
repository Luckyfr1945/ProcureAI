-- Preserve existing data, but require a new evidence-backed extraction before reuse.
ALTER TABLE supplier_offers ADD COLUMN needs_review INTEGER NOT NULL DEFAULT 1;
CREATE INDEX idx_offer_review ON supplier_offers(supplier_id, needs_review);
CREATE TABLE supplier_profile_corrections (
 supplier_id INTEGER PRIMARY KEY REFERENCES suppliers(id) ON DELETE CASCADE,
 corrected_by INTEGER NOT NULL REFERENCES users(id),
 corrected_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
