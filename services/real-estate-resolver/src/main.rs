use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

const DEFAULT_RENDER_KEEPALIVE_SECONDS: u64 = 10 * 60;
const MIN_RENDER_KEEPALIVE_SECONDS: u64 = 60;
const BANGER_GOOGLE_TILES_PROXY_PREFIX: &str = "/api/banger/google-tiles/";
const BANGER_CESIUM_ION_TOKEN_PATH: &str = "/api/banger/cesium-ion-token";
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveRequest {
    agency_name: Option<String>,
    city: Option<String>,
    query: Option<String>,
    country_code: Option<String>,
    surface: Option<String>,
    scope: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AgencyLocation {
    lat: f64,
    lng: f64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AgencyPayload {
    display_name: String,
    formatted_address: String,
    website_uri: String,
    google_maps_uri: String,
    national_phone_number: Option<String>,
    location: AgencyLocation,
    confidence: f64,
    source: String,
}

fn main() -> Result<(), String> {
    let port = std::env::var("PORT")
        .ok()
        .and_then(|value| value.trim().parse::<u16>().ok())
        .or_else(|| {
            std::env::args()
                .skip(1)
                .find_map(|arg| arg.parse::<u16>().ok())
        })
        .unwrap_or(8765);
    let listener = TcpListener::bind(("0.0.0.0", port))
        .map_err(|err| format!("bind real estate resolver on {port}: {err}"))?;
    println!("forge-real-estate-resolver listening on 0.0.0.0:{port}");
    spawn_render_keepalive();
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    let _ = handle_connection(stream);
                });
            }
            Err(err) => eprintln!("accept error: {err}"),
        }
    }
    Ok(())
}

fn handle_connection(mut stream: TcpStream) -> Result<(), String> {
    let mut request_bytes = Vec::with_capacity(32 * 1024);
    let mut buffer = [0_u8; 8192];
    let header_end = loop {
        let bytes_read = stream
            .read(&mut buffer)
            .map_err(|err| format!("read request: {err}"))?;
        if bytes_read == 0 {
            if request_bytes.is_empty() {
                return Ok(());
            }
            return write_json_response(
                &mut stream,
                400,
                &json!({ "error": "bad_request", "message": "incomplete request headers" }),
            );
        }
        request_bytes.extend_from_slice(&buffer[..bytes_read]);
        if let Some(index) = find_header_end(&request_bytes) {
            break index;
        }
        if request_bytes.len() > 256 * 1024 {
            return write_json_response(
                &mut stream,
                413,
                &json!({ "error": "payload_too_large", "message": "headers exceed limit" }),
            );
        }
    };
    let header_text = String::from_utf8_lossy(&request_bytes[..header_end]).to_string();
    let request_line = header_text.lines().next().unwrap_or_default().to_string();
    let mut content_length = 0_usize;
    let mut auth_header = String::new();
    for line in header_text.lines().skip(1) {
        if line.is_empty() {
            break;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("content-length:") {
            content_length = value.trim().parse::<usize>().unwrap_or(0);
        } else if lower.starts_with("authorization:") {
            auth_header = line
                .split_once(':')
                .map(|(_, value)| value.trim().to_string())
                .unwrap_or_default();
        }
    }
    let body_start = header_end
        + if request_bytes.get(header_end..header_end + 4) == Some(b"\r\n\r\n") {
            4
        } else {
            2
        };
    while request_bytes.len().saturating_sub(body_start) < content_length {
        let bytes_read = stream
            .read(&mut buffer)
            .map_err(|err| format!("read request body: {err}"))?;
        if bytes_read == 0 {
            break;
        }
        request_bytes.extend_from_slice(&buffer[..bytes_read]);
    }
    let body_bytes = if content_length > 0 {
        let available = request_bytes.len().saturating_sub(body_start);
        if available < content_length {
            return write_json_response(
                &mut stream,
                400,
                &json!({ "error": "bad_request", "message": "request body shorter than content-length" }),
            );
        }
        &request_bytes[body_start..body_start + content_length]
    } else {
        &request_bytes[body_start..]
    };
    let body = String::from_utf8_lossy(body_bytes).to_string();

    if request_line.starts_with("GET /health ") || request_line.starts_with("GET /healthz ") {
        return write_json_response(
            &mut stream,
            200,
            &json!({ "ok": true, "service": "forge-real-estate-resolver", "sleepGuard": render_keepalive_enabled() }),
        );
    }

    if let Some(target) = request_target(&request_line) {
        let (path, _query) = request_path_and_query(target);
        if request_line.starts_with("GET ") && path == BANGER_CESIUM_ION_TOKEN_PATH {
            return write_banger_cesium_ion_token(&mut stream);
        }
        if request_line.starts_with("OPTIONS ") && path == BANGER_CESIUM_ION_TOKEN_PATH {
            return write_empty_response(&mut stream, 204);
        }
        if request_line.starts_with("GET ") && target.starts_with(BANGER_GOOGLE_TILES_PROXY_PREFIX) {
            return proxy_banger_google_tiles(&mut stream, target);
        }
        if request_line.starts_with("OPTIONS ") && target.starts_with(BANGER_GOOGLE_TILES_PROXY_PREFIX) {
            return write_empty_response(&mut stream, 204);
        }
    }

    let expected_token = resolver_token();
    if let Some(expected) = expected_token {
        let presented = auth_header
            .strip_prefix("Bearer ")
            .map(str::trim)
            .unwrap_or_default();
        if presented != expected {
            return write_json_response(
                &mut stream,
                401,
                &json!({ "error": "unauthorized", "message": "missing or invalid bearer token" }),
            );
        }
    }

    if !request_line.starts_with("POST /api/agency/resolve ") {
        return write_json_response(
            &mut stream,
            404,
            &json!({ "error": "not_found", "message": request_line }),
        );
    }

    let payload: ResolveRequest = match serde_json::from_str(&body) {
        Ok(payload) => payload,
        Err(err) => {
            return write_json_response(
                &mut stream,
                400,
                &json!({ "error": "bad_request", "message": format!("decode request json: {err}") }),
            );
        }
    };
    let agency_name = payload
        .agency_name
        .unwrap_or_else(|| "Agence Forge".to_string())
        .trim()
        .to_string();
    let city = payload
        .city
        .unwrap_or_else(|| "Paris".to_string())
        .trim()
        .to_string();
    let query = payload
        .query
        .clone()
        .unwrap_or_else(|| format!("{agency_name} {city} agence immobiliere"));

    let resolved = match google_places_api_key() {
        Some(api_key) => google_places_text_search_contact(&api_key, &query)
            .unwrap_or_else(|_| build_agency_payload(&agency_name, &city)),
        None => build_agency_payload(&agency_name, &city),
    };
    let source = resolved.source.clone();
    let llm_handoff = google_places_llm_handoff_payload(&resolved);
    write_json_response(
        &mut stream,
        200,
        &json!({
            "agency": resolved,
            "llmHandoff": llm_handoff,
            "meta": {
                "countryCode": payload.country_code.unwrap_or_else(|| "FR".to_string()),
                "surface": payload.surface.unwrap_or_else(|| "forge-ui".to_string()),
                "scope": payload.scope.unwrap_or_else(|| "real-estate-onboarding".to_string()),
                "echoQuery": query,
                "resolverSource": source
            }
        }),
    )
}

fn truthy_env(name: &str) -> bool {
    std::env::var(name)
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
}

fn request_target(request_line: &str) -> Option<&str> {
    request_line.split_whitespace().nth(1)
}

fn request_path_and_query(target: &str) -> (&str, &str) {
    target.split_once('?').unwrap_or((target, ""))
}

fn render_keepalive_enabled() -> bool {
    if std::env::var("FORGE_RENDER_KEEPALIVE")
        .map(|value| value.trim().eq_ignore_ascii_case("false") || value.trim() == "0")
        .unwrap_or(false)
    {
        return false;
    }
    truthy_env("FORGE_RENDER_KEEPALIVE") || truthy_env("RENDER")
}

fn render_keepalive_url() -> Option<String> {
    std::env::var("FORGE_RENDER_KEEPALIVE_URL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var("RENDER_EXTERNAL_URL")
                .ok()
                .map(|value| value.trim().trim_end_matches('/').to_string())
                .filter(|value| !value.is_empty())
                .map(|base| format!("{base}/health"))
        })
}

fn render_keepalive_interval() -> Duration {
    let seconds = std::env::var("FORGE_RENDER_KEEPALIVE_SECONDS")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_RENDER_KEEPALIVE_SECONDS)
        .max(MIN_RENDER_KEEPALIVE_SECONDS);
    Duration::from_secs(seconds)
}

fn spawn_render_keepalive() {
    if !render_keepalive_enabled() {
        return;
    }
    let Some(url) = render_keepalive_url() else {
        eprintln!("render keepalive enabled but no FORGE_RENDER_KEEPALIVE_URL or RENDER_EXTERNAL_URL is available");
        return;
    };
    let interval = render_keepalive_interval();
    thread::spawn(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(8))
            .user_agent("forge-render-keepalive/1.0")
            .build();
        let Ok(client) = client else {
            eprintln!("render keepalive client could not be built");
            return;
        };
        loop {
            thread::sleep(interval);
            match client.get(&url).header("x-forge-keepalive", "internal").send() {
                Ok(response) if response.status().is_success() => {
                    println!("render keepalive ok {} {}", response.status(), url);
                }
                Ok(response) => {
                    eprintln!("render keepalive non-success {} {}", response.status(), url);
                }
                Err(error) => {
                    eprintln!("render keepalive failed for {url}: {error}");
                }
            }
        }
    });
}

fn google_places_llm_handoff_payload(resolved: &AgencyPayload) -> Value {
    json!({
        "googlePlacesResult": {
            "tool": "google_places_search",
            "mustUse": true,
            "agencyName": resolved.display_name.clone(),
            "address": resolved.formatted_address.clone(),
            "phone": resolved.national_phone_number.clone().unwrap_or_default(),
            "website": resolved.website_uri.clone(),
            "source": resolved.source.clone(),
            "status": if resolved.source.contains("google_places") { "ok" } else { "fallback" }
        }
    })
}

fn find_header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .or_else(|| bytes.windows(2).position(|window| window == b"\n\n"))
}

fn write_json_response(
    stream: &mut TcpStream,
    status: u16,
    payload: &Value,
) -> Result<(), String> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "OK",
    };
    let body = serde_json::to_string(payload).map_err(|err| format!("encode response: {err}"))?;
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Content-Type, Authorization\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|err| format!("write response: {err}"))?;
    stream.flush().map_err(|err| format!("flush response: {err}"))
}

fn write_empty_response(stream: &mut TcpStream, status: u16) -> Result<(), String> {
    let reason = match status {
        204 => "No Content",
        _ => "OK",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type, Authorization\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|err| format!("write response: {err}"))?;
    stream.flush().map_err(|err| format!("flush response: {err}"))
}

fn write_binary_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<(), String> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "OK",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(header.as_bytes())
        .map_err(|err| format!("write response headers: {err}"))?;
    stream
        .write_all(body)
        .map_err(|err| format!("write response body: {err}"))?;
    stream.flush().map_err(|err| format!("flush response: {err}"))
}

fn resolver_token() -> Option<String> {
    [
        "FORGE_REAL_ESTATE_RESOLVER_TOKEN",
        "FORGE_REAL_ESTATE_BACKEND_TOKEN",
        "FORGE_TOKEN",
    ]
    .iter()
    .find_map(|name| {
        std::env::var(name)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn google_places_api_key() -> Option<String> {
    [
        "FORGE_GOOGLE_PLACES_API_KEY",
        "GOOGLE_PLACES_API_KEY",
        "GOOGLE_API_KEY",
        "GEMINI_API_KEY",
    ]
    .iter()
    .find_map(|name| {
        std::env::var(name)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn google_map_tiles_api_key() -> Option<String> {
    [
        "FORGE_GOOGLE_MAP_TILES_API_KEY",
        "GOOGLE_MAP_TILES_API_KEY",
        "GOOGLE_MAPS_API_KEY",
        "GOOGLE_API_KEY",
    ]
    .iter()
    .find_map(|name| {
        std::env::var(name)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn cesium_ion_access_token() -> Option<String> {
    [
        "FORGE_CESIUM_ACCESS_TOKEN",
        "CESIUM_ACCESS_TOKEN",
        "VITE_CESIUM_ACCESS_TOKEN",
    ]
    .iter()
    .find_map(|name| {
        std::env::var(name)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

fn write_banger_cesium_ion_token(stream: &mut TcpStream) -> Result<(), String> {
    let Some(token) = cesium_ion_access_token() else {
        return write_json_response(
            stream,
            503,
            &json!({
                "accepted": false,
                "schema": "forge.banger.cesium_ion_token.v1",
                "error": "cesium_ion_token_missing"
            }),
        );
    };
    write_json_response(
        stream,
        200,
        &json!({
            "accepted": true,
            "schema": "forge.banger.cesium_ion_token.v1",
            "token": token
        }),
    )
}

fn proxy_banger_google_tiles(stream: &mut TcpStream, target: &str) -> Result<(), String> {
    let Some(api_key) = google_map_tiles_api_key() else {
        return write_json_response(
            stream,
            503,
            &json!({
                "error": "google_map_tiles_key_missing",
                "message": "Set GOOGLE_MAP_TILES_API_KEY or FORGE_GOOGLE_MAP_TILES_API_KEY on Render."
            }),
        );
    };
    let relative = target
        .strip_prefix(BANGER_GOOGLE_TILES_PROXY_PREFIX)
        .unwrap_or("root.json");
    let (tile_path, query) = relative.split_once('?').unwrap_or((relative, ""));
    let tile_path = normalize_google_tiles_proxy_path(tile_path);
    let query = sanitize_google_tiles_query(query);
    let query_prefix = if query.trim().is_empty() {
        "?".to_string()
    } else {
        format!("?{query}&")
    };
    let remote_url = format!(
        "https://tile.googleapis.com/v1/3dtiles/{tile_path}{query_prefix}key={api_key}"
    );
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|err| format!("google tiles client: {err}"))?;
    let response = client
        .get(remote_url)
        .send()
        .map_err(|err| format!("google tiles request: {err}"))?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    let bytes = response
        .bytes()
        .map_err(|err| format!("google tiles body: {err}"))?;
    let rewritten_json;
    let body = if content_type.contains("json") {
        rewritten_json = rewrite_google_tiles_json(bytes.as_ref(), BANGER_GOOGLE_TILES_PROXY_PREFIX);
        rewritten_json.as_deref().unwrap_or(bytes.as_ref())
    } else {
        bytes.as_ref()
    };
    write_binary_response(stream, status, &content_type, body)
}

fn rewrite_google_tiles_json(bytes: &[u8], proxy_prefix: &str) -> Option<Vec<u8>> {
    let mut value: Value = serde_json::from_slice(bytes).ok()?;
    rewrite_google_tiles_value(&mut value, proxy_prefix);
    serde_json::to_vec(&value).ok()
}

fn normalize_google_tiles_proxy_path(tile_path: &str) -> String {
    let trimmed = tile_path.trim().trim_start_matches('/');
    let without_api_prefix = trimmed.strip_prefix("v1/3dtiles/").unwrap_or(trimmed);
    if without_api_prefix.is_empty() {
        "root.json".to_string()
    } else {
        without_api_prefix.to_string()
    }
}

fn sanitize_google_tiles_query(query: &str) -> String {
    query
        .split('&')
        .filter(|part| {
            let key = part.split_once('=').map(|(key, _)| key).unwrap_or(part);
            !key.eq_ignore_ascii_case("key")
        })
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>()
        .join("&")
}

fn rewrite_google_tiles_value(value: &mut Value, proxy_prefix: &str) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(uri)) = map.get_mut("uri") {
                if let Some(rewritten) = rewrite_google_tiles_uri(uri, proxy_prefix) {
                    *uri = rewritten;
                }
            }
            for child in map.values_mut() {
                rewrite_google_tiles_value(child, proxy_prefix);
            }
        }
        Value::Array(items) => {
            for child in items {
                rewrite_google_tiles_value(child, proxy_prefix);
            }
        }
        _ => {}
    }
}

fn rewrite_google_tiles_uri(uri: &str, proxy_prefix: &str) -> Option<String> {
    let google_prefix = "https://tile.googleapis.com/v1/3dtiles/";
    let rest = uri.strip_prefix(google_prefix)?;
    let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let query = sanitize_google_tiles_query(query);
    if query.is_empty() {
        Some(format!("{proxy_prefix}{path}"))
    } else {
        Some(format!("{proxy_prefix}{path}?{query}"))
    }
}

fn google_places_text_search_contact(
    api_key: &str,
    text_query: &str,
) -> Result<AgencyPayload, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| format!("google places client: {err}"))?;
    let response = client
        .post("https://places.googleapis.com/v1/places:searchText")
        .header("Content-Type", "application/json")
        .header("X-Goog-Api-Key", api_key)
        .header(
            "X-Goog-FieldMask",
            "places.displayName,places.formattedAddress,places.nationalPhoneNumber,places.websiteUri,places.googleMapsUri,places.location",
        )
        .json(&json!({
            "textQuery": text_query,
            "languageCode": "fr",
            "pageSize": 1
        }))
        .send()
        .map_err(|err| format!("google places request: {err}"))?;
    if !response.status().is_success() {
        let code = response.status().as_u16();
        let body = response.text().unwrap_or_default();
        return Err(format!(
            "google places status {code}: {}",
            truncate_for_log(&body, 220)
        ));
    }
    let payload: Value = response
        .json()
        .map_err(|err| format!("google places parse: {err}"))?;
    let Some(place) = payload
        .get("places")
        .and_then(|places| places.as_array())
        .and_then(|places| places.first())
    else {
        return Err("google places returned no candidate".to_string());
    };
    let display_name = place
        .get("displayName")
        .and_then(|item| item.get("text"))
        .and_then(|item| item.as_str())
        .map(|value| value.to_string())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "google places missing displayName".to_string())?;
    let formatted_address = place
        .get("formattedAddress")
        .and_then(|item| item.as_str())
        .map(|value| value.to_string())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "google places missing formattedAddress".to_string())?;
    let website_uri = place
        .get("websiteUri")
        .and_then(|item| item.as_str())
        .map(|value| value.to_string())
        .unwrap_or_default();
    let google_maps_uri = place
        .get("googleMapsUri")
        .and_then(|item| item.as_str())
        .map(|value| value.to_string())
        .unwrap_or_default();
    let national_phone_number = place
        .get("nationalPhoneNumber")
        .and_then(|item| item.as_str())
        .map(|value| value.to_string());
    let lat = place
        .get("location")
        .and_then(|item| item.get("latitude"))
        .and_then(|item| item.as_f64())
        .ok_or_else(|| "google places missing latitude".to_string())?;
    let lng = place
        .get("location")
        .and_then(|item| item.get("longitude"))
        .and_then(|item| item.as_f64())
        .ok_or_else(|| "google places missing longitude".to_string())?;
    Ok(AgencyPayload {
        display_name,
        formatted_address,
        website_uri,
        google_maps_uri,
        national_phone_number,
        location: AgencyLocation { lat, lng },
        confidence: 0.99,
        source: "google-places".to_string(),
    })
}

fn truncate_for_log(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        return value.to_string();
    }
    format!("{}…", &value[..limit])
}

fn build_agency_payload(agency_name: &str, city: &str) -> AgencyPayload {
    let (lat, lng) = city_coordinates(city);
    let slug = agency_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    AgencyPayload {
        display_name: agency_name.to_string(),
        formatted_address: format!("{}, {}, France", synthetic_street(city), city),
        website_uri: format!(
            "https://{}.example",
            if slug.is_empty() {
                "agence-forge"
            } else {
                &slug
            }
        ),
        google_maps_uri: format!("https://maps.google.com/?q={lat},{lng}"),
        national_phone_number: None,
        location: AgencyLocation { lat, lng },
        confidence: 0.42,
        source: "forge-resolver-fallback".to_string(),
    }
}

fn synthetic_street(city: &str) -> &'static str {
    match normalize_city(city).as_str() {
        "marcq en baroeul" => "12 avenue de la Republique",
        "lyon" => "18 rue des Fleurs",
        "lille" => "7 place du Theatre",
        "paris" => "24 rue de Rivoli",
        _ => "9 rue de la Forge",
    }
}

fn city_coordinates(city: &str) -> (f64, f64) {
    let known: HashMap<&'static str, (f64, f64)> = HashMap::from([
        ("marcq en baroeul", (50.6767, 3.0946)),
        ("lyon", (45.7640, 4.8357)),
        ("lille", (50.6292, 3.0573)),
        ("paris", (48.8566, 2.3522)),
        ("marseille", (43.2965, 5.3698)),
        ("bordeaux", (44.8378, -0.5792)),
    ]);
    if let Some(coords) = known.get(normalize_city(city).as_str()) {
        return *coords;
    }
    (48.8566, 2.3522)
}

fn normalize_city(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'a',
            'ç' | 'Ç' => 'c',
            'è' | 'é' | 'ê' | 'ë' | 'È' | 'É' | 'Ê' | 'Ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' | 'Ì' | 'Í' | 'Î' | 'Ï' => 'i',
            'ñ' | 'Ñ' => 'n',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'o',
            'ù' | 'ú' | 'û' | 'ü' | 'Ù' | 'Ú' | 'Û' | 'Ü' => 'u',
            'ý' | 'ÿ' | 'Ý' => 'y',
            '\'' | '-' | '_' => ' ',
            _ => ch.to_ascii_lowercase(),
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
