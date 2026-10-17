//! Private TLS fixtures for the actual transport path. No official URL is called.
use super::*;
use std::sync::{atomic::AtomicUsize, mpsc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::rustls::{
    self,
    pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer},
};
const DEADLINE_TEST_BUDGET: Duration = Duration::from_secs(3);
const SETUP_ENVELOPE: Duration = Duration::from_secs(2);
const DEADLINE_SCHEDULING_SLACK: Duration = Duration::from_secs(1);
#[derive(Default)]
struct Stages {
    before_tls: Option<(mpsc::SyncSender<Instant>, mpsc::Receiver<()>)>,
    request_received: Option<mpsc::SyncSender<Instant>>,
}
struct Server {
    url: String,
    cert: rustls::pki_types::CertificateDer<'static>,
    calls: Arc<AtomicUsize>,
    request: Arc<std::sync::Mutex<Vec<u8>>>,
    response_bytes: Arc<AtomicUsize>,
    listener: std::net::TcpListener,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Server {
    fn new(response: Vec<u8>, drip: bool, cert_name: &str) -> Self {
        Self::with_chunks(
            response,
            if drip { 32 } else { usize::MAX },
            if drip {
                Duration::from_millis(20)
            } else {
                Duration::ZERO
            },
            Duration::ZERO,
            cert_name,
        )
    }
    fn with_chunks(
        response: Vec<u8>,
        chunk_size: usize,
        delay: Duration,
        linger: Duration,
        cert_name: &str,
    ) -> Self {
        Self::with_stages(
            response,
            chunk_size,
            delay,
            linger,
            cert_name,
            Stages::default(),
        )
    }
    fn with_stages(
        response: Vec<u8>,
        chunk_size: usize,
        delay: Duration,
        linger: Duration,
        cert_name: &str,
        stages: Stages,
    ) -> Self {
        let certified = rcgen::generate_simple_self_signed(vec![cert_name.into()]).unwrap();
        let cert = certified.cert.der().clone();
        let key =
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(certified.key_pair.serialize_der()));
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![certified.cert.der().clone()], key)
            .unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let listener_observer = listener.try_clone().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let response_bytes = Arc::new(AtomicUsize::new(0));
        let written = response_bytes.clone();
        let capture = Arc::new(std::sync::Mutex::new(Vec::new()));
        let request = capture.clone();
        let count = calls.clone();
        let thread = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async {
                let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                let Ok(Ok((socket, _))) =
                    tokio::time::timeout(Duration::from_secs(2), listener.accept()).await
                else {
                    return;
                };
                if let Some((arrived, release)) = stages.before_tls {
                    if arrived.send(Instant::now()).is_err()
                        || release
                            .recv_timeout(DEADLINE_TEST_BUDGET + DEADLINE_SCHEDULING_SLACK)
                            .is_err()
                    {
                        return;
                    }
                }
                let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
                let Ok(Ok(mut stream)) =
                    tokio::time::timeout(Duration::from_secs(2), acceptor.accept(socket)).await
                else {
                    return;
                };
                count.fetch_add(1, Ordering::SeqCst);
                let mut bytes = Vec::new();
                let mut block = [0; 1024];
                loop {
                    let Ok(Ok(n)) =
                        tokio::time::timeout(Duration::from_secs(2), stream.read(&mut block)).await
                    else {
                        return;
                    };
                    if n == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&block[..n]);
                    if bytes.len() > BODY_LIMIT {
                        return;
                    }
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                        let len = headers
                            .lines()
                            .find_map(|s| {
                                s.strip_prefix("content-length: ")
                                    .and_then(|s| s.parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + len {
                            break;
                        }
                    }
                }
                *capture.lock().unwrap() = bytes;
                if let Some(arrived) = stages.request_received {
                    if arrived.send(Instant::now()).is_err() {
                        return;
                    }
                }
                for part in response.chunks(chunk_size) {
                    if stream.write_all(part).await.is_err() {
                        return;
                    }
                    written.fetch_add(part.len(), Ordering::SeqCst);
                    if !delay.is_zero() {
                        tokio::time::sleep(delay).await;
                    }
                }
                if !linger.is_zero() {
                    tokio::time::sleep(linger).await;
                }
                let _ = stream.shutdown().await;
            });
        });
        Self {
            url: format!("https://127.0.0.1:{port}/fixture"),
            cert,
            calls,
            request,
            response_bytes,
            listener: listener_observer,
            thread: Some(thread),
        }
    }
    fn request(
        &self,
        vendor: Vendor,
        trust: bool,
        budget: Duration,
    ) -> Result<HttpResponse, HttpFailure> {
        self.request_until(vendor, trust, Instant::now() + budget)
    }
    fn request_until(
        &self,
        vendor: Vendor,
        trust: bool,
        deadline: Instant,
    ) -> Result<HttpResponse, HttpFailure> {
        let endpoint = Endpoint {
            vendor,
            synthetic: Some((self.url.clone(), trust.then(|| self.cert.clone()))),
        };
        let secret = Secret::new(b"synthetic-http-canary".to_vec()).unwrap();
        execute(
            endpoint,
            b"{}".to_vec(),
            &secret,
            deadline,
            deadline,
            &AtomicBool::new(false),
        )
    }
    fn assert_no_second_connection(&self) {
        assert_eq!(
            self.listener.accept().err().unwrap().kind(),
            std::io::ErrorKind::WouldBlock,
            "an automatic retry queued a second connection"
        );
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.thread.take().unwrap().join().unwrap();
    }
}
#[test]
fn fixed_auth_headers_and_one_post_with_actual_tls() {
    for vendor in Vendor::ALL {
        let s = Server::new(
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".to_vec(),
            false,
            "127.0.0.1",
        );
        let r = s
            .request(vendor, true, Duration::from_secs(2))
            .unwrap_or_else(|_| panic!("synthetic TLS request failed"));
        assert_eq!(r.status, 200);
        assert_eq!(r.body, b"{}");
        assert_eq!(s.calls.load(Ordering::SeqCst), 1);
        let raw = s.request.lock().unwrap();
        let text = String::from_utf8_lossy(&raw);
        assert!(text.starts_with("POST /fixture HTTP/1.1"));
        assert_eq!(text.matches("synthetic-http-canary").count(), 1);
        match vendor {
            Vendor::Openai => assert!(text.contains("authorization: Bearer synthetic-http-canary")),
            Vendor::Anthropic => {
                assert!(text.contains("x-api-key: synthetic-http-canary"));
                assert!(text.contains("anthropic-version: 2023-06-01"));
            }
        }
    }
}
#[test]
fn redirects_are_not_followed_and_credentials_never_reach_target() {
    let target = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    target.set_nonblocking(true).unwrap();
    let response=format!("HTTP/1.1 307 Temporary Redirect\r\nLocation: https://127.0.0.1:{}/stolen\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",target.local_addr().unwrap().port());
    let s = Server::new(response.into_bytes(), false, "127.0.0.1");
    let r = s
        .request(Vendor::Openai, true, Duration::from_secs(2))
        .unwrap_or_else(|_| panic!("redirect was not returned"));
    assert_eq!(r.status, 307);
    assert_eq!(s.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        target.accept().err().unwrap().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
#[test]
fn invalid_certificates_and_hostnames_fail_closed_without_http() {
    for (trust, name) in [(false, "127.0.0.1"), (true, "wrong.invalid")] {
        let s = Server::new(vec![], false, name);
        assert!(s
            .request(Vendor::Openai, trust, Duration::from_secs(2))
            .is_err());
        assert_eq!(s.calls.load(Ordering::SeqCst), 0);
        assert!(s.request.lock().unwrap().is_empty());
    }
}
#[test]
fn oversized_headers_content_encoding_and_chunked_body_are_rejected() {
    let responses = vec![
        format!(
            "HTTP/1.1 200 OK\r\nX-Large: {}\r\nContent-Length: 0\r\n\r\n",
            "x".repeat(HEADER_LIMIT)
        )
        .into_bytes(),
        b"HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: 0\r\n\r\n".to_vec(),
        format!(
            "HTTP/1.1 503 Failure\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n",
            BODY_LIMIT + 1,
            "x".repeat(BODY_LIMIT + 1)
        )
        .into_bytes(),
    ];
    for response in responses {
        let s = Server::new(response, false, "127.0.0.1");
        let e = s
            .request(Vendor::Anthropic, true, Duration::from_secs(2))
            .err()
            .unwrap();
        assert_eq!(e.error.code, "CLOUD_PROTOCOL");
        assert_eq!(s.calls.load(Ordering::SeqCst), 1);
    }
}
fn padded_head(size: usize) -> Vec<u8> {
    let prefix = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nX-Pad: x";
    let suffix = b"\r\n\r\n";
    let mut bytes = prefix.to_vec();
    bytes.resize(size - suffix.len(), b' ');
    bytes.extend_from_slice(suffix);
    bytes
}
#[test]
fn raw_header_exact_limit_including_whitespace_and_status_line() {
    for size in [HEADER_LIMIT - 1, HEADER_LIMIT, HEADER_LIMIT + 1] {
        let mut response = padded_head(size);
        response.extend_from_slice(b"{}");
        let s = Server::new(response, false, "127.0.0.1");
        let result = s.request(Vendor::Openai, true, Duration::from_secs(2));
        if size <= HEADER_LIMIT {
            let response = result.unwrap_or_else(|_| panic!("bounded head rejected: {size}"));
            assert_eq!(response.header_bytes, size);
            assert_eq!(response.body, b"{}");
        } else {
            assert_eq!(result.err().unwrap().error.code, "CLOUD_PROTOCOL");
        }
        assert_eq!(s.calls.load(Ordering::SeqCst), 1);
    }
}
#[test]
fn incomplete_slow_header_is_rejected_at_cap_without_waiting_for_delimiter_or_eof() {
    for extra in [0, 8192] {
        let mut response = b"HTTP/1.1 200 OK\r\nX-Pad: x".to_vec();
        response.resize(HEADER_LIMIT + extra, b' ');
        let s = Server::with_chunks(
            response,
            1024,
            Duration::from_millis(5),
            Duration::from_secs(1),
            "127.0.0.1",
        );
        let start = Instant::now();
        let failure = s
            .request(Vendor::Openai, true, Duration::from_secs(3))
            .err()
            .unwrap();
        assert_eq!(failure.error.code, "CLOUD_PROTOCOL");
        assert!(!failure.not_sent);
        assert!(start.elapsed() < Duration::from_secs(1));
        assert_eq!(s.calls.load(Ordering::SeqCst), 1);
    }
}
#[test]
fn informational_heads_share_one_raw_budget_and_cannot_reset_it() {
    for extra in [0, 1] {
        let mut response = b"HTTP/1.1 103 Early Hints\r\nX-Hint: ignored\r\n\r\n".to_vec();
        response.extend_from_slice(&padded_head(HEADER_LIMIT - response.len() + extra));
        response.extend_from_slice(b"{}");
        let s = Server::new(response, false, "127.0.0.1");
        let result = s.request(Vendor::Openai, true, Duration::from_secs(2));
        if extra == 0 {
            let response = result.unwrap_or_else(|_| panic!("bounded interim heads rejected"));
            assert_eq!(response.header_bytes, HEADER_LIMIT);
            assert_eq!(response.body, b"{}");
        } else {
            assert_eq!(result.err().unwrap().error.code, "CLOUD_PROTOCOL");
        }
        assert_eq!(s.calls.load(Ordering::SeqCst), 1);
    }
}
#[test]
fn expiry_during_gated_tls_is_not_sent_without_retry() {
    let (arrived_tx, arrived_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    let s = Server::with_stages(
        vec![],
        usize::MAX,
        Duration::ZERO,
        Duration::ZERO,
        "127.0.0.1",
        Stages {
            before_tls: Some((arrived_tx, release_rx)),
            ..Stages::default()
        },
    );
    let start = Instant::now();
    let deadline = start + DEADLINE_TEST_BUDGET;
    let (accepted, failure) = std::thread::scope(|scope| {
        let client = scope.spawn(|| s.request_until(Vendor::Openai, true, deadline));
        let accepted = arrived_rx
            .recv_timeout(SETUP_ENVELOPE)
            .expect("TCP accept missed the bounded setup envelope");
        // Hold the actual handshake until the original operation deadline. No
        // application request can be sent, even though TCP already connected.
        let failure = client.join().unwrap().err().unwrap();
        (accepted, failure)
    });
    let finished = Instant::now();
    release_tx.send(()).unwrap();
    assert!(accepted <= start + SETUP_ENVELOPE);
    assert_eq!(failure.error.code, "REMOTE_OUTCOME_UNKNOWN");
    assert!(failure.not_sent);
    assert!(finished >= deadline && finished < deadline + DEADLINE_SCHEDULING_SLACK);
    assert_eq!(s.calls.load(Ordering::SeqCst), 0);
    assert!(s.request.lock().unwrap().is_empty());
    assert_eq!(s.response_bytes.load(Ordering::SeqCst), 0);
    s.assert_no_second_connection();
    eprintln!(
        "pre-send expiry: accepted_ms={} elapsed_ms={} http_calls=0 not_sent=true",
        accepted.duration_since(start).as_millis(),
        finished.duration_since(start).as_millis()
    );
}
#[test]
fn slow_drip_and_disconnect_are_bounded_never_retried() {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: 10000\r\n\r\n{}",
        "x".repeat(10000)
    );
    let response_len = response.len();
    let chunk_size = 32;
    let chunk_delay = Duration::from_millis(20);
    let minimum_drain = chunk_delay * ((response_len - 1) / chunk_size) as u32;
    assert!(DEADLINE_TEST_BUDGET + DEADLINE_SCHEDULING_SLACK < minimum_drain);
    let (arrived_tx, arrived_rx) = mpsc::sync_channel(1);
    let s = Server::with_stages(
        response.into_bytes(),
        chunk_size,
        chunk_delay,
        Duration::ZERO,
        "127.0.0.1",
        Stages {
            request_received: Some(arrived_tx),
            ..Stages::default()
        },
    );
    // Three seconds includes runtime/root/TCP/TLS setup; the request must be
    // captured within two. The 10,000-byte body alone needs at least 6.24s at
    // 32 bytes/20ms, so even the 1s scheduling slack cannot allow a full drain.
    // There is ONE absolute deadline, never refreshed at the barrier/progress.
    let start = Instant::now();
    let deadline = start + DEADLINE_TEST_BUDGET;
    let (captured, e) = std::thread::scope(|scope| {
        let client = scope.spawn(|| s.request_until(Vendor::Openai, true, deadline));
        let captured = arrived_rx
            .recv_timeout(SETUP_ENVELOPE)
            .expect("HTTP request missed the bounded setup envelope");
        (captured, client.join().unwrap().err().unwrap())
    });
    let finished = Instant::now();
    assert!(captured <= start + SETUP_ENVELOPE);
    assert!(s
        .request
        .lock()
        .unwrap()
        .starts_with(b"POST /fixture HTTP/1.1\r\n"));
    assert_eq!(e.error.code, "REMOTE_OUTCOME_UNKNOWN");
    assert!(!e.not_sent);
    assert!(finished >= deadline && finished < deadline + DEADLINE_SCHEDULING_SLACK);
    assert_eq!(s.calls.load(Ordering::SeqCst), 1);
    let written = s.response_bytes.load(Ordering::SeqCst);
    assert!(
        written >= 96 && written < response_len,
        "slow response must make progress without completing: {written}/{response_len}"
    );
    s.assert_no_second_connection();
    eprintln!("post-send expiry: captured_ms={} elapsed_ms={} response_bytes={} http_calls=1 not_sent=false",
        captured.duration_since(start).as_millis(), finished.duration_since(start).as_millis(), written);
    let s = Server::new(
        b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nshort".to_vec(),
        false,
        "127.0.0.1",
    );
    let failure = s
        .request(Vendor::Openai, true, SETUP_ENVELOPE)
        .err()
        .unwrap();
    assert_eq!(failure.error.code, "REMOTE_OUTCOME_UNKNOWN");
    assert!(!failure.not_sent);
    assert!(s
        .request
        .lock()
        .unwrap()
        .starts_with(b"POST /fixture HTTP/1.1\r\n"));
    assert_eq!(s.calls.load(Ordering::SeqCst), 1);
    s.assert_no_second_connection();
}
#[test]
fn owner_cancellation_closes_an_active_request_without_retry() {
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: 10000\r\n\r\n{}",
        "x".repeat(10000)
    );
    let server = Server::new(response.into_bytes(), true, "127.0.0.1");
    let cancelled = AtomicBool::new(false);
    let deadline = Instant::now() + Duration::from_secs(2);
    let failure = std::thread::scope(|scope| {
        scope.spawn(|| {
            let wait_until = Instant::now() + Duration::from_secs(1);
            while server.request.lock().unwrap().is_empty() {
                assert!(Instant::now() < wait_until);
                std::thread::sleep(Duration::from_millis(1));
            }
            cancelled.store(true, Ordering::SeqCst);
        });
        execute(
            Endpoint {
                vendor: Vendor::Openai,
                synthetic: Some((server.url.clone(), Some(server.cert.clone()))),
            },
            b"{}".to_vec(),
            &Secret::new(b"synthetic-only".to_vec()).unwrap(),
            deadline,
            deadline,
            &cancelled,
        )
        .err()
        .unwrap()
    });
    assert_eq!(failure.error.code, "INTERRUPTED");
    assert!(!failure.not_sent);
    assert_eq!(server.calls.load(Ordering::SeqCst), 1);
}
#[test]
fn expired_permission_or_cancelled_owner_never_connects() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    for cancelled in [false, true] {
        let endpoint = Endpoint {
            vendor: Vendor::Openai,
            synthetic: Some((
                format!(
                    "https://127.0.0.1:{}/fixture",
                    listener.local_addr().unwrap().port()
                ),
                None,
            )),
        };
        let secret = Secret::new(b"synthetic-only".to_vec()).unwrap();
        let now = Instant::now();
        let e = execute(
            endpoint,
            b"{}".to_vec(),
            &secret,
            now + Duration::from_secs(1),
            now - Duration::from_millis(1),
            &AtomicBool::new(cancelled),
        )
        .err()
        .unwrap();
        assert!(e.not_sent);
        assert_eq!(
            listener.accept().err().unwrap().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}
#[test]
fn ambient_proxy_environment_does_not_route_the_request() {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "cloud::http::tests::proxy_environment_child",
            "--ignored",
        ])
        .env_clear()
        .env("HTTP_PROXY", "http://127.0.0.1:9")
        .env("HTTPS_PROXY", "http://127.0.0.1:9")
        .env("ALL_PROXY", "http://127.0.0.1:9")
        .output()
        .unwrap();
    assert!(output.status.success());
}
#[test]
#[ignore = "isolated proxy-environment subprocess fixture"]
fn proxy_environment_child() {
    fixed_auth_headers_and_one_post_with_actual_tls();
}
