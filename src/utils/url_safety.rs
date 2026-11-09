use std::net::IpAddr;
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum UrlSafetyError {
    #[error("Invalid URL format: {0}")]
    ParseError(#[from] url::ParseError),
    #[error("Disallowed scheme: '{0}'. Only http and https are permitted")]
    DisallowedScheme(String),
    #[error("Missing host in URL")]
    MissingHost,
    #[error("Host is blocked (localhost, loopback, or private domain)")]
    BlockedHost,
    #[error("Target IP {0} is private, loopback, link-local, or cloud metadata")]
    BlockedIp(IpAddr),
    #[error("Disallowed port: {0}. Only ports 80 and 443 are allowed")]
    DisallowedPort(u16),
    #[error("DNS resolution error: {0}")]
    DnsError(String),
}

pub fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            // Loopback 127.0.0.0/8
            ipv4.is_loopback()
                // Unspecified 0.0.0.0
                || ipv4.is_unspecified()
                // Broadcast 255.255.255.255
                || ipv4.is_broadcast()
                // Link-local / Cloud metadata: 169.254.0.0/16
                || ipv4.is_link_local()
                // Private Class A: 10.0.0.0/8
                || octets[0] == 10
                // Private Class B: 172.16.0.0/12
                || (octets[0] == 172 && (16..=31).contains(&octets[1]))
                // Private Class C: 192.168.0.0/16
                || (octets[0] == 192 && octets[1] == 168)
                // Multicast: 224.0.0.0/4
                || ipv4.is_multicast()
                // Carrier-grade NAT: 100.64.0.0/10
                || (octets[0] == 100 && (octets[1] & 0xc0) == 64)
        }
        IpAddr::V6(ipv6) => {
            ipv6.is_loopback()
                || ipv6.is_unspecified()
                || ipv6.is_multicast()
                // Unique Local Address fc00::/7
                || (ipv6.segments()[0] & 0xfe00) == 0xfc00
                // Link-local fe80::/10
                || (ipv6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

pub async fn validate_url_safety(raw_url: &str) -> Result<Url, UrlSafetyError> {
    let url = Url::parse(raw_url)?;

    // 1. Check scheme
    let scheme = url.scheme().to_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(UrlSafetyError::DisallowedScheme(scheme));
    }

    // 2. Check host
    let host_str = match url.host_str() {
        Some(h) => h.to_lowercase(),
        None => return Err(UrlSafetyError::MissingHost),
    };

    if host_str == "localhost"
        || host_str.ends_with(".localhost")
        || host_str.ends_with(".local")
        || host_str.ends_with(".internal")
        || host_str.ends_with(".onion")
    {
        return Err(UrlSafetyError::BlockedHost);
    }

    // 3. Check port
    if let Some(port) = url.port() {
        if port != 80 && port != 443 && port != 8080 {
            return Err(UrlSafetyError::DisallowedPort(port));
        }
    }

    // 4. Check if direct IP address
    if let Ok(ip) = host_str.parse::<IpAddr>() {
        if is_private_ip(&ip) {
            return Err(UrlSafetyError::BlockedIp(ip));
        }
    } else {
        // 5. DNS resolution check (prevent DNS rebinding / SSRF)
        let host_with_port = format!("{}:{}", host_str, url.port_or_known_default().unwrap_or(80));
        let addrs = tokio::net::lookup_host(&host_with_port)
            .await
            .map_err(|e| UrlSafetyError::DnsError(e.to_string()))?;

        for addr in addrs {
            let ip = addr.ip();
            if is_private_ip(&ip) {
                return Err(UrlSafetyError::BlockedIp(ip));
            }
        }
    }

    Ok(url)
}

pub fn normalize_url(raw_url: &str) -> Result<String, url::ParseError> {
    let mut url = Url::parse(raw_url)?;

    // Scheme to lowercase
    let scheme = url.scheme().to_lowercase();
    let _ = url.set_scheme(&scheme);

    // Strip fragment
    url.set_fragment(None);

    // Filter tracking query params
    let filtered_query: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| {
            let key = k.to_lowercase();
            !key.starts_with("utm_")
                && key != "fbclid"
                && key != "gclid"
                && key != "ref"
                && key != "source"
        })
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    if filtered_query.is_empty() {
        url.set_query(None);
    } else {
        url.query_pairs_mut().clear();
        for (k, v) in filtered_query {
            url.query_pairs_mut().append_pair(&k, &v);
        }
    }

    // Remove default ports
    if (scheme == "http" && url.port() == Some(80))
        || (scheme == "https" && url.port() == Some(443))
    {
        let _ = url.set_port(None);
    }

    // Remove trailing slash if path is just "/"
    let mut s = url.to_string();
    if s.ends_with('/') && url.path() == "/" && url.query().is_none() {
        s.pop();
    }

    Ok(s)
}

pub fn extract_domain(raw_url: &str) -> Option<String> {
    let url = Url::parse(raw_url).ok()?;
    let host = url.host_str()?;
    let clean = host.strip_prefix("www.").unwrap_or(host);
    Some(clean.to_lowercase())
}
