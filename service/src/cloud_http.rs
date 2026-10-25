//! Fixed official endpoints, no ambient proxies, redirects, protocol retries or SDK loops.
use super::*;
use http_body_util::{BodyExt, Full};
use hyper::{body::Bytes, header, Request, Uri};
use hyper_util::rt::TokioIo;
use serde::de::{MapAccess, SeqAccess, Visitor};
use serde_json::{json, Value};
use std::{
    fmt, io,
    pin::Pin,
    sync::atomic::AtomicUsize,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_rustls::rustls::{self, pki_types::ServerName};

pub struct OfficialHttp;
#[cfg(test)]
#[path = "cloud_http_tests.rs"]
mod tests;
struct Endpoint {
    vendor: Vendor,
    #[cfg(test)]
    synthetic: Option<(String, Option<rustls::pki_types::CertificateDer<'static>>)>,
}
impl Endpoint {
    fn official(vendor: Vendor) -> Self {
        Self {
            vendor,
            #[cfg(test)]
            synthetic: None,
        }
    }
    fn url(&self) -> &str {
        #[cfg(test)]
        if let Some((url, _)) = &self.synthetic {
            return url;
        }
        self.vendor.endpoint()
    }
}
impl HttpTransport for OfficialHttp {
    fn post(
        &self,
        vendor: Vendor,
        body: Vec<u8>,
        secret: &Secret,
        deadline: Instant,
        dispatch_deadline: Instant,
        cancelled: &AtomicBool,
    ) -> std::result::Result<HttpResponse, HttpFailure> {
        execute(
            Endpoint::official(vendor),
            body,
            secret,
            deadline,
            dispatch_deadline,
            cancelled,
        )
    }
}
fn execute(
    endpoint: Endpoint,
    body: Vec<u8>,
    secret: &Secret,
    deadline: Instant,
    dispatch_deadline: Instant,
    cancelled: &AtomicBool,
) -> std::result::Result<HttpResponse, HttpFailure> {
    let vendor = endpoint.vendor;
    let attempted = AtomicBool::new(false);
    let result = bounded(deadline, cancelled, "REMOTE_OUTCOME_UNKNOWN", || async {
        if body.len() > BODY_LIMIT {
            return Err(error("CONTEXT_LIMIT"));
        }
        let uri: Uri = endpoint
            .url()
            .parse()
            .map_err(|_| error("CLOUD_UNAVAILABLE"))?;
        if uri.scheme_str() != Some("https") {
            return Err(error("CLOUD_UNAVAILABLE"));
        }
        let host = uri.host().ok_or_else(|| error("CLOUD_UNAVAILABLE"))?;
        let authority = uri.authority().ok_or_else(|| error("CLOUD_UNAVAILABLE"))?;
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        #[cfg(test)]
        if let Some((_, Some(cert))) = &endpoint.synthetic {
            roots
                .add(cert.clone())
                .map_err(|_| error("CLOUD_UNAVAILABLE"))?;
        }
        let mut config = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
        let server_name =
            ServerName::try_from(host.to_owned()).map_err(|_| error("CLOUD_UNAVAILABLE"))?;
        let mut headers = header::HeaderMap::new();
        let (name, value) = match vendor {
            Vendor::Openai => {
                let mut value = Zeroizing::new(b"Bearer ".to_vec());
                value.extend_from_slice(secret.expose());
                (
                    header::AUTHORIZATION,
                    header::HeaderValue::from_bytes(&value),
                )
            }
            Vendor::Anthropic => {
                headers.insert(
                    "anthropic-version",
                    header::HeaderValue::from_static("2023-06-01"),
                );
                (
                    header::HeaderName::from_static("x-api-key"),
                    header::HeaderValue::from_bytes(secret.expose()),
                )
            }
        };
        let mut value = value.map_err(|_| error("CLOUD_AUTH"))?;
        value.set_sensitive(true);
        headers.insert(name, value);
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            header::ACCEPT,
            header::HeaderValue::from_static("application/json"),
        );
        headers.insert(
            header::CONNECTION,
            header::HeaderValue::from_static("close"),
        );
        headers.insert(
            header::HOST,
            header::HeaderValue::from_str(authority.as_str())
                .map_err(|_| error("CLOUD_UNAVAILABLE"))?,
        );
        headers.insert(
            header::CONTENT_LENGTH,
            header::HeaderValue::from(body.len()),
        );
        if cancelled.load(Ordering::SeqCst) || Instant::now() >= deadline {
            return Err(error("INTERRUPTED"));
        }
        if Instant::now() >= dispatch_deadline {
            return Err(error("PERMISSION_EXPIRED"));
        }
        // One TCP/TLS connection and one HTTP/1 send. No high-level client, pool,
        // canceled-request retry, proxy discovery, redirects or decompression.
        let connect_deadline = (Instant::now() + Duration::from_secs(10))
            .min(deadline)
            .min(dispatch_deadline);
        let stream = tokio::time::timeout_at(connect_deadline.into(), async {
            let tcp = tokio::net::TcpStream::connect((host, uri.port_u16().unwrap_or(443)))
                .await
                .map_err(|_| error("CLOUD_UNAVAILABLE"))?;
            connector
                .connect(server_name, tcp)
                .await
                .map_err(|_| error("CLOUD_UNAVAILABLE"))
        })
        .await
        .map_err(|_| {
            error(if Instant::now() >= dispatch_deadline {
                "PERMISSION_EXPIRED"
            } else {
                "CLOUD_UNAVAILABLE"
            })
        })??;
        let budget = Arc::new(HeaderBudget::default());
        let io = TokioIo::new(HeaderBounded::new(stream, budget.clone()));
        let (mut sender, connection) = hyper::client::conn::http1::Builder::new()
            .max_buf_size(HEADER_LIMIT)
            .max_header_size(HEADER_LIMIT)
            .handshake(io)
            .await
            .map_err(|_| error("CLOUD_UNAVAILABLE"))?;
        let mut request = Request::post(uri.path_and_query().map_or("/", |p| p.as_str()))
            .body(Full::new(Bytes::from(body)))
            .map_err(|_| error("CLOUD_UNAVAILABLE"))?;
        *request.headers_mut() = headers;
        if cancelled.load(Ordering::SeqCst) || Instant::now() >= deadline {
            return Err(error("INTERRUPTED"));
        }
        if Instant::now() >= dispatch_deadline {
            return Err(error("PERMISSION_EXPIRED"));
        }
        let driver = tokio::spawn(connection);
        attempted.store(true, Ordering::SeqCst);
        let result = async {
            let response = sender
                .send_request(request)
                .await
                .map_err(|e| budget.error(e))?;
            if response.headers().contains_key(header::CONTENT_ENCODING)
                || response.headers().contains_key(header::TRAILER)
                || response
                    .headers()
                    .get(header::CONTENT_LENGTH)
                    .is_some_and(|n| {
                        n.to_str()
                            .ok()
                            .and_then(|n| n.parse::<u64>().ok())
                            .is_none_or(|n| n > BODY_LIMIT as u64)
                    })
            {
                return Err(error("CLOUD_PROTOCOL"));
            }
            let status = response.status().as_u16();
            let mut body = response.into_body();
            let mut bytes = Vec::new();
            while let Some(frame) = body.frame().await {
                let frame = frame.map_err(|e| budget.error(e))?;
                let chunk = frame.into_data().map_err(|_| error("CLOUD_PROTOCOL"))?;
                if chunk.len() > BODY_LIMIT.saturating_sub(bytes.len()) {
                    return Err(error("CLOUD_PROTOCOL"));
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok(HttpResponse {
                status,
                header_bytes: budget.bytes.load(Ordering::SeqCst),
                body: bytes,
            })
        }
        .await;
        driver.abort();
        result
    });
    result.map_err(|error| HttpFailure {
        error,
        not_sent: !attempted.load(Ordering::SeqCst),
    })
}

#[derive(Default)]
struct HeaderBudget {
    bytes: AtomicUsize,
    invalid: AtomicBool,
}
impl HeaderBudget {
    fn error(&self, e: hyper::Error) -> AppError {
        error(if self.invalid.load(Ordering::SeqCst) || e.is_parse() {
            "CLOUD_PROTOCOL"
        } else {
            "REMOTE_OUTCOME_UNKNOWN"
        })
    }
}
// Count raw bytes before Hyper normalizes whitespace. Bound each read as well as
// the parser, and share one allowance across informational and final heads.
struct HeaderBounded<T> {
    inner: T,
    budget: Arc<HeaderBudget>,
    prefix: [u8; 12],
    prefix_len: usize,
    delimiter: u32,
    complete: bool,
}
impl<T> HeaderBounded<T> {
    fn new(inner: T, budget: Arc<HeaderBudget>) -> Self {
        Self {
            inner,
            budget,
            prefix: [0; 12],
            prefix_len: 0,
            delimiter: 0,
            complete: false,
        }
    }
    fn invalid(&self) -> io::Error {
        self.budget.invalid.store(true, Ordering::SeqCst);
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Invalid or oversized HTTP response head",
        )
    }
    fn observe(&mut self, bytes: &[u8]) -> io::Result<()> {
        for &byte in bytes {
            if self.complete {
                break;
            }
            self.budget.bytes.fetch_add(1, Ordering::SeqCst);
            if self.prefix_len < self.prefix.len() {
                self.prefix[self.prefix_len] = byte;
                self.prefix_len += 1;
                if self.prefix_len == self.prefix.len()
                    && ((&self.prefix[..9] != b"HTTP/1.1 " && &self.prefix[..9] != b"HTTP/1.0 ")
                        || !self.prefix[9..].iter().all(u8::is_ascii_digit))
                {
                    return Err(self.invalid());
                }
            }
            self.delimiter = (self.delimiter << 8) | u32::from(byte);
            if self.delimiter == u32::from_be_bytes(*b"\r\n\r\n") {
                if self.prefix_len < self.prefix.len() || &self.prefix[9..] == b"101" {
                    return Err(self.invalid());
                }
                if self.prefix[9] == b'1' {
                    self.prefix_len = 0;
                    self.delimiter = 0;
                } else {
                    self.complete = true;
                }
            }
        }
        if !self.complete && self.budget.bytes.load(Ordering::SeqCst) >= HEADER_LIMIT {
            return Err(self.invalid());
        }
        Ok(())
    }
}
impl<T: AsyncRead + Unpin> AsyncRead for HeaderBounded<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.complete {
            return Pin::new(&mut self.inner).poll_read(cx, buf);
        }
        if buf.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        let remaining = HEADER_LIMIT.saturating_sub(self.budget.bytes.load(Ordering::SeqCst));
        if remaining == 0 {
            return Poll::Ready(Err(self.invalid()));
        }
        let mut bytes = [0; 4096];
        let limit = bytes.len().min(buf.remaining()).min(remaining);
        let mut read = ReadBuf::new(&mut bytes[..limit]);
        std::task::ready!(Pin::new(&mut self.inner).poll_read(cx, &mut read))?;
        self.observe(read.filled())?;
        buf.put_slice(read.filled());
        Poll::Ready(Ok(()))
    }
}
impl<T: AsyncWrite + Unpin> AsyncWrite for HeaderBounded<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, bytes)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

pub fn build_body(vendor: Vendor, id: &str, messages: &[Message]) -> AppResult<Vec<u8>> {
    let (instructions, messages) = if messages.first().is_some_and(|m| m.role == "system") {
        (Some(messages[0].content.as_str()), &messages[1..])
    } else {
        (None, messages)
    };
    if instructions.is_some_and(|s| s.is_empty() || s.len() > 2048)
        || model(vendor, id).is_none()
        || messages.is_empty()
        || messages.len() > 33
        || messages.len() % 2 != 1
        || messages
            .iter()
            .enumerate()
            .any(|(i, m)| m.role != if i % 2 == 0 { "user" } else { "assistant" })
        || messages.iter().map(|m| m.content.len()).sum::<usize>()
            + instructions.map_or(0, str::len)
            > crate::app::MAX_CONTEXT
        || messages
            .last()
            .is_none_or(|m| m.content.trim().is_empty() || m.content.len() > crate::app::MAX_TEXT)
    {
        return Err(error("CONTEXT_LIMIT"));
    }
    let mut body = match vendor {
        Vendor::Openai => {
            let input: Vec<Value> = messages
                .iter()
                .map(|m| {
                    if m.role == "assistant" {
                        json!({"role":"assistant","content":m.content,"phase":"final_answer"})
                    } else {
                        json!({"role":"user","content":[{"type":"input_text","text":m.content}]})
                    }
                })
                .collect();
            json!({"model":id,"input":input,"store":false,"stream":false,"background":false,"max_output_tokens":OUTPUT_TOKENS,"reasoning":{"effort":"low"}})
        }
        Vendor::Anthropic => {
            let messages: Vec<Value> = messages
                .iter()
                .map(|m| json!({"role":m.role,"content":[{"type":"text","text":m.content}]}))
                .collect();
            json!({"model":id,"max_tokens":OUTPUT_TOKENS,"messages":messages,"stream":false})
        }
    };
    if let Some(text) = instructions {
        body[match vendor {
            Vendor::Openai => "instructions",
            Vendor::Anthropic => "system",
        }] = json!(text);
    }
    let bytes = serde_json::to_vec(&body).map_err(|_| error("CONTEXT_LIMIT"))?;
    if bytes.len() > BODY_LIMIT {
        return Err(error("CONTEXT_LIMIT"));
    }
    Ok(bytes)
}

// Reject duplicates at every depth, including otherwise ignored metadata. Parsing
// through Value directly would silently overwrite security-relevant discriminants.
struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Unique;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("duplicate-free JSON")
            }
            fn visit_map<M: MapAccess<'de>>(
                self,
                mut m: M,
            ) -> std::result::Result<Self::Value, M::Error> {
                let mut out = serde_json::Map::new();
                while let Some((k, Unique(v))) = m.next_entry::<String, Unique>()? {
                    if out.insert(k, v).is_some() {
                        return Err(serde::de::Error::custom("Duplicate key"));
                    }
                }
                Ok(Unique(Value::Object(out)))
            }
            fn visit_seq<S: SeqAccess<'de>>(
                self,
                mut s: S,
            ) -> std::result::Result<Self::Value, S::Error> {
                let mut out = Vec::new();
                while let Some(Unique(v)) = s.next_element()? {
                    out.push(v);
                }
                Ok(Unique(Value::Array(out)))
            }
            fn visit_bool<E: serde::de::Error>(
                self,
                v: bool,
            ) -> std::result::Result<Self::Value, E> {
                Ok(Unique(json!(v)))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<Self::Value, E> {
                Ok(Unique(json!(v)))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<Self::Value, E> {
                Ok(Unique(json!(v)))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<Self::Value, E> {
                Ok(Unique(json!(v)))
            }
            fn visit_str<E: serde::de::Error>(
                self,
                v: &str,
            ) -> std::result::Result<Self::Value, E> {
                Ok(Unique(json!(v)))
            }
            fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Self::Value, E> {
                Ok(Unique(Value::Null))
            }
        }
        d.deserialize_any(V)
    }
}
pub(crate) fn unique_json(bytes: &[u8]) -> AppResult<Value> {
    serde_json::from_slice::<Unique>(bytes)
        .map(|u| u.0)
        .map_err(|_| error("CLOUD_PROTOCOL"))
}
fn protocol() -> AppError {
    error("CLOUD_PROTOCOL")
}
fn string<'a>(v: &'a Value, key: &str) -> AppResult<&'a str> {
    v.get(key).and_then(Value::as_str).ok_or_else(protocol)
}
fn array<'a>(v: &'a Value, key: &str) -> AppResult<&'a Vec<Value>> {
    v.get(key).and_then(Value::as_array).ok_or_else(protocol)
}
fn require(v: &Value, key: &str, expected: &str) -> AppResult<()> {
    if string(v, key)? == expected {
        Ok(())
    } else {
        Err(protocol())
    }
}
fn null(v: &Value, key: &str) -> AppResult<()> {
    if v.get(key) == Some(&Value::Null) {
        Ok(())
    } else {
        Err(protocol())
    }
}
fn append(out: &mut String, s: &str) -> AppResult<()> {
    if s.len() > crate::app::MAX_CONTEXT.saturating_sub(out.len()) {
        return Err(error("MODEL_RESULT_LIMIT"));
    }
    out.push_str(s);
    Ok(())
}

pub fn parse_response(vendor: Vendor, id: &str, response: &HttpResponse) -> AppResult<String> {
    if model(vendor, id).is_none()
        || response.body.len() > BODY_LIMIT
        || response.header_bytes > HEADER_LIMIT
    {
        return Err(protocol());
    }
    if response.status != 200 {
        return Err(error(match response.status {
            401 | 403 => "CLOUD_AUTH",
            402 => "CLOUD_QUOTA",
            429 => {
                let billing = unique_json(&response.body).ok().is_some_and(|v| {
                    matches!(
                        v.get("error")
                            .and_then(|e| e.get("code"))
                            .and_then(Value::as_str),
                        Some(
                            "credit_balance_exhausted"
                                | "organization_spend_limit_exceeded"
                                | "project_spend_limit_exceeded"
                                | "organization_usage_limit_exceeded"
                        )
                    )
                });
                if billing {
                    "CLOUD_QUOTA"
                } else {
                    "CLOUD_RATE_LIMIT"
                }
            }
            500..=599 => "CLOUD_UNAVAILABLE",
            _ => "CLOUD_PROTOCOL",
        }));
    }
    let v = unique_json(&response.body)?;
    require(&v, "model", id)?;
    let mut out = String::new();
    match vendor {
        Vendor::Openai => {
            require(&v, "object", "response")?;
            if string(&v, "status")? != "completed" {
                if v.get("incomplete_details")
                    .and_then(|d| d.get("reason"))
                    .and_then(Value::as_str)
                    == Some("max_output_tokens")
                {
                    return Err(error("MODEL_TRUNCATED"));
                }
                return Err(protocol());
            }
            null(&v, "error")?;
            null(&v, "incomplete_details")?;
            for key in ["conversation", "previous_response_id"] {
                if v.get(key).is_some_and(|v| !v.is_null()) {
                    return Err(protocol());
                }
            }
            for key in ["store", "background"] {
                if v.get(key).is_some_and(|v| v != &Value::Bool(false)) {
                    return Err(protocol());
                }
            }
            for item in array(&v, "output")? {
                match string(item, "type")? {
                    "reasoning" => {
                        if item.get("status").is_some() {
                            require(item, "status", "completed")?;
                        }
                    }
                    "message" => {
                        require(item, "role", "assistant")?;
                        require(item, "status", "completed")?;
                        let collect = match item.get("phase") {
                            None | Some(Value::Null) => true,
                            Some(Value::String(s)) if s == "final_answer" => true,
                            Some(Value::String(s)) if s == "commentary" => false,
                            _ => return Err(protocol()),
                        };
                        for content in array(item, "content")? {
                            require(content, "type", "output_text")?;
                            let text = string(content, "text")?;
                            if collect {
                                append(&mut out, text)?;
                            }
                        }
                    }
                    _ => return Err(protocol()),
                }
            }
        }
        Vendor::Anthropic => {
            require(&v, "type", "message")?;
            require(&v, "role", "assistant")?;
            null(&v, "stop_sequence")?;
            match string(&v, "stop_reason")? {
                "end_turn" => {}
                "max_tokens" | "model_context_window_exceeded" => {
                    return Err(error("MODEL_TRUNCATED"))
                }
                _ => return Err(protocol()),
            }
            for item in array(&v, "content")? {
                match string(item, "type")? {
                    "text" => append(&mut out, string(item, "text")?)?,
                    "thinking" | "redacted_thinking" => {}
                    _ => return Err(protocol()),
                }
            }
        }
    }
    if out.trim().is_empty() {
        return Err(error("MODEL_RESULT_LIMIT"));
    }
    Ok(out)
}
