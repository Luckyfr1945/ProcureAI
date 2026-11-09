use crate::models::Supplier;

pub fn are_duplicate_suppliers(a: &Supplier, b: &Supplier) -> bool {
    // 1. Strict domain match
    if let (Some(da), Some(db)) = (&a.domain, &b.domain) {
        if !da.is_empty() && !db.is_empty() && da.eq_ignore_ascii_case(db) {
            return true;
        }
    }

    // 2. Exact canonical key match
    if !a.canonical_key.is_empty() && a.canonical_key == b.canonical_key {
        return true;
    }

    // 3. Exact normalized company name + country match
    if !a.normalized_name.is_empty() && a.normalized_name == b.normalized_name {
        if let (Some(ca), Some(cb)) = (&a.country, &b.country) {
            if ca.eq_ignore_ascii_case(cb) {
                return true;
            }
        }
    }

    // Conservative: do not merge just because names sound similar!
    false
}
