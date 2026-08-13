//! One-shot web search worker: brief egress window, then gone.
//!
//! Not attached to smith-brain-net (internal). Uses the default bridge so it can
//! reach the internet, labeled under compose project `smith`, then `--rm`.

use super::compose::push_compose_service_labels;
use super::*;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

pub const SEARCH_IMAGE: &str = "alpine:3.20";
pub const SEARCH_CONTAINER_PREFIX: &str = "smith-search-";

const MAX_RESULT_CHARS: usize = 8_000;

/// Run a temporary search container and return declassified text for the brain.
pub fn run_oneshot_web_search(query: &str) -> Result<String, String> {
    check_docker_available()?;

    let q = query.trim();
    if q.is_empty() {
        return Err("Search query is empty".to_string());
    }

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let name = format!("{}{}", SEARCH_CONTAINER_PREFIX, ts);

    let ddg_url = format!(
        "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
        urlencode(q)
    );

    let wttr_url = if looks_like_weather_query(q) {
        let place = guess_weather_place(q);
        Some(format!(
            "https://wttr.in/{}?format=%l:+%t+%h+%c+%C+%w",
            urlencode(&place)
        ))
    } else {
        None
    };

    let mut args = vec![
        "run".to_string(),
        "--rm".to_string(),
        "--name".to_string(),
        name.clone(),
        "--network".to_string(),
        "bridge".to_string(),
        "--restart".to_string(),
        "no".to_string(),
        "-e".to_string(),
        format!("SMITH_DDG_URL={}", ddg_url),
    ];
    if let Some(ref w) = wttr_url {
        args.push("-e".to_string());
        args.push(format!("SMITH_WTTR_URL={}", w));
    }
    push_compose_service_labels(&mut args, "search");
    args.extend([
        SEARCH_IMAGE.to_string(),
        "sh".to_string(),
        "-c".to_string(),
        r#"echo '=== ddg ==='
wget -qO- --timeout=20 -U 'smith/0.3' "$SMITH_DDG_URL" || echo '(ddg failed)'
echo
if [ -n "$SMITH_WTTR_URL" ]; then
  echo '=== wttr ==='
  wget -qO- --timeout=15 -U 'smith/0.3' "$SMITH_WTTR_URL" || echo '(wttr failed)'
fi
"#
        .to_string(),
    ]);

    println!("  Opening brief search window (then shutdown)…");
    let output = Command::new("docker")
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to start search container: {}", e))?;

    // Belt-and-suspenders: --rm should drop it, but never leave a search box running.
    let _ = Command::new("docker").args(["rm", "-f", &name]).output();

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Search window failed: {}",
            err.trim().chars().take(500).collect::<String>()
        ));
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    let summarized = summarize_search_output(&raw);
    if summarized.trim().is_empty() {
        Err("Search window returned no usable results".to_string())
    } else {
        Ok(truncate_chars(&summarized, MAX_RESULT_CHARS))
    }
}

fn summarize_search_output(raw: &str) -> String {
    let mut ddg_raw = String::new();
    let mut wttr_raw = String::new();
    let mut section = "";
    for line in raw.lines() {
        let t = line.trim();
        if t == "=== ddg ===" {
            section = "ddg";
            continue;
        }
        if t == "=== wttr ===" {
            section = "wttr";
            continue;
        }
        match section {
            "ddg" => {
                if !ddg_raw.is_empty() {
                    ddg_raw.push('\n');
                }
                ddg_raw.push_str(line);
            }
            "wttr" => {
                if !wttr_raw.is_empty() {
                    wttr_raw.push('\n');
                }
                wttr_raw.push_str(line);
            }
            _ => {}
        }
    }

    let mut out = String::new();
    let ddg = summarize_ddg_json(ddg_raw.trim());
    if !ddg.is_empty() {
        out.push_str("Web:\n");
        out.push_str(&ddg);
    }
    let wttr = wttr_raw.trim();
    if !wttr.is_empty() && !wttr.contains("(wttr failed)") {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str("Live weather:\n");
        out.push_str(wttr);
    }
    if out.trim().is_empty() {
        truncate_chars(raw.trim(), 2_000)
    } else {
        out
    }
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
        truncate_chars(raw, 1_500)
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
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{:02X}", b)),
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
    fn weather_place_corona() {
        assert_eq!(
            guess_weather_place("what is the weather like today in Corona California"),
            "Corona California"
        );
        assert!(looks_like_weather_query("weather in Corona?"));
    }

    #[test]
    fn summarize_ddg_extracts_abstract() {
        let json = r#"{"AbstractText":"Hot day.","Answer":"","RelatedTopics":[{"Text":"Heat wave warning"}]}"#;
        let s = summarize_ddg_json(json);
        assert!(s.contains("Hot day"));
        assert!(s.contains("Heat wave"));
    }
}
