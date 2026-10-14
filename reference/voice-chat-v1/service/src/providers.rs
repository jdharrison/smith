//! Fixed loopback, tool-free provider transport. No environment endpoint overrides.
use crate::app::{AppError, AppResult, Message};
use crate::Result;
use base64::{engine::general_purpose::STANDARD, Engine};
use reqwest::blocking::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::Read;
use std::time::{Duration, Instant};

const INPUT_AUDIO: usize = 2 * 1024 * 1024;
const OUTPUT_AUDIO: usize = 2_880_044;
const JSON_LIMIT: usize = 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Audio {
    pub mime: String,
    pub data_base64: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
}
impl Audio {
    fn bytes(&self, limit: usize, rate: u32) -> AppResult<Vec<u8>> {
        if self.mime != "audio/wav"
            || self.sample_rate.is_some_and(|r| r != rate)
            || self.data_base64.len() > limit.div_ceil(3) * 4
        {
            return Err(error("AUDIO_LIMIT", "Audio format or size rejected"));
        }
        let bytes = STANDARD
            .decode(&self.data_base64)
            .map_err(|_| error("INVALID_AUDIO", "Invalid base64 audio"))?;
        validate_wav(&bytes, rate, limit)?;
        Ok(bytes)
    }
    pub fn input_bytes(&self) -> AppResult<Vec<u8>> {
        if self.sample_rate.is_some() {
            return Err(error(
                "INVALID_AUDIO",
                "Input audio accepts only mime and data_base64",
            ));
        }
        self.bytes(INPUT_AUDIO, 16000)
    }
    pub fn output_bytes(&self) -> AppResult<Vec<u8>> {
        if self.sample_rate != Some(24000) {
            return Err(error("INVALID_AUDIO", "Output rate must be 24000"));
        }
        self.bytes(OUTPUT_AUDIO, 24000)
    }
    pub fn output(bytes: &[u8]) -> AppResult<Self> {
        validate_wav(bytes, 24000, OUTPUT_AUDIO)?;
        Ok(Self {
            mime: "audio/wav".into(),
            sample_rate: Some(24000),
            data_base64: STANDARD.encode(bytes),
        })
    }
}
fn error(code: &str, msg: &str) -> AppError {
    AppError::new(code, msg)
}
/// Parse RIFF chunks rather than trusting a fixed header or Content-Length.
pub fn validate_wav(bytes: &[u8], rate: u32, limit: usize) -> AppResult<u64> {
    let bad = || {
        error(
            "INVALID_AUDIO",
            "Expected bounded mono PCM16 WAV at the required rate",
        )
    };
    if bytes.len() > limit
        || bytes.len() < 44
        || &bytes[..4] != b"RIFF"
        || &bytes[8..12] != b"WAVE"
        || u32::from_le_bytes(bytes[4..8].try_into().map_err(|_| bad())?) as usize + 8
            != bytes.len()
    {
        return Err(bad());
    }
    let mut offset = 12;
    let mut format = false;
    let mut data = None;
    while offset < bytes.len() {
        if offset + 8 > bytes.len() {
            return Err(bad());
        }
        let n = u32::from_le_bytes(
            bytes[offset + 4..offset + 8]
                .try_into()
                .map_err(|_| bad())?,
        ) as usize;
        let end = offset
            .checked_add(8)
            .and_then(|o| o.checked_add(n))
            .ok_or_else(bad)?;
        if end > bytes.len() {
            return Err(bad());
        }
        let chunk = &bytes[offset + 8..end];
        match &bytes[offset..offset + 4] {
            b"fmt " => {
                if format
                    || n != 16
                    || chunk[0..2] != 1u16.to_le_bytes()
                    || chunk[2..4] != 1u16.to_le_bytes()
                    || chunk[4..8] != rate.to_le_bytes()
                    || chunk[8..12] != (rate * 2).to_le_bytes()
                    || chunk[12..14] != 2u16.to_le_bytes()
                    || chunk[14..16] != 16u16.to_le_bytes()
                {
                    return Err(bad());
                }
                format = true;
            }
            b"data" => {
                if data.is_some() || n == 0 || n % 2 != 0 || n as u64 > rate as u64 * 2 * 60 {
                    return Err(bad());
                }
                data = Some(n);
            }
            _ => {}
        }
        offset = end.checked_add(n % 2).ok_or_else(bad)?;
        if offset > bytes.len() {
            return Err(bad());
        }
    }
    if !format {
        return Err(bad());
    }
    let n = data.ok_or_else(bad)?;
    Ok(n as u64 * 1000 / (rate as u64 * 2))
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub name: String,
    pub digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    pub ready: bool,
    #[serde(deserialize_with = "crate::app::required_nullable")]
    pub error: Option<AppError>,
}
impl Default for Observation {
    fn default() -> Self {
        Self {
            ready: false,
            error: Some(error(
                "NOT_OBSERVED",
                "Provider has not been explicitly refreshed",
            )),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SttObservation {
    pub ready: bool,
    #[serde(deserialize_with = "crate::app::required_nullable")]
    pub error: Option<AppError>,
    #[serde(deserialize_with = "crate::app::required_nullable")]
    pub model: Option<String>,
}
impl Default for SttObservation {
    fn default() -> Self {
        Self {
            ready: false,
            error: Observation::default().error,
            model: None,
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Observations {
    pub llm: Observation,
    pub stt: SttObservation,
    pub tts: Observation,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    pub models: Vec<Model>,
    pub voices: Vec<String>,
    pub providers: Observations,
}
impl Inventory {
    pub(crate) fn valid(&self) -> bool {
        let mut models = HashSet::new();
        let mut voices = HashSet::new();
        self.models.len() <= 256
            && self.voices.len() <= 256
            && self.models.iter().all(|m| {
                valid_tag_name(&m.name) && valid_digest(&m.digest) && models.insert(&m.name)
            })
            && self
                .voices
                .iter()
                .all(|v| valid_label(v) && voices.insert(v))
            && valid_observation(self.providers.llm.ready, self.providers.llm.error.as_ref())
            && valid_observation(self.providers.stt.ready, self.providers.stt.error.as_ref())
            && valid_observation(self.providers.tts.ready, self.providers.tts.error.as_ref())
            && self
                .providers
                .stt
                .model
                .as_ref()
                .is_none_or(|m| valid_label(m))
            && (!self.providers.stt.ready || self.providers.stt.model.is_some())
            && (self.providers.llm.ready || self.models.is_empty())
            && if self.providers.tts.ready {
                !self.voices.is_empty()
            } else {
                self.voices.is_empty()
            }
            && crate::app::reply_fits(self)
    }
}
fn valid_observation(ready: bool, error: Option<&AppError>) -> bool {
    if ready {
        error.is_none()
    } else {
        error.is_some_and(AppError::valid)
    }
}
fn valid_label(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 256 && !s.chars().any(char::is_control)
}
fn valid_tag_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 256
        && name
            .rsplit_once(':')
            .is_some_and(|(model, tag)| !model.is_empty() && !tag.is_empty())
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"/:_.-".contains(&c))
        && !name.to_ascii_lowercase().contains("cloud")
        && !name.starts_with("http:")
        && !name.starts_with("https:")
}
fn valid_digest(digest: &str) -> bool {
    digest.len() == 64 && digest.bytes().all(|c| c.is_ascii_hexdigit())
}
/// Library injection seam; production always constructs the fixed-endpoint Providers.
pub trait Provider: Send + Sync + 'static {
    fn inventory(&self) -> AppResult<Inventory>;
    fn chat(&self, model: &str, messages: &[Message]) -> AppResult<String>;
    fn transcribe(&self, audio: &[u8]) -> AppResult<Value>;
    fn speak(&self, text: &str, voice: Option<&str>) -> AppResult<Audio>;
}
pub struct Providers {
    client: Client,
}
impl Providers {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| "Cannot build fixed loopback provider transport")?;
        Ok(Self { client })
    }
    fn get(&self, url: &str, deadline: Instant) -> AppResult<Value> {
        self.json(
            self.client
                .get(url)
                .timeout(remaining(deadline)?.min(Duration::from_secs(10)))
                .send()
                .map_err(|_| {
                    error(
                        "PROVIDER_UNAVAILABLE",
                        "Local provider unavailable or timed out",
                    )
                })?,
        )
    }
    fn json(&self, response: Response) -> AppResult<Value> {
        let bytes = body(response, JSON_LIMIT)?;
        serde_json::from_slice(&bytes)
            .map_err(|_| error("PROVIDER_PROTOCOL", "Invalid local provider JSON"))
    }
    fn tags(&self, deadline: Instant) -> AppResult<Vec<Model>> {
        eligible_tags(&self.get("http://127.0.0.1:11434/api/tags", deadline)?)
    }
    fn health(&self, stt: bool, deadline: Instant) -> AppResult<Health> {
        let v = self.get(
            if stt {
                "http://127.0.0.1:4890/v1/health"
            } else {
                "http://127.0.0.1:4891/v1/health"
            },
            deadline,
        )?;
        let h: Health = serde_json::from_value(v)
            .map_err(|_| error("PROVIDER_PROTOCOL", "Invalid speech health schema"))?;
        if h.schema_version != 1
            || h.service
                != if stt {
                    "smith-next-stt"
                } else {
                    "smith-next-tts"
                }
            || h.voices.len() > 256
            || h.voices.iter().any(|v| v.is_empty() || v.len() > 256)
            || h.model.as_ref().is_some_and(|v| v.len() > 256)
            || (h.ready
                && (h.error_code.is_some()
                    || (stt && h.model.as_ref().is_none_or(|m| m.is_empty()))
                    || (!stt && h.voices.is_empty())))
        {
            return Err(error(
                "PROVIDER_PROTOCOL",
                "Speech provider identity or readiness rejected",
            ));
        }
        Ok(h)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Health {
    schema_version: u32,
    service: String,
    ready: bool,
    model: Option<String>,
    voices: Vec<String>,
    error_code: Option<String>,
}
impl Provider for Providers {
    fn inventory(&self) -> AppResult<Inventory> {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut i = Inventory::default();
        match self.tags(deadline) {
            Ok(models) => {
                i.models = models;
                i.providers.llm = Observation {
                    ready: true,
                    error: None,
                };
            }
            Err(e) => i.providers.llm.error = Some(e),
        }
        match self.health(true, deadline) {
            Ok(h) => {
                i.providers.stt = SttObservation {
                    ready: h.ready,
                    model: h.model,
                    error: if h.ready {
                        None
                    } else {
                        Some(error("STT_NOT_READY", "STT local weights are not ready"))
                    },
                }
            }
            Err(e) => {
                i.providers.stt = SttObservation {
                    ready: false,
                    model: None,
                    error: Some(e),
                }
            }
        }
        match self.health(false, deadline) {
            Ok(h) => {
                i.providers.tts = Observation {
                    ready: h.ready,
                    error: if h.ready {
                        None
                    } else {
                        Some(error("TTS_NOT_READY", "TTS local weights are not ready"))
                    },
                };
                if h.ready {
                    i.voices = h.voices;
                }
            }
            Err(e) => {
                i.providers.tts = Observation {
                    ready: false,
                    error: Some(e),
                }
            }
        }
        Ok(i)
    }
    fn chat(&self, model: &str, messages: &[Message]) -> AppResult<String> {
        let deadline = Instant::now() + Duration::from_secs(120);
        if !self.tags(deadline)?.iter().any(|m| m.name == model) {
            return Err(error(
                "MODEL_UNAVAILABLE",
                "Exact eligible local model is no longer installed",
            ));
        }
        if messages.len() > 33
            || messages.iter().map(|m| m.content.len()).sum::<usize>() > 64 * 1024
            || messages
                .iter()
                .any(|m| !["user", "assistant"].contains(&m.role.as_str()))
        {
            return Err(error("CONTEXT_LIMIT", "Chat context exceeds bounds"));
        }
        let response=self.client.post("http://127.0.0.1:11434/api/chat").timeout(remaining(deadline)?).json(&json!({"model":model,"messages":messages,"stream":false,"options":{"num_ctx":4096,"num_predict":2048}})).send().map_err(|_|error("LLM_UNAVAILABLE","Local generation unavailable or timed out; no retry"))?;
        chat_result(model, self.json(response)?)
    }
    fn transcribe(&self, audio: &[u8]) -> AppResult<Value> {
        let duration = validate_wav(audio, 16000, INPUT_AUDIO)?;
        let response = self
            .client
            .post("http://127.0.0.1:4890/v1/transcribe")
            .header(reqwest::header::CONTENT_TYPE, "audio/wav")
            .timeout(Duration::from_secs(90))
            .body(audio.to_vec())
            .send()
            .map_err(|_| {
                error(
                    "STT_UNAVAILABLE",
                    "Transcription unavailable or timed out; no retry",
                )
            })?;
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Transcript {
            schema_version: u32,
            service: String,
            text: String,
            language: String,
            duration_ms: u64,
        }
        let t: Transcript = serde_json::from_value(self.json(response)?)
            .map_err(|_| error("PROVIDER_PROTOCOL", "Invalid transcript schema"))?;
        if t.schema_version != 1
            || t.service != "smith-next-stt"
            || t.text.len() > 16 * 1024
            || t.language.len() > 64
            || t.duration_ms > 60_000
            || t.duration_ms.abs_diff(duration) > 1
        {
            return Err(error(
                "PROVIDER_PROTOCOL",
                "Transcript bounds or identity rejected",
            ));
        }
        Ok(json!({"text":t.text,"language":t.language,"duration_ms":t.duration_ms}))
    }
    fn speak(&self, text: &str, voice: Option<&str>) -> AppResult<Audio> {
        if text.trim().is_empty() || text.len() > 4096 {
            return Err(error("TTS_INPUT_LIMIT", "TTS text exceeds bounds"));
        }
        let voice = voice.ok_or_else(|| error("VOICE_UNAVAILABLE", "No voice selected"))?;
        let deadline = Instant::now() + Duration::from_secs(60);
        let health = self.health(false, deadline)?;
        if !health.ready || !health.voices.iter().any(|v| v == voice) {
            return Err(error(
                "VOICE_UNAVAILABLE",
                "Selected voice is not observed ready",
            ));
        }
        let response = self
            .client
            .post("http://127.0.0.1:4891/v1/speak")
            .timeout(remaining(deadline)?)
            .json(&json!({"schema_version":1,"text":text,"voice":voice}))
            .send()
            .map_err(|_| {
                error(
                    "TTS_UNAVAILABLE",
                    "Synthesis unavailable or timed out; text retained",
                )
            })?;
        if response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(';').next().unwrap_or("").trim())
            != Some("audio/wav")
        {
            return Err(error("PROVIDER_PROTOCOL", "TTS returned non-WAV content"));
        }
        Audio::output(&body(response, OUTPUT_AUDIO)?)
    }
}
fn chat_result(model: &str, v: Value) -> AppResult<String> {
    let mut metadata = v.clone();
    if let Some(object) = metadata.as_object_mut() {
        object.remove("message");
    }
    if v.get("model").and_then(Value::as_str) != Some(model)
        || v.get("done") != Some(&Value::Bool(true))
        || v.get("error").is_some_and(|e| !e.is_null())
        || remote_metadata(&metadata)
        || v.pointer("/message/role").and_then(Value::as_str) != Some("assistant")
        || [v.pointer("/message/tool_calls"), v.get("tool_calls")]
            .into_iter()
            .flatten()
            .any(|t| !t.is_null() && t.as_array().is_none_or(|a| !a.is_empty()))
    {
        return Err(error(
            "PROVIDER_PROTOCOL",
            "Model identity or tool-free response rejected",
        ));
    }
    let content = v
        .pointer("/message/content")
        .and_then(Value::as_str)
        .ok_or_else(|| error("PROVIDER_PROTOCOL", "Missing assistant text"))?;
    if content.trim().is_empty() || content.len() > 64 * 1024 {
        return Err(error("MODEL_RESULT_LIMIT", "Assistant text exceeds bounds"));
    }
    Ok(content.into())
}
fn remaining(deadline: Instant) -> AppResult<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| error("PROVIDER_TIMEOUT", "Operation budget exhausted; no retry"))
}
fn body(response: Response, limit: usize) -> AppResult<Vec<u8>> {
    if !response.status().is_success() {
        return Err(error("PROVIDER_HTTP", "Local provider refused request"));
    }
    let mut bytes = Vec::new();
    response
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| {
            error(
                "PROVIDER_UNAVAILABLE",
                "Provider body unavailable or timed out",
            )
        })?;
    if bytes.len() > limit {
        return Err(error("PROVIDER_BODY_LIMIT", "Provider body exceeds limit"));
    }
    Ok(bytes)
}
/// Reject cloud/remote names and metadata; only explicitly named installed tags are eligible.
pub fn eligible_tags(v: &Value) -> AppResult<Vec<Model>> {
    let entries = v
        .get("models")
        .and_then(Value::as_array)
        .ok_or_else(|| error("PROVIDER_PROTOCOL", "Missing installed model inventory"))?;
    if entries.len() > 256 {
        return Err(error("PROVIDER_BODY_LIMIT", "Too many installed models"));
    }
    let mut models = Vec::new();
    for m in entries {
        let Some(name) = m.get("name").and_then(Value::as_str) else {
            return Err(error("PROVIDER_PROTOCOL", "Invalid installed tag"));
        };
        let Some(digest) = m.get("digest").and_then(Value::as_str) else {
            return Err(error("PROVIDER_PROTOCOL", "Invalid installed digest"));
        };
        if !valid_tag_name(name) || remote_metadata(m) {
            continue;
        }
        if !valid_digest(digest) {
            return Err(error("PROVIDER_PROTOCOL", "Invalid installed digest"));
        }
        if m.get("model")
            .and_then(Value::as_str)
            .is_some_and(|s| s != name)
        {
            continue;
        }
        if !m
            .get("details")
            .and_then(|d| d.get("format"))
            .and_then(Value::as_str)
            .is_some_and(|f| f == "gguf" || f == "safetensors")
        {
            continue;
        }
        if models.iter().any(|x: &Model| x.name == name) {
            return Err(error("PROVIDER_PROTOCOL", "Duplicate installed tag"));
        }
        models.push(Model {
            name: name.into(),
            digest: digest.into(),
        });
    }
    Ok(models)
}
fn remote_metadata(v: &Value) -> bool {
    match v {
        Value::Object(o) => o.iter().any(|(k, v)| {
            let k = k.to_ascii_lowercase();
            ((k.contains("remote") || k.contains("cloud"))
                && !v.is_null()
                && v != &Value::Bool(false)
                && v != &Value::String(String::new()))
                || remote_metadata(v)
        }),
        Value::Array(a) => a.iter().any(remote_metadata),
        Value::String(s) => {
            s.eq_ignore_ascii_case("cloud")
                || s.eq_ignore_ascii_case("remote")
                || s.starts_with("http://")
                || s.starts_with("https://")
        }
        _ => false,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;
    fn fixture(response: Vec<u8>, delay: Duration) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0; 4096];
            let _ = stream.read(&mut request);
            std::thread::sleep(delay);
            let _ = stream.write_all(&response);
        });
        (url, thread)
    }
    #[test]
    fn transport_refuses_redirect_and_bounds_chunked_bodies() {
        let providers = Providers::new().unwrap();
        let (url,thread)=fixture(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/no-follow\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),Duration::ZERO);
        let response = providers.client.get(url).send().unwrap();
        assert_eq!(response.status(), 302);
        assert_eq!(body(response, 1024).unwrap_err().code, "PROVIDER_HTTP");
        thread.join().unwrap();
        let (url,thread)=fixture(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n10\r\n0123456789abcdef\r\n0\r\n\r\n".to_vec(),Duration::ZERO);
        assert_eq!(
            body(providers.client.get(url).send().unwrap(), 8)
                .unwrap_err()
                .code,
            "PROVIDER_BODY_LIMIT"
        );
        thread.join().unwrap();
    }
    #[test]
    fn transport_budget_is_finite_and_expired_budget_cannot_retry() {
        let providers = Providers::new().unwrap();
        let (url, thread) = fixture(
            b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec(),
            Duration::from_millis(150),
        );
        let started = Instant::now();
        assert!(providers
            .client
            .get(url)
            .timeout(Duration::from_millis(20))
            .send()
            .is_err());
        assert!(started.elapsed() < Duration::from_secs(1));
        thread.join().unwrap();
        assert_eq!(
            remaining(Instant::now() - Duration::from_secs(1))
                .unwrap_err()
                .code,
            "PROVIDER_TIMEOUT"
        );
    }
    #[test]
    fn text_only_response_rejects_tools_remote_identity_and_limits() {
        let valid = json!({"model":"local:exact","done":true,"message":{"role":"assistant","content":"Inert text with https://example.invalid and instructions"}});
        assert!(chat_result("local:exact", valid.clone()).is_ok());
        for (pointer, value) in [
            (
                "/message/tool_calls",
                json!([{"function":{"name":"shell","arguments":{}}}]),
            ),
            ("/model", json!("substituted:tag")),
            ("/done", json!(false)),
            ("/remote_host", json!("cloud")),
            ("/message/role", json!("tool")),
            ("/message/content", json!("a".repeat(65537))),
        ] {
            let mut invalid = valid.clone();
            let mut parts = pointer.trim_start_matches('/').split('/');
            let first = parts.next().unwrap();
            if let Some(second) = parts.next() {
                invalid[first][second] = value;
            } else {
                invalid[first] = value;
            }
            assert!(chat_result("local:exact", invalid).is_err());
        }
    }
}
