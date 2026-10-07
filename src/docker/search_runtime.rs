//! Web search for the sealed brain: local SearXNG + optional weather fetches.
//!
//! General web results come from always-on `smith-searxng` (loopback :4792).
//! Weather still hits wttr / Open-Meteo directly (small public APIs).
//! Results are declassified text only — the brain never browses.

use super::searx_runtime::{self, SEARXNG_PORT};
use super::*;
use serde_json::Value;

const MAX_RESULT_CHARS: usize = 8_000;
const FETCH_UA: &str = "curl/8.4.0";

/// Run search and return declassified text for the brain.
pub fn run_oneshot_web_search(query: &str) -> Result<String, String> {
    check_docker_available()?;

    let q = query.trim();
    if q.is_empty() {
        return Err("Search query is empty".to_string());
    }

    println!("  Opening search via local SearXNG (:{SEARXNG_PORT})…");

    let mut parts: Vec<String> = Vec::new();

    if looks_like_weather_query(q) {
        let place = guess_weather_place(q);
        if let Some(w) = fetch_weather(&place) {
            parts.push(w);
        }
    }

    match searx_runtime::searxng_search_json(q) {
        Ok(json) => {
            let web = summarize_searx_json(&json);
            if !web.is_empty() {
                parts.push(format!("Web (SearXNG):\n{web}"));
            } else if let Some(ddg) = fetch_ddg_fallback(q) {
                parts.push(format!("Web (DDG fallback):\n{ddg}"));
            }
        }
        Err(e) => {
            println!("  SearXNG unavailable ({e}); trying DDG Instant Answer fallback…");
            if let Some(ddg) = fetch_ddg_fallback(q) {
                parts.push(format!("Web (DDG fallback):\n{ddg}"));
            } else if parts.is_empty() {
                return Err(format!(
                    "Search failed: SearXNG error ({e}); DDG also empty. \
                     Start Docker and retry, or open http://127.0.0.1:{SEARXNG_PORT}"
                ));
            }
        }
    }

    let summarized = parts.join("\n\n");
    if summarized.trim().is_empty() {
        Err("Search returned no usable results".to_string())
    } else {
        let preview: String = summarized.chars().take(120).collect();
        println!("  Search declassified: {}…", preview.replace('\n', " "));
        Ok(truncate_chars(&summarized, MAX_RESULT_CHARS))
    }
}

fn summarize_searx_json(raw: &str) -> String {
    let Ok(v) = serde_json::from_str::<Value>(raw) else {
        return truncate_chars(raw.trim(), 2_000);
    };
    let mut lines = Vec::new();

    if let Some(answer) = v
        .get("answers")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
        .and_then(|x| {
            x.as_str().map(|s| s.to_string()).or_else(|| {
                x.get("answer")
                    .and_then(Value::as_str)
                    .map(|s| s.to_string())
            })
        })
        .filter(|s| !s.is_empty())
    {
        lines.push(format!("Answer: {answer}"));
    }

    if let Some(results) = v.get("results").and_then(Value::as_array) {
        for (i, item) in results.iter().take(8).enumerate() {
            let title = item
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            let url = item.get("url").and_then(Value::as_str).unwrap_or("").trim();
            let content = item
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            if title.is_empty() && content.is_empty() {
                continue;
            }
            let mut block = format!("{}. {}", i + 1, title);
            if !url.is_empty() {
                block.push_str(&format!("\n   {url}"));
            }
            if !content.is_empty() {
                block.push_str(&format!("\n   {}", truncate_chars(content, 280)));
            }
            lines.push(block);
        }
    }

    if let Some(infoboxes) = v.get("infoboxes").and_then(Value::as_array) {
        for box_ in infoboxes.iter().take(2) {
            if let Some(content) = box_.get("content").and_then(Value::as_str) {
                let t = content.trim();
                if !t.is_empty() {
                    lines.push(format!("Infobox: {}", truncate_chars(t, 400)));
                }
            }
        }
    }

    lines.join("\n\n")
}

fn fetch_ddg_fallback(q: &str) -> Option<String> {
    let url = format!(
        "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
        urlencode(q)
    );
    let raw = http_get(&url)?;
    let s = summarize_ddg_json(&raw);
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn fetch_weather(place: &str) -> Option<String> {
    let mut out = String::new();
    let wttr_url = format!("https://wttr.in/{}?format=3", urlencode(place));
    if let Some(wttr) = http_get(&wttr_url) {
        let t = wttr.trim();
        if !t.is_empty() && !looks_like_error_page(t) {
            out.push_str("Live weather (wttr):\n");
            out.push_str(t);
        }
    }

    let geo_url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=3&language=en&format=json",
        urlencode(place)
    );
    if let Some(geo) = http_get(&geo_url) {
        if let Some((lat, lon, label)) = parse_geo_first(&geo) {
            let meteo_url = format!(
                "https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}\
                 &current=temperature_2m,relative_humidity_2m,weather_code,wind_speed_10m\
                 &temperature_unit=fahrenheit&wind_speed_unit=mph&timezone=auto"
            );
            if let Some(meteo) = http_get(&meteo_url) {
                let sum =
                    summarize_open_meteo(&format!("place={label} lat={lat} lon={lon}\n{meteo}"));
                if !sum.is_empty() {
                    if !out.is_empty() {
                        out.push_str("\n\n");
                    }
                    out.push_str("Live weather (open-meteo):\n");
                    out.push_str(&sum);
                }
            }
        }
    }

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn parse_geo_first(geo: &str) -> Option<(String, String, String)> {
    let v: Value = serde_json::from_str(geo).ok()?;
    let first = v.get("results")?.as_array()?.first()?;
    let lat = first.get("latitude")?.to_string();
    let lon = first.get("longitude")?.to_string();
    let name = first
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let admin = first.get("admin1").and_then(Value::as_str).unwrap_or("");
    let country = first.get("country").and_then(Value::as_str).unwrap_or("");
    let label = [name.as_str(), admin, country]
        .iter()
        .filter(|s| !s.is_empty())
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    Some((lat, lon, label))
}

fn http_get(url: &str) -> Option<String> {
    let out = Command::new("curl")
        .args(["-fsSL", "--max-time", "20", "-A", FETCH_UA, url])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

fn looks_like_error_page(s: &str) -> bool {
    let l = s.to_lowercase();
    l.contains("<html") || l.contains("502 bad gateway") || l.contains("503 service")
}

fn summarize_open_meteo(raw: &str) -> String {
    let mut place = String::new();
    let mut json_part = String::new();
    for line in raw.lines() {
        let t = line.trim();
        if t.starts_with("place=") {
            place = t.trim_start_matches("place=").to_string();
        } else if t.starts_with('{') {
            json_part = t.to_string();
        }
    }
    if json_part.is_empty() {
        // whole blob might be JSON
        if raw.trim().starts_with('{') {
            json_part = raw.trim().to_string();
        } else {
            return String::new();
        }
    }
    let Ok(v) = serde_json::from_str::<Value>(&json_part) else {
        return String::new();
    };
    let current = match v.get("current") {
        Some(c) => c,
        None => return String::new(),
    };
    let temp = current
        .get("temperature_2m")
        .and_then(Value::as_f64)
        .map(|t| format!("{t:.0}°F"));
    let humidity = current
        .get("relative_humidity_2m")
        .and_then(Value::as_f64)
        .map(|h| format!("humidity {h:.0}%"));
    let wind = current
        .get("wind_speed_10m")
        .and_then(Value::as_f64)
        .map(|w| format!("wind {w:.0} mph"));
    let code = current
        .get("weather_code")
        .and_then(Value::as_i64)
        .map(weather_code_label);
    let mut bits: Vec<String> = Vec::new();
    if !place.is_empty() {
        bits.push(place);
    }
    if let Some(t) = temp {
        bits.push(t);
    }
    if let Some(c) = code {
        bits.push(c);
    }
    if let Some(h) = humidity {
        bits.push(h);
    }
    if let Some(w) = wind {
        bits.push(w);
    }
    bits.join(" · ")
}

fn weather_code_label(code: i64) -> String {
    let label = match code {
        0 => "Clear",
        1 | 2 => "Partly cloudy",
        3 => "Overcast",
        45 | 48 => "Fog",
        51 | 53 | 55 => "Drizzle",
        61 | 63 | 65 => "Rain",
        71 | 73 | 75 => "Snow",
        80..=82 => "Rain showers",
        95 | 96 | 99 => "Thunderstorm",
        _ => "Mixed conditions",
    };
    format!("{label} (code {code})")
}

fn summarize_ddg_json(raw: &str) -> String {
    if raw.is_empty() || raw.contains("(ddg failed)") {
        return String::new();
    }
    let Ok(v) = serde_json::from_str::<Value>(raw) else {
        return truncate_chars(raw, 2_000);
    };
    let mut parts = Vec::new();
    for key in ["Answer", "AbstractText", "Abstract", "Definition"] {
        if let Some(s) = v.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()) {
            parts.push(s.to_string());
        }
    }
    if let Some(arr) = v.get("RelatedTopics").and_then(Value::as_array) {
        for item in arr.iter().take(6) {
            if let Some(t) = item
                .get("Text")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
            {
                parts.push(t.to_string());
            }
        }
    }
    if parts.is_empty() {
        String::new()
    } else {
        parts.join("\n\n")
    }
}

fn looks_like_weather_query(q: &str) -> bool {
    let l = q.to_lowercase();
    l.contains("weather")
        || l.contains("forecast")
        || l.contains("temperature")
        || l.contains("humidity")
        || l.contains("heat wave")
        || l.contains("heatwave")
}

fn guess_weather_place(q: &str) -> String {
    const DROP: &[&str] = &[
        "what",
        "whats",
        "what's",
        "is",
        "the",
        "like",
        "today",
        "now",
        "current",
        "weather",
        "forecast",
        "temperature",
        "humidity",
        "in",
        "for",
        "a",
        "an",
        "how",
        "outside",
    ];
    let words: Vec<&str> = q
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .filter(|w| !DROP.contains(&w.to_lowercase().as_str()))
        .collect();
    if words.is_empty() {
        q.to_string()
    } else {
        words.join(" ")
    }
}

fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match *b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(*b));
            }
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_searx_extracts_results() {
        let json = r#"{
          "query": "rust lang",
          "results": [
            {"title": "Rust", "url": "https://www.rust-lang.org/", "content": "A language."},
            {"title": "Book", "url": "https://doc.rust-lang.org/book/", "content": "The book."}
          ]
        }"#;
        let s = summarize_searx_json(json);
        assert!(s.contains("Rust"));
        assert!(s.contains("rust-lang.org"));
    }

    #[test]
    fn summarize_ddg_empty_instant_answer_is_blank() {
        let json = r#"{"Abstract":"","Answer":"","RelatedTopics":[]}"#;
        assert!(summarize_ddg_json(json).is_empty());
    }

    #[test]
    fn guess_place_from_weather_question() {
        assert_eq!(
            guess_weather_place("what's the weather in Austin"),
            "Austin"
        );
    }
}
