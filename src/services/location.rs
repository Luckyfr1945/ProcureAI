use crate::domain::ProductRequirement;
use crate::services::gemini::{ExtractedSupplierPayload, PlannedQuery};
use crate::utils::url_safety::extract_domain;

const GLOBAL_LABELS: &[&str] = &[
    "worldwide",
    "world",
    "global",
    "semua negara",
    "seluruh dunia",
];

const ID_CITY_KEYWORDS: &[&str] = &[
    "surabaya",
    "gresik",
    "sidoarjo",
    "jakarta",
    "tangerang",
    "bekasi",
    "cikarang",
    "karawang",
    "bandung",
    "semarang",
    "kudus",
    "medan",
    "batam",
    "makassar",
    "yogyakarta",
    "jogja",
    "surakarta",
    "malang",
    "balikpapan",
    "samarinda",
    "palembang",
    "pekanbaru",
    "bogor",
    "depok",
    "cilegon",
    "serang",
    "denpasar",
    "bali",
    "jabodetabek",
    "jawa timur",
    "jawa barat",
    "jawa tengah",
    "banten",
    "sumatera",
    "kalimantan",
    "sulawesi",
    "kepulauan riau",
    "nusa tenggara",
];

const ID_CITY_CANONICAL: &[(&str, &str)] = &[
    ("surabaya", "Surabaya, Jawa Timur"),
    ("gresik", "Gresik, Jawa Timur"),
    ("sidoarjo", "Sidoarjo, Jawa Timur"),
    ("malang", "Malang, Jawa Timur"),
    ("jakarta", "Jakarta"),
    ("tangerang", "Tangerang, Banten"),
    ("cilegon", "Cilegon, Banten"),
    ("serang", "Serang, Banten"),
    ("bekasi", "Bekasi, Jawa Barat"),
    ("cikarang", "Cikarang, Jawa Barat"),
    ("karawang", "Karawang, Jawa Barat"),
    ("bogor", "Bogor, Jawa Barat"),
    ("depok", "Depok, Jawa Barat"),
    ("bandung", "Bandung, Jawa Barat"),
    ("semarang", "Semarang, Jawa Tengah"),
    ("kudus", "Kudus, Jawa Tengah"),
    ("surakarta", "Surakarta, Jawa Tengah"),
    ("yogyakarta", "Yogyakarta"),
    ("medan", "Medan, Sumatera Utara"),
    ("palembang", "Palembang, Sumatera Selatan"),
    ("pekanbaru", "Pekanbaru, Riau"),
    ("batam", "Batam, Kepulauan Riau"),
    ("balikpapan", "Balikpapan, Kalimantan Timur"),
    ("samarinda", "Samarinda, Kalimantan Timur"),
    ("makassar", "Makassar, Sulawesi Selatan"),
    ("denpasar", "Denpasar, Bali"),
    ("jabodetabek", "Jabodetabek"),
    ("jawa timur", "Jawa Timur"),
    ("jawa barat", "Jawa Barat"),
    ("jawa tengah", "Jawa Tengah"),
    ("banten", "Banten"),
    ("bali", "Bali"),
];

const CN_CITY_CANONICAL: &[(&str, &str)] = &[
    ("shenzhen", "Shenzhen, China"),
    ("guangzhou", "Guangzhou, China"),
    ("shanghai", "Shanghai, China"),
    ("dongguan", "Dongguan, China"),
    ("ningbo", "Ningbo, China"),
    ("suzhou", "Suzhou, China"),
    ("hangzhou", "Hangzhou, China"),
    ("wenzhou", "Wenzhou, China"),
    ("qingdao", "Qingdao, China"),
    ("beijing", "Beijing, China"),
    ("yiwu", "Yiwu, China"),
    ("foshan", "Foshan, China"),
    ("tianjin", "Tianjin, China"),
    ("xiamen", "Xiamen, China"),
];

pub fn is_global_label(raw: &str) -> bool {
    let l = raw.trim().to_lowercase();
    l.is_empty() || GLOBAL_LABELS.contains(&l.as_str())
}

pub fn contains_ascii_word(haystack_lower: &str, needle_lower: &str) -> bool {
    if needle_lower.is_empty() {
        return false;
    }
    let bytes = haystack_lower.as_bytes();
    let mut start = 0;
    while let Some(pos) = haystack_lower[start..].find(needle_lower) {
        let abs_start = start + pos;
        let abs_end = abs_start + needle_lower.len();
        let before_ok = abs_start == 0 || !bytes[abs_start - 1].is_ascii_alphanumeric();
        let after_ok = abs_end == bytes.len() || !bytes[abs_end].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        start = abs_start
            + haystack_lower[abs_start..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
    }
    false
}

pub fn country_code_from_name(raw: &str) -> Option<&'static str> {
    let l = raw.trim().to_lowercase();
    if l.is_empty() || is_global_label(&l) {
        return None;
    }
    if l == "id"
        || contains_ascii_word(&l, "indonesia")
        || ID_CITY_KEYWORDS.iter().any(|k| contains_ascii_word(&l, k))
    {
        return Some("ID");
    }
    if l == "cn"
        || l == "prc"
        || contains_ascii_word(&l, "china")
        || contains_ascii_word(&l, "tiongkok")
        || CN_CITY_CANONICAL
            .iter()
            .any(|(k, _)| contains_ascii_word(&l, k))
    {
        return Some("CN");
    }
    if l == "sg" || contains_ascii_word(&l, "singapore") || contains_ascii_word(&l, "singapura") {
        return Some("SG");
    }
    if l == "my"
        || contains_ascii_word(&l, "malaysia")
        || contains_ascii_word(&l, "kuala lumpur")
        || contains_ascii_word(&l, "penang")
        || contains_ascii_word(&l, "selangor")
    {
        return Some("MY");
    }
    if l == "jp"
        || contains_ascii_word(&l, "japan")
        || contains_ascii_word(&l, "jepang")
        || contains_ascii_word(&l, "tokyo")
        || contains_ascii_word(&l, "osaka")
    {
        return Some("JP");
    }
    if l == "kr" || contains_ascii_word(&l, "korea") || contains_ascii_word(&l, "seoul") {
        return Some("KR");
    }
    if l == "tw" || contains_ascii_word(&l, "taiwan") || contains_ascii_word(&l, "taipei") {
        return Some("TW");
    }
    if l == "de"
        || contains_ascii_word(&l, "germany")
        || contains_ascii_word(&l, "jerman")
        || contains_ascii_word(&l, "munich")
    {
        return Some("DE");
    }
    if l == "se" || contains_ascii_word(&l, "sweden") || contains_ascii_word(&l, "swedia") {
        return Some("SE");
    }
    if l == "th" || contains_ascii_word(&l, "thailand") || contains_ascii_word(&l, "bangkok") {
        return Some("TH");
    }
    if l == "vn"
        || contains_ascii_word(&l, "vietnam")
        || contains_ascii_word(&l, "ho chi minh")
        || contains_ascii_word(&l, "hanoi")
    {
        return Some("VN");
    }
    if l == "in"
        || contains_ascii_word(&l, "india")
        || contains_ascii_word(&l, "mumbai")
        || contains_ascii_word(&l, "delhi")
        || contains_ascii_word(&l, "chennai")
    {
        return Some("IN");
    }
    if l == "au"
        || contains_ascii_word(&l, "australia")
        || contains_ascii_word(&l, "sydney")
        || contains_ascii_word(&l, "melbourne")
    {
        return Some("AU");
    }
    if l == "gb"
        || l == "uk"
        || contains_ascii_word(&l, "united kingdom")
        || contains_ascii_word(&l, "inggris")
    {
        return Some("GB");
    }
    if l == "us"
        || l == "usa"
        || contains_ascii_word(&l, "united states")
        || contains_ascii_word(&l, "amerika")
    {
        return Some("US");
    }
    super::countries::COUNTRIES
        .iter()
        .find(|(code, name)| {
            code.eq_ignore_ascii_case(&l) || contains_ascii_word(&l, &name.to_lowercase())
        })
        .map(|(code, _)| *code)
}

pub fn canonical_country_name(code: &str) -> &'static str {
    match code.to_uppercase().as_str() {
        "ID" => "Indonesia",
        "CN" => "China",
        "SG" => "Singapore",
        "MY" => "Malaysia",
        "JP" => "Japan",
        "KR" => "South Korea",
        "TW" => "Taiwan",
        "DE" => "Germany",
        "SE" => "Sweden",
        "TH" => "Thailand",
        "VN" => "Vietnam",
        "IN" => "India",
        "AU" => "Australia",
        "GB" => "United Kingdom",
        "US" => "United States",
        _ => super::countries::COUNTRIES
            .iter()
            .find(|(cc, _)| cc.eq_ignore_ascii_case(code))
            .map(|(_, name)| *name)
            .unwrap_or("Unknown"),
    }
}

pub fn country_code_from_domain(domain: &str) -> Option<&'static str> {
    let d = domain.trim().to_lowercase();
    if d.is_empty() {
        return None;
    }

    if d.ends_with(".id")
        || d.ends_with(".co.id")
        || d.ends_with(".or.id")
        || d.ends_with(".web.id")
    {
        return Some("ID");
    }
    if d.ends_with(".cn") || d.ends_with(".com.cn") {
        return Some("CN");
    }
    if d.ends_with(".sg") || d.ends_with(".com.sg") {
        return Some("SG");
    }
    if d.ends_with(".my") || d.ends_with(".com.my") {
        return Some("MY");
    }
    if d.ends_with(".tw") || d.ends_with(".com.tw") {
        return Some("TW");
    }
    if d.ends_with(".kr") || d.ends_with(".co.kr") {
        return Some("KR");
    }
    if d.ends_with(".jp") || d.ends_with(".co.jp") {
        return Some("JP");
    }
    if d.ends_with(".de") {
        return Some("DE");
    }
    if d.ends_with(".se") {
        return Some("SE");
    }
    if d.ends_with(".co.uk") || d.ends_with(".uk") {
        return Some("GB");
    }
    if d.ends_with(".th") || d.ends_with(".co.th") {
        return Some("TH");
    }
    if d.ends_with(".vn") || d.ends_with(".com.vn") {
        return Some("VN");
    }
    if d.ends_with(".in") || d.ends_with(".co.in") {
        return Some("IN");
    }
    if d.ends_with(".au") || d.ends_with(".com.au") {
        return Some("AU");
    }
    if d.ends_with(".us") {
        return Some("US");
    }

    let id_domains = [
        "tokopedia.com",
        "bukalapak.com",
        "blibli.com",
        "indotrading.com",
        "ralali.com",
        "bhinneka.com",
        "kawanlama.com",
        "perkakasku.com",
        "glodokshop.com",
        "monotaro.id",
    ];
    if id_domains
        .iter()
        .any(|m| d == *m || d.ends_with(&format!(".{}", m)))
    {
        return Some("ID");
    }

    let cn_domains = [
        "made-in-china.com",
        "1688.com",
        "aliexpress.com",
        "dhgate.com",
        "hisupplier.com",
        "globalsources.com",
    ];
    if cn_domains
        .iter()
        .any(|m| d == *m || d.ends_with(&format!(".{}", m)))
    {
        return Some("CN");
    }

    let in_domains = ["indiamart.com", "tradeindia.com", "exportersindia.com"];
    if in_domains
        .iter()
        .any(|m| d == *m || d.ends_with(&format!(".{}", m)))
    {
        return Some("IN");
    }

    let us_domains = [
        "amazon.com",
        "ebay.com",
        "grainger.com",
        "mcmaster.com",
        "walmart.com",
        "homedepot.com",
        "bearmaxusa.com",
        "fastenal.com",
    ];
    if us_domains
        .iter()
        .any(|m| d == *m || d.ends_with(&format!(".{}", m)))
    {
        return Some("US");
    }

    None
}

pub fn country_code_from_phone(phone: &str) -> Option<&'static str> {
    let trimmed = phone.trim();
    if trimmed.is_empty() {
        return None;
    }
    let digits: String = trimmed.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 7 {
        return None;
    }

    let has_plus = trimmed.starts_with('+');
    if has_plus {
        if digits.starts_with("886") {
            return Some("TW");
        }
        if digits.starts_with("62") {
            return Some("ID");
        }
        if digits.starts_with("86") {
            return Some("CN");
        }
        if digits.starts_with("65") {
            return Some("SG");
        }
        if digits.starts_with("60") {
            return Some("MY");
        }
        if digits.starts_with("81") {
            return Some("JP");
        }
        if digits.starts_with("82") {
            return Some("KR");
        }
        if digits.starts_with("49") {
            return Some("DE");
        }
        if digits.starts_with("46") {
            return Some("SE");
        }
        if digits.starts_with("44") {
            return Some("GB");
        }
        if digits.starts_with("66") {
            return Some("TH");
        }
        if digits.starts_with("84") {
            return Some("VN");
        }
        if digits.starts_with("91") {
            return Some("IN");
        }
        if digits.starts_with("61") {
            return Some("AU");
        }
        if digits.starts_with('1') && digits.len() >= 11 {
            return Some("US");
        }
    }

    // Domestic Indonesian mobile (08xx...) or area codes ((021), (031), (022), (024), (061))
    if digits.starts_with("08") && digits.len() >= 9 && digits.len() <= 14 {
        return Some("ID");
    }
    if (trimmed.starts_with("(021)")
        || trimmed.starts_with("(031)")
        || trimmed.starts_with("(022)")
        || trimmed.starts_with("(024)")
        || trimmed.starts_with("(061)")
        || trimmed.starts_with("(0274)")
        || trimmed.starts_with("(0361)"))
        && digits.len() >= 8
    {
        return Some("ID");
    }
    // Common raw 628... without plus
    if digits.starts_with("628") && digits.len() >= 10 && digits.len() <= 15 {
        return Some("ID");
    }
    // Common raw 861... Chinese mobile without plus
    if digits.starts_with("861") && digits.len() == 13 {
        return Some("CN");
    }

    None
}

pub fn extract_indonesian_city_or_region(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    for (kw, canonical) in ID_CITY_CANONICAL {
        if contains_ascii_word(&lower, kw) {
            return Some(*canonical);
        }
    }
    None
}

pub fn extract_chinese_city(text: &str) -> Option<&'static str> {
    let lower = text.to_lowercase();
    for (kw, canonical) in CN_CITY_CANONICAL {
        if contains_ascii_word(&lower, kw) {
            return Some(*canonical);
        }
    }
    None
}

pub fn target_country_codes(target_regions: &[String]) -> Vec<String> {
    let mut codes = Vec::new();
    for reg in target_regions {
        if is_global_label(reg) {
            continue;
        }
        if let Some(cc) = country_code_from_name(reg) {
            if !codes.iter().any(|c| c == cc) {
                codes.push(cc.to_string());
            }
        }
    }
    codes
}

pub fn target_subregions(target_regions: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for reg in target_regions {
        if is_global_label(reg) {
            continue;
        }
        if country_code_from_name(reg).is_some_and(|cc| {
            reg.eq_ignore_ascii_case(cc) || reg.eq_ignore_ascii_case(canonical_country_name(cc))
        }) {
            continue;
        }
        for piece in reg.split(&[',', '(', ')', '&', '/'][..]) {
            let clean = piece.trim().to_lowercase();
            if clean.len() < 3
                || is_global_label(&clean)
                || clean == "seluruh indonesia"
                || clean == "indonesia"
                || clean == "china"
                || clean == "tiongkok"
                || clean == "singapore"
                || clean == "singapura"
                || clean == "malaysia"
                || clean == "japan"
                || clean == "jepang"
                || clean == "south korea"
                || clean == "korea selatan"
                || clean == "taiwan"
                || clean == "germany"
                || clean == "jerman"
                || clean == "united states"
                || clean == "amerika serikat"
                || clean == "usa"
                || clean == "dan"
            {
                continue;
            }
            if !out.contains(&clean) {
                out.push(clean);
            }
        }
    }
    // A parent province in a city selection must not broaden that selection.
    let selected = out.clone();
    out.retain(|region| {
        !ID_CITY_CANONICAL.iter().any(|(city, canonical)| {
            city != region
                && selected.iter().any(|s| s == city)
                && canonical.to_lowercase().ends_with(&format!(", {}", region))
        })
    });
    out
}

/// Location gate shared by web discovery and reuse of stored suppliers.
/// City/region matching uses the supplier address, never product/marketing text.
pub fn supplier_matches_targets(
    country_code: Option<&str>,
    country_name: Option<&str>,
    address: Option<&str>,
    targets: &[String],
) -> bool {
    if targets.is_empty() || targets.iter().all(|t| is_global_label(t)) {
        return true;
    }
    let codes = target_country_codes(targets);
    let resolved = country_code
        .and_then(country_code_from_name)
        .or_else(|| country_name.and_then(country_code_from_name))
        .or_else(|| address.and_then(country_code_from_name));
    if !codes.is_empty() && !resolved.is_some_and(|cc| codes.iter().any(|c| c == cc)) {
        return false;
    }
    let regions = target_subregions(targets);
    if regions.is_empty() {
        return !codes.is_empty();
    }
    let addr = address.unwrap_or("").to_lowercase();
    if addr.is_empty() {
        return false;
    }
    regions.iter().any(|region| {
        contains_ascii_word(&addr, region)
            || (region == "jabodetabek"
                && ["jakarta", "bogor", "depok", "tangerang", "bekasi"]
                    .iter()
                    .any(|city| contains_ascii_word(&addr, city)))
            || ID_CITY_CANONICAL.iter().any(|(city, canonical)| {
                contains_ascii_word(&addr, city)
                    && contains_ascii_word(&canonical.to_lowercase(), region)
            })
    })
}

pub fn primary_subregion_for_query(subregions: &[String]) -> Option<String> {
    if subregions.is_empty() {
        return None;
    }
    let chosen = if subregions[0] == "jabodetabek" && subregions.len() > 1 {
        &subregions[1]
    } else {
        &subregions[0]
    };
    Some(capitalize_words(chosen))
}

fn capitalize_words(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Ensures every query matches the user's selected target country and subregion
/// and strips stray foreign country keywords when a concrete country is targeted.
pub fn enforce_query_regions(
    mut queries: Vec<PlannedQuery>,
    req: &ProductRequirement,
    max_queries: usize,
) -> Vec<PlannedQuery> {
    let target_codes = target_country_codes(&req.target_regions);
    if target_codes.is_empty() {
        queries.truncate(max_queries);
        return queries;
    }

    let subregions = target_subregions(&req.target_regions);
    let primary_city = primary_subregion_for_query(&subregions);
    let primary_cc = target_codes[0].as_str();
    let primary_country_name = canonical_country_name(primary_cc);

    let foreign_country_words = [
        ("indonesia", "ID"),
        ("china", "CN"),
        ("singapore", "SG"),
        ("malaysia", "MY"),
        ("japan", "JP"),
        ("germany", "DE"),
        ("united states", "US"),
        ("usa", "US"),
    ];

    for (query_index, q) in queries.iter_mut().enumerate() {
        let primary_city = if subregions.is_empty() {
            primary_city.clone()
        } else {
            Some(capitalize_words(
                &subregions[query_index % subregions.len()],
            ))
        };
        let mut text = q.query_text.trim().to_string();

        // Remove accidental foreign country keywords not in target_codes
        for (word, code) in &foreign_country_words {
            if !target_codes.iter().any(|c| c == code) {
                let re_pat = format!(r"(?i)\b{}\b", regex::escape(word));
                if let Ok(re) = regex::Regex::new(&re_pat) {
                    text = re.replace_all(&text, "").to_string();
                }
            }
        }
        text = text.split_whitespace().collect::<Vec<_>>().join(" ");

        let lower = text.to_lowercase();
        let has_country = contains_ascii_word(&lower, &primary_country_name.to_lowercase())
            || (primary_cc == "ID"
                && (lower.contains("site:.id") || lower.contains("site:.co.id")))
            || (primary_cc == "CN" && lower.contains("site:.cn"));
        let has_subregion = subregions.iter().any(|sr| contains_ascii_word(&lower, sr));

        if primary_cc == "ID" {
            if let Some(ref city) = primary_city {
                if !has_subregion {
                    text = if has_country {
                        format!("{} {}", text, city)
                    } else {
                        format!("{} {} Indonesia", text, city)
                    };
                } else if !has_country && !has_subregion {
                    text = format!("{} Indonesia", text);
                }
            } else if !has_country {
                text = format!("{} Indonesia", text);
            }
        } else if !has_country && !has_subregion {
            if let Some(ref city) = primary_city {
                text = format!("{} {} {}", text, city, primary_country_name);
            } else {
                text = format!("{} {}", text, primary_country_name);
            }
        }

        q.query_text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    }

    queries.truncate(max_queries);
    queries
}

/// Checks if a URL/domain is blatantly foreign for the selected target country codes.
pub fn is_foreign_candidate_url(domain: &str, url: &str, target_codes: &[String]) -> bool {
    if target_codes.is_empty() {
        return false;
    }

    if let Some(dom_cc) = country_code_from_domain(domain) {
        if !target_codes.iter().any(|c| c.eq_ignore_ascii_case(dom_cc)) {
            return true;
        }
    }

    let dl = domain.to_lowercase();
    let ul = url.to_lowercase();

    // Global platforms with US/foreign regional paths or subdomains
    if !target_codes.iter().any(|c| c == "US") {
        if ul.contains("/en-us/")
            || ul.contains("/en_us/")
            || ul.contains("/us/en/")
            || dl.starts_with("us.")
            || dl.starts_with("usa.")
            || dl.contains("usa.com")
        {
            return true;
        }
    }
    if !target_codes.iter().any(|c| c == "CN") {
        if ul.contains("/zh-cn/") || dl.contains("alibaba.com") {
            return true;
        }
    }
    if !target_codes.iter().any(|c| c == "GB") && ul.contains("/en-gb/") {
        return true;
    }
    if !target_codes.iter().any(|c| c == "JP") && ul.contains("/ja-jp/") {
        return true;
    }

    false
}

/// Checks if a search result title+snippet clearly describes a foreign country when target is concrete.
pub fn is_foreign_snippet(text_blob_lower: &str, target_codes: &[String]) -> bool {
    if target_codes.is_empty() {
        return false;
    }

    let targets_id = target_codes.iter().any(|c| c == "ID");
    let mentions_target = target_codes.iter().any(|cc| match cc.as_str() {
        "ID" => {
            contains_ascii_word(text_blob_lower, "indonesia")
                || ID_CITY_KEYWORDS
                    .iter()
                    .any(|k| contains_ascii_word(text_blob_lower, k))
        }
        "CN" => {
            contains_ascii_word(text_blob_lower, "china")
                || contains_ascii_word(text_blob_lower, "shenzhen")
        }
        "SG" => {
            contains_ascii_word(text_blob_lower, "singapore")
                || contains_ascii_word(text_blob_lower, "singapura")
        }
        "MY" => contains_ascii_word(text_blob_lower, "malaysia"),
        "JP" => contains_ascii_word(text_blob_lower, "japan"),
        "US" => {
            contains_ascii_word(text_blob_lower, "united states")
                || contains_ascii_word(text_blob_lower, "usa")
        }
        _ => contains_ascii_word(text_blob_lower, &canonical_country_name(cc).to_lowercase()),
    });

    if mentions_target {
        return false;
    }

    let foreign_signals: &[(&str, &str)] = &[
        ("united states", "US"),
        ("free shipping", "US"),
        ("bearmax usa", "US"),
        ("california", "US"),
        ("texas", "US"),
        ("made in china", "CN"),
        ("china manufacturer", "CN"),
        ("china supplier", "CN"),
        ("shenzhen", "CN"),
        ("guangzhou", "CN"),
        ("indiamart", "IN"),
        ("mumbai", "IN"),
    ];

    for (sig, cc) in foreign_signals {
        if !target_codes.iter().any(|c| c == cc) && text_blob_lower.contains(sig) {
            // Avoid rejecting generic phrases like "made in china" if page is clearly Indonesian
            if *sig == "made in china" && targets_id && text_blob_lower.contains("jual") {
                continue;
            }
            return true;
        }
    }

    false
}

/// Keep address/contact statements; destination lists and product origin are not vendor locations.
pub fn contact_location_text(text: &str) -> String {
    text.split(&['\n', ';'][..])
        .filter(|line| {
            let lower = line.to_lowercase();
            [
                "alamat",
                "address",
                "office:",
                "kantor",
                "located at",
                "lokasi:",
            ]
            .iter()
            .any(|cue| lower.contains(cue))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Conflicting location signals are not accepted as a confirmed local vendor.
pub fn consistent_supplier_location(
    code: Option<&str>,
    name: Option<&str>,
    address: Option<&str>,
    phone: Option<&str>,
) -> bool {
    let signals = [
        code.and_then(country_code_from_name),
        name.and_then(country_code_from_name),
        address.and_then(country_code_from_name),
        phone.and_then(country_code_from_phone),
    ];
    let mut known = signals.into_iter().flatten();
    let Some(first) = known.next() else {
        return true;
    };
    known.all(|cc| cc == first)
}

/// Resolves and normalizes country_code, country_name, and missing city/address on an extracted payload.
pub fn enrich_extracted_location(
    payload: &mut ExtractedSupplierPayload,
    page_url: &str,
    page_text: &str,
) {
    let domain = extract_domain(page_url).unwrap_or_default();
    let _domain_cc = country_code_from_domain(&domain);
    let phone_cc = payload
        .public_phone
        .as_deref()
        .and_then(country_code_from_phone);
    let code_cc = payload
        .country_code
        .as_deref()
        .and_then(country_code_from_name);
    let name_cc = payload
        .country_name
        .as_deref()
        .and_then(country_code_from_name);
    let contact_text = contact_location_text(page_text);
    let id_city = extract_indonesian_city_or_region(&contact_text);
    let cn_city = extract_chinese_city(&contact_text);

    // Do not let hosting domain overwrite a supplier's physical address.
    let resolved_cc = payload
        .address
        .as_deref()
        .and_then(country_code_from_name)
        .or_else(|| country_code_from_name(&contact_text))
        .or(code_cc)
        .or(name_cc)
        .or(phone_cc);

    if let Some(cc) = resolved_cc {
        payload.country_code = Some(cc.to_string());
        let name_matches = payload
            .country_name
            .as_deref()
            .and_then(country_code_from_name)
            .map(|ncc| ncc == cc)
            .unwrap_or(false);
        if payload.country_name.is_none() || (!name_matches && name_cc.is_none()) {
            payload.country_name = Some(canonical_country_name(cc).to_string());
        }

        if payload
            .address
            .as_deref()
            .map(|a| a.trim().is_empty())
            .unwrap_or(true)
        {
            if cc == "ID" {
                if let Some(city) = id_city {
                    payload.address = Some(city.to_string());
                }
            } else if cc == "CN" {
                if let Some(city) = cn_city {
                    payload.address = Some(city.to_string());
                }
            }
        }
    } else if payload
        .address
        .as_deref()
        .map(|a| a.trim().is_empty())
        .unwrap_or(true)
    {
        if let Some(city) = id_city {
            payload.address = Some(city.to_string());
        }
    }
}

/// Checks whether an extracted supplier with resolved location is acceptable for the job's target_regions.
pub fn is_supplier_allowed_for_targets(
    payload: &ExtractedSupplierPayload,
    page_url: &str,
    page_text: &str,
    target_regions: &[String],
) -> bool {
    if !consistent_supplier_location(
        payload.country_code.as_deref(),
        payload.country_name.as_deref(),
        payload.address.as_deref(),
        payload.public_phone.as_deref(),
    ) {
        return false;
    }
    if !supplier_matches_targets(
        payload.country_code.as_deref(),
        payload.country_name.as_deref(),
        payload.address.as_deref(),
        target_regions,
    ) {
        return false;
    }
    let target_codes = target_country_codes(target_regions);
    if target_codes.is_empty() {
        return true;
    }

    let domain = extract_domain(page_url).unwrap_or_default();
    if let Some(ref cc) = payload.country_code {
        return target_codes.iter().any(|tc| tc.eq_ignore_ascii_case(cc));
    }

    if let Some(name_cc) = payload
        .country_name
        .as_deref()
        .and_then(country_code_from_name)
    {
        return target_codes
            .iter()
            .any(|tc| tc.eq_ignore_ascii_case(name_cc));
    }

    // Unknown country on a generic TLD (.com/.net/.org): require positive signal of target country
    let lower = page_text.to_lowercase();
    let url_lower = page_url.to_lowercase();
    let email_lower = payload.public_email.as_deref().unwrap_or("").to_lowercase();
    let phone_str = payload.public_phone.as_deref().unwrap_or("");

    target_codes.iter().any(|cc| match cc.as_str() {
        "ID" => {
            domain.ends_with(".id")
                || email_lower.ends_with(".id")
                || url_lower.contains("/id/")
                || country_code_from_phone(phone_str) == Some("ID")
                || contains_ascii_word(&lower, "indonesia")
                || extract_indonesian_city_or_region(page_text).is_some()
                || ((lower.contains("rp ") || lower.contains("idr"))
                    && (contains_ascii_word(&lower, "jual")
                        || contains_ascii_word(&lower, "harga")
                        || contains_ascii_word(&lower, "hubungi")))
        }
        "CN" => {
            domain.ends_with(".cn")
                || country_code_from_phone(phone_str) == Some("CN")
                || contains_ascii_word(&lower, "china")
                || extract_chinese_city(page_text).is_some()
        }
        _ => {
            let cname = canonical_country_name(cc).to_lowercase();
            country_code_from_phone(phone_str) == Some(cc.as_str())
                || contains_ascii_word(&lower, &cname)
        }
    })
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    #[test]
    fn city_selection_is_a_filter_not_a_country_bonus() {
        let targets = vec![
            "Surabaya, Gresik, Sidoarjo, Jawa Timur".into(),
            "Indonesia".into(),
        ];
        assert!(supplier_matches_targets(
            Some("ID"),
            None,
            Some("Jl Industri, Surabaya"),
            &targets
        ));
        assert!(!supplier_matches_targets(
            Some("ID"),
            None,
            Some("Malang, Jawa Timur"),
            &targets
        ));
        assert!(!supplier_matches_targets(
            Some("ID"),
            None,
            Some("Jakarta"),
            &targets
        ));
        assert!(!supplier_matches_targets(Some("ID"), None, None, &targets));
        assert!(!supplier_matches_targets(
            Some("CN"),
            None,
            Some("Surabaya"),
            &targets
        ));
        assert!(supplier_matches_targets(
            Some("ID"),
            None,
            Some("Bogor"),
            &["Jabodetabek".into(), "Indonesia".into()]
        ));
        assert!(supplier_matches_targets(
            Some("ID"),
            None,
            Some("Surabaya"),
            &["Jawa Timur".into(), "Indonesia".into()]
        ));
        assert!(supplier_matches_targets(
            Some("SE"),
            None,
            None,
            &["Sweden".into()]
        ));
        assert!(supplier_matches_targets(
            None,
            None,
            None,
            &["worldwide".into()]
        ));
        assert!(!supplier_matches_targets(
            None,
            None,
            None,
            &["Vietnam".into()]
        ));
        assert!(supplier_matches_targets(
            Some("ID"),
            None,
            Some("Kudus"),
            &["Kudus, Jawa Tengah".into(), "Indonesia".into()]
        ));
        assert!(!supplier_matches_targets(
            Some("ID"),
            None,
            Some("Semarang, Jawa Tengah"),
            &["Kudus, Jawa Tengah".into(), "Indonesia".into()]
        ));
    }
}

#[cfg(test)]
mod country_regression_tests {
    use super::*;
    fn payload(
        code: &str,
        name: &str,
        address: Option<&str>,
        phone: Option<&str>,
    ) -> ExtractedSupplierPayload {
        serde_json::from_value(serde_json::json!({"country_code": code, "country_name": name, "address": address, "public_phone": phone})).unwrap()
    }
    #[test]
    fn indonesia_does_not_accept_chinese_vendor_or_conflicting_contact() {
        let target = vec!["Indonesia".into()];
        let mut vendor = payload(
            "CN",
            "China",
            Some("Shenzhen, China"),
            Some("+8618365833398"),
        );
        enrich_extracted_location(
            &mut vendor,
            "https://vendor.id",
            "We ship to Indonesia and worldwide",
        );
        assert_eq!(vendor.country_code.as_deref(), Some("CN"));
        assert!(!is_supplier_allowed_for_targets(
            &vendor,
            "https://vendor.id",
            "Indonesia",
            &target
        ));
        let wrong = payload(
            "ID",
            "Indonesia",
            Some("Jakarta, Indonesia"),
            Some("+8618365833398"),
        );
        assert!(!is_supplier_allowed_for_targets(
            &wrong,
            "https://vendor.com",
            "Indonesia",
            &target
        ));
        let local = payload(
            "ID",
            "Indonesia",
            Some("Jakarta, Indonesia"),
            Some("+6281234567890"),
        );
        assert!(is_supplier_allowed_for_targets(
            &local,
            "https://local.com",
            "Distributor barang made in China",
            &target
        ));
        assert!(contact_location_text("Ships to Indonesia. Product made in China").is_empty());
        let unknown: ExtractedSupplierPayload =
            serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(!is_supplier_allowed_for_targets(
            &unknown,
            "https://vendor.com",
            "Ships to Indonesia",
            &target
        ));
    }
    #[test]
    fn expanded_queries_keep_the_selected_country() {
        let req: ProductRequirement = serde_json::from_value(serde_json::json!({"product_name": "bearing SKF", "category": "bearing", "target_regions": ["Indonesia"], "search_languages": ["id", "en"]})).unwrap();
        let client = crate::services::gemini::GeminiClient::new(String::new(), "unused".into());
        let planned = client.fallback_plan_queries(&req, 12);
        assert!(planned.len() >= 8);
        for query in planned {
            assert!(query.query_text.to_lowercase().contains("indonesia"));
        }
        let extracted = client.fallback_extract_supplier(
            "https://vendor.com",
            "China manufacturer",
            "We ship bearings to Indonesia. Made in China.",
            &req,
        );
        assert!(
            extracted.country_code.is_none(),
            "Shipping destination cannot establish seller country"
        );
    }
}
