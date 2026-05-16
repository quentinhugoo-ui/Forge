use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

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
    println!(
        "Forge real-estate resolver listening on http://0.0.0.0:{port}/api/agency/resolve"
    );
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
    let mut buffer = [0_u8; 32 * 1024];
    let bytes_read = stream
        .read(&mut buffer)
        .map_err(|err| format!("read request: {err}"))?;
    if bytes_read == 0 {
        return Ok(());
    }
    let request = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();
    let request_line = request.lines().next().unwrap_or_default().to_string();
    let mut content_length = 0_usize;
    let mut auth_header = String::new();
    for line in request.lines().skip(1) {
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
    let body = request
        .split("\r\n\r\n")
        .nth(1)
        .or_else(|| request.split("\n\n").nth(1))
        .unwrap_or_default();
    let body = if content_length > 0 && body.len() >= content_length {
        &body[..content_length]
    } else {
        body
    };

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

    if request_line.starts_with("GET /health ") {
        return write_json_response(
            &mut stream,
            200,
            &json!({ "ok": true, "service": "forge-real-estate-resolver" }),
        );
    }

    if !request_line.starts_with("POST /api/agency/resolve ") {
        return write_json_response(
            &mut stream,
            404,
            &json!({ "error": "not_found", "message": request_line }),
        );
    }
    let payload: ResolveRequest = serde_json::from_str(body)
        .map_err(|err| format!("decode request json: {err}"))?;
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
    let query = payload.query.clone().unwrap_or_else(|| {
        format!("{agency_name} {city} agence immobiliere")
    });

    let resolved = match google_places_api_key() {
        Some(api_key) => google_places_text_search_contact(&api_key, &query)
            .unwrap_or_else(|_| build_agency_payload(&agency_name, &city)),
        None => build_agency_payload(&agency_name, &city),
    };
    let source = resolved.source.clone();
    write_json_response(
        &mut stream,
        200,
        &json!({
            "agency": resolved,
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

fn write_json_response(
    stream: &mut TcpStream,
    status: u16,
    payload: &serde_json::Value,
) -> Result<(), String> {
    let reason = match status {
        200 => "OK",
        401 => "Unauthorized",
        404 => "Not Found",
        _ => "OK",
    };
    let body = serde_json::to_string(payload).map_err(|err| format!("encode response: {err}"))?;
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|err| format!("write response: {err}"))?;
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

fn google_places_text_search_contact(
    api_key: &str,
    text_query: &str,
) -> Result<AgencyPayload, String> {
    let payload: Option<Value> = tauri::async_runtime::block_on(async move {
            let client = reqwest::Client::builder()
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
                .await
                .map_err(|err| format!("google places request: {err}"))?;
            if !response.status().is_success() {
                let code = response.status().as_u16();
                let body = response.text().await.unwrap_or_default();
                return Err(format!(
                    "google places status {code}: {}",
                    truncate_for_log(&body, 220)
                ));
            }
            let payload: Value = response
                .json()
                .await
                .map_err(|err| format!("google places parse: {err}"))?;
            let place = payload
                .get("places")
                .and_then(|places| places.as_array())
                .and_then(|places| places.first())
                .cloned();
            Ok(place)
        })?;

    let Some(place) = payload else {
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
