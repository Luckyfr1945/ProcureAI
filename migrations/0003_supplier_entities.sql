CREATE TABLE IF NOT EXISTS suppliers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    canonical_name TEXT NOT NULL,
    normalized_name TEXT NOT NULL,
    website TEXT,
    normalized_domain TEXT,
    country_code TEXT,
    country_name TEXT,
    address TEXT,
    supplier_type TEXT DEFAULT 'unknown',
    business_category TEXT,
    public_email TEXT,
    public_phone TEXT,
    contact_page_url TEXT,
    status TEXT NOT NULL DEFAULT 'discovered',
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS supplier_sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    supplier_id INTEGER NOT NULL REFERENCES suppliers(id) ON DELETE CASCADE,
    url TEXT NOT NULL,
    canonical_url TEXT NOT NULL,
    page_type TEXT DEFAULT 'company',
    page_title TEXT,
    content_hash TEXT,
    retrieved_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    http_status INTEGER DEFAULT 200,
    source_type TEXT DEFAULT 'web',
    UNIQUE(supplier_id, canonical_url)
);

CREATE TABLE IF NOT EXISTS products (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    canonical_name TEXT NOT NULL,
    brand TEXT,
    model TEXT,
    part_number TEXT,
    category_key TEXT,
    attributes_json TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS supplier_offers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    supplier_id INTEGER NOT NULL REFERENCES suppliers(id) ON DELETE CASCADE,
    product_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    source_id INTEGER REFERENCES supplier_sources(id) ON DELETE SET NULL,
    product_url TEXT,
    description TEXT,
    attributes_json TEXT,
    price_amount NUMERIC,
    currency TEXT,
    price_basis TEXT,
    moq TEXT,
    lead_time TEXT,
    availability_text TEXT,
    product_fit_score REAL NOT NULL DEFAULT 0.0,
    evidence_completeness_score REAL NOT NULL DEFAULT 0.0,
    extracted_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_suppliers_domain ON suppliers(normalized_domain);
CREATE INDEX IF NOT EXISTS idx_suppliers_name ON suppliers(normalized_name);
CREATE INDEX IF NOT EXISTS idx_products_part_num ON products(part_number);
CREATE INDEX IF NOT EXISTS idx_products_cat ON products(category_key);
CREATE INDEX IF NOT EXISTS idx_offers_supplier ON supplier_offers(supplier_id);
CREATE INDEX IF NOT EXISTS idx_offers_product ON supplier_offers(product_id);
