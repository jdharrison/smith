use super::*;
use crate::git_tree::{self, Tree};
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::UnixStream;

const ID: &str = "0123456789abcdef0123456789abcdef";

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "smith-source-export-{}",
            crate::app::new_id().unwrap()
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
    fn root(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
/// Synthetic commit over `files` without invoking Git; IDs are host-verified anyway.
fn commit(files: &[(&str, &[u8])]) -> (String, Vec<u8>) {
    let mut tree = Tree::default();
    for (path, bytes) in files {
        assert!(tree.insert(path, git_tree::REGULAR, git_tree::object_id("blob", bytes)));
    }
    let raw = format!(
        "tree {}\nauthor A <a@invalid> 0 +0000\ncommitter A <a@invalid> 0 +0000\n\nm\n",
        git_tree::hex(&tree.id())
    )
    .into_bytes();
    (git_tree::hex(&git_tree::object_id("commit", &raw)), raw)
}
struct FakeRelay {
    pin: String,
    requests: usize,
}
impl Relay for FakeRelay {
    fn request(
        &mut self,
        index: usize,
        advertisement: bool,
        body: &[u8],
        _until: Instant,
        _cancelled: &AtomicBool,
    ) -> LocalResult<(String, Vec<u8>)> {
        assert_eq!((index, advertisement, body), (0, true, &b""[..]));
        self.requests += 1;
        Ok((
            "application/x-git-upload-pack-advertisement".into(),
            b"ad".to_vec(),
        ))
    }
    fn pins(&mut self, checkouts: &[Checkout]) -> LocalResult<Vec<Checkout>> {
        assert_eq!(checkouts.len(), 1);
        Ok(vec![Checkout {
            commit: self.pin.clone(),
            ..checkouts[0].clone()
        }])
    }
}
fn repositories() -> Vec<Repository> {
    vec![Repository {
        url: "https://github.com/smith-fixtures/source.git".into(),
        reference: Some("main".into()),
    }]
}
// The host may correctly abort mid-script; the simulated worker then just stops.
fn send(stream: &mut UnixStream, frame: Frame) -> Option<()> {
    let until = Instant::now() + Duration::from_secs(5);
    protocol::send(stream, &frame, until, &|| false).ok()
}
fn receive(stream: &mut BufReader<UnixStream>) -> Option<Frame> {
    let until = Instant::now() + Duration::from_secs(5);
    protocol::receive(stream, until, &|| false).ok()
}
/// Simulated worker: relay one advertisement, publish a checkout, then `script`.
fn worker(
    script: Vec<Frame>,
    handshake: bool,
) -> (UnixStream, UnixStream, std::thread::JoinHandle<()>) {
    let (host_in, worker_in) = UnixStream::pair().unwrap();
    let (worker_out, host_out) = UnixStream::pair().unwrap();
    for s in [&host_in, &host_out] {
        s.set_nonblocking(true).unwrap();
    }
    let thread = std::thread::spawn(move || {
        let mut input = BufReader::new(worker_in);
        let mut output = worker_out;
        let _ = simulate(&mut input, &mut output, script, handshake);
        // Keep the pipe open until the host stops reading.
        let _ = input
            .get_mut()
            .set_read_timeout(Some(Duration::from_millis(500)));
        let _ = std::io::Read::read(&mut input, &mut [0u8; 1]);
    });
    (host_in, host_out, thread)
}
fn simulate(
    input: &mut BufReader<UnixStream>,
    output: &mut UnixStream,
    script: Vec<Frame>,
    handshake: bool,
) -> Option<()> {
    assert!(matches!(
        receive(input)?,
        Frame::Initialize { version: 1, .. }
    ));
    if handshake {
        send(
            output,
            Frame::RepositoryRequest {
                request_id: ID.into(),
                sequence: 1,
                repository: 0,
                advertisement: true,
                body: vec![],
            },
        )?;
        assert!(matches!(
            receive(input)?,
            Frame::ResponseStart { sequence: 1, .. }
        ));
        assert!(matches!(
            receive(input)?,
            Frame::ResponseChunk { last: true, .. }
        ));
        send(
            output,
            Frame::Repositories {
                request_id: ID.into(),
                repositories: vec![Checkout {
                    url: repositories()[0].url.clone(),
                    reference: "main".into(),
                    commit: "f".repeat(40),
                }],
            },
        )?;
        assert!(matches!(
            receive(input)?,
            Frame::ResponseStart { sequence: 0, .. }
        ));
        assert!(matches!(receive(input)?, Frame::ResponseChunk { .. }));
    }
    for frame in script {
        send(output, frame)?;
    }
    Some(())
}
fn file(path: &str, bytes: &[u8], last: bool) -> Frame {
    Frame::SourceFile {
        request_id: ID.into(),
        repository: 0,
        path: path.into(),
        mode: git_tree::REGULAR,
        bytes: bytes.to_vec(),
        last,
    }
}
fn run(
    pin: &str,
    script: Vec<Frame>,
    handshake: bool,
    root: PathBuf,
) -> LocalResult<Vec<PreparedRepository>> {
    let (mut input, output, thread) = worker(script, handshake);
    let mut output = BufReader::new(output);
    let mut relay = FakeRelay {
        pin: pin.into(),
        requests: 0,
    };
    let until = Instant::now() + Duration::from_secs(5);
    let result = drive(
        &mut input,
        &mut output,
        &mut relay,
        ID,
        &repositories(),
        &[root],
        until,
        until,
        &AtomicBool::new(false),
    );
    drop(input);
    drop(output);
    thread.join().unwrap();
    result
}
fn complete(raw: &[u8], files: Vec<Frame>, count: usize) -> Vec<Frame> {
    let mut script = vec![Frame::SourceCommit {
        request_id: ID.into(),
        repository: 0,
        object: raw.to_vec(),
    }];
    script.extend(files);
    script.push(Frame::SourceEnd {
        request_id: ID.into(),
        repository: 0,
        files: count,
    });
    script
}

#[test]
fn verified_export_writes_only_included_entries() {
    let f = Fixture::new();
    let (pin, raw) = commit(&[("a.txt", b"hello\n"), ("AGENTS.md", b"rules\n")]);
    let root = f.root("ok");
    let script = complete(
        &raw,
        vec![
            file("AGENTS.md", b"rules\n", true),
            file("a.txt", b"hel", false),
            file("a.txt", b"lo\n", true),
        ],
        2,
    );
    let prepared = run(&pin, script, true, root.clone()).unwrap();
    assert_eq!(prepared.len(), 1);
    assert_eq!(prepared[0].commit, pin);
    assert_eq!(prepared[0].files.keys().collect::<Vec<_>>(), ["a.txt"]);
    assert_eq!(fs::read(root.join("a.txt")).unwrap(), b"hello\n");
    assert!(!root.join("AGENTS.md").exists());
}

#[test]
fn tampering_pin_mismatch_and_protocol_violations_fail_closed() {
    let f = Fixture::new();
    let (pin, raw) = commit(&[("a.txt", b"hello\n")]);
    let tampered = complete(&raw, vec![file("a.txt", b"HELLO\n", true)], 1);
    assert_eq!(
        run(&pin, tampered, true, f.root("tampered")).err(),
        Some(LocalError::RepositoryRejected)
    );
    let honest = complete(&raw, vec![file("a.txt", b"hello\n", true)], 1);
    // The relay's pin, not the worker's claim, selects the commit to verify.
    assert_eq!(
        run(&"e".repeat(40), honest, true, f.root("pin")).err(),
        Some(LocalError::RepositoryRejected)
    );
    let early = complete(&raw, vec![file("a.txt", b"hello\n", true)], 1);
    assert_eq!(
        run(&pin, early, false, f.root("early")).err(),
        Some(LocalError::Protocol)
    );
    let mut foreign = complete(&raw, vec![], 0);
    foreign.insert(
        0,
        Frame::SourceEnd {
            request_id: "f".repeat(32),
            repository: 0,
            files: 0,
        },
    );
    assert_eq!(
        run(&pin, foreign, true, f.root("foreign")).err(),
        Some(LocalError::Protocol)
    );
    let mut order = complete(&raw, vec![file("a.txt", b"hello\n", true)], 1);
    if let Frame::SourceFile { repository, .. } = &mut order[1] {
        *repository = 1;
    }
    assert_eq!(
        run(&pin, order, true, f.root("order")).err(),
        Some(LocalError::Protocol)
    );
    let failed = vec![Frame::Failed {
        request_id: ID.into(),
        kind: "source_export_failed".into(),
    }];
    assert_eq!(
        run(&pin, failed, true, f.root("failed")).err(),
        Some(LocalError::RepositoryUnavailable)
    );
    let oversized = complete(
        &raw,
        vec![file("a.txt", &vec![b'x'; protocol::CHUNK + 1], true)],
        1,
    );
    assert_eq!(
        run(&pin, oversized, true, f.root("oversized")).err(),
        Some(LocalError::Protocol)
    );
}

#[test]
fn export_worker_shares_inspection_isolation() {
    for flag in [
        "--network=none",
        "--read-only",
        "--cap-drop=ALL",
        "--security-opt=no-new-privileges:true",
        "--user=1000:1000",
    ] {
        assert!(ISOLATION.contains(&flag), "{flag}");
    }
    assert!(!ISOLATION
        .iter()
        .any(|f| f.starts_with("--volume") || f.starts_with("--mount")));
}
