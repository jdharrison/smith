//! Private socket-pair Secret Service protocol fixtures, never a user session bus.
use super::*;
use sha2::Digest;
use std::sync::{atomic::AtomicUsize, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
const COLLECTION_PATH: &str = "/org/freedesktop/secrets/collection/test";
const ITEM_PATH: &str = "/org/freedesktop/secrets/collection/test/item";
const SESSION_PATH: &str = "/org/freedesktop/secrets/session/test";
fn identity() -> CredentialRef {
    CredentialRef {
        provider: Vendor::Openai,
        generation: "1".repeat(32),
        save_request_id: "2".repeat(32),
    }
}
fn path(s: &str) -> OwnedObjectPath {
    OwnedObjectPath::try_from(s).unwrap()
}
#[derive(Clone, Copy, Default)]
enum Negotiation {
    #[default]
    Valid,
    Padded,
    WrongType,
    Empty,
    Oversized,
    Zero,
    One,
    MinusOne,
    Modulus,
    AboveModulus,
    RootPath,
    Rejected,
    Stalled,
}
#[derive(Clone, Copy, Default)]
enum BusFault {
    #[default]
    None,
    Missing,
    WellKnownOwner,
    BusDaemonOwner,
    MalformedOwner,
    WrongUid,
    OwnerChanged,
    UidChanged,
}
#[derive(Default)]
struct State {
    exists: AtomicBool,
    locked: AtomicBool,
    ambiguous: AtomicBool,
    prompt: AtomicBool,
    changed: AtomicBool,
    slow: AtomicBool,
    calls: AtomicUsize,
    creates: AtomicUsize,
    gets: AtomicUsize,
    identity: Mutex<Option<CredentialRef>>,
    returned_attributes: Mutex<Option<std::collections::HashMap<String, String>>>,
    after_read_attributes: Mutex<Option<std::collections::HashMap<String, String>>>,
    wire: Mutex<Option<WireSecret>>,
    returned_content_type: Mutex<Option<String>>,
    peer_key: Mutex<Option<[u8; 16]>>,
    negotiation: Mutex<Negotiation>,
    bus_fault: Mutex<BusFault>,
    hellos: AtomicUsize,
    owner_lookups: AtomicUsize,
    uid_lookups: AtomicUsize,
    opens: AtomicUsize,
    targets: Mutex<Vec<String>>,
}
impl State {
    fn expected_attributes(&self) -> std::collections::HashMap<String, String> {
        let identity = self
            .identity
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(identity);
        // Assert the wire identity independently of the production attrs() helper.
        std::collections::HashMap::from([
            ("application".into(), "smith-next".into()),
            ("provider".into(), identity.provider.name().into()),
            ("generation".into(), identity.generation),
            ("save_request_id".into(), identity.save_request_id),
        ])
    }
}
fn no_auto_start(header: &zbus::message::Header<'_>) {
    assert!(header
        .primary()
        .flags()
        .contains(zbus::message::Flags::NoAutoStart));
}
fn pinned(state: &State, header: &zbus::message::Header<'_>) {
    no_auto_start(header);
    let destination = header.destination().unwrap().as_str();
    assert_eq!(destination, ":1.2");
    state.targets.lock().unwrap().push(destination.into());
}
struct Bus(Arc<State>);
impl Bus {
    fn hello(&self) -> &str {
        self.0.hellos.fetch_add(1, Ordering::SeqCst);
        ":1.3"
    }
    fn get_name_owner(
        &self,
        name: &str,
        header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<String> {
        no_auto_start(&header);
        assert_eq!(name, "org.freedesktop.secrets");
        let index = self.0.owner_lookups.fetch_add(1, Ordering::SeqCst);
        Ok(match *self.0.bus_fault.lock().unwrap() {
            BusFault::Missing => {
                return Err(zbus::fdo::Error::NameHasNoOwner(
                    "synthetic owner missing".into(),
                ))
            }
            BusFault::WellKnownOwner => "org.freedesktop.secrets",
            BusFault::BusDaemonOwner => "org.freedesktop.DBus",
            BusFault::MalformedOwner => ":invalid owner",
            BusFault::OwnerChanged if index > 0 => ":1.4",
            _ => ":1.2",
        }
        .into())
    }
    fn get_connection_unix_user(&self, name: &str, header: zbus::message::Header<'_>) -> u32 {
        no_auto_start(&header);
        assert!(matches!(name, ":1.2" | ":1.4"));
        let index = self.0.uid_lookups.fetch_add(1, Ordering::SeqCst);
        let uid = unsafe { libc::geteuid() };
        match *self.0.bus_fault.lock().unwrap() {
            BusFault::WrongUid => uid + 1,
            BusFault::UidChanged if index > 0 => uid + 1,
            _ => uid,
        }
    }
}
fn serve_bus(conn: &Connection, state: Arc<State>) {
    // The private endpoint models bus routing itself; only Secret Service
    // messages are dispatched by zbus's peer object server.
    let mut messages = zbus::MessageStream::from(conn);
    let conn = conn.clone();
    tokio::spawn(async move {
        use zbus::export::futures_util::StreamExt;
        let bus = Bus(state);
        while let Some(Ok(message)) = messages.next().await {
            let header = message.header();
            if header.destination().map(|d| d.as_str()) != Some("org.freedesktop.DBus") {
                continue;
            }
            assert_eq!(
                header.interface().map(|s| s.as_str()),
                Some("org.freedesktop.DBus")
            );
            match header.member().unwrap().as_str() {
                "Hello" => conn.reply(&message, &bus.hello()).await.unwrap(),
                "GetNameOwner" => {
                    let (name,): (String,) = message.body().deserialize().unwrap();
                    match bus.get_name_owner(&name, header) {
                        Ok(owner) => conn.reply(&message, &owner).await.unwrap(),
                        Err(error) => conn
                            .reply_dbus_error(&message.header(), error)
                            .await
                            .unwrap(),
                    }
                }
                "GetConnectionUnixUser" => {
                    let (name,): (String,) = message.body().deserialize().unwrap();
                    conn.reply(&message, &bus.get_connection_unix_user(&name, header))
                        .await
                        .unwrap();
                }
                _ => panic!("unexpected private bus operation"),
            }
        }
    });
}
// Independent peer implementation of RFC2409 group 2 and RFC5869 HKDF.
// In particular this does not call the production derivation or inject its key.
fn peer_prime() -> BigUint {
    BigUint::parse_bytes(b"FFFFFFFFFFFFFFFFC90FDAA22168C234C4C6628B80DC1CD129024E088A67CC74020BBEA63B139B22514A08798E3404DDEF9519B3CD3A431B302B0A6DF25F1437\
4FE1356D6D51C245E485B576625E7EC6F44C42E9A637ED6B0BFF5CB6F406B7EDEE386BFB5A899FA5AE9F24117C4B1FE649286651ECE65381FFFFFFFFFFFFFFFF", 16).unwrap()
}
fn peer_hmac(key: &[u8], data: &[u8]) -> [u8; 32] {
    assert!(key.len() <= 64);
    let mut inner = [0x36; 64];
    let mut outer = [0x5c; 64];
    for (i, byte) in key.iter().enumerate() {
        inner[i] ^= byte;
        outer[i] ^= byte;
    }
    let mut digest = sha2::Sha256::new();
    digest.update(inner);
    digest.update(data);
    let hash = digest.finalize();
    let mut digest = sha2::Sha256::new();
    digest.update(outer);
    digest.update(hash);
    digest.finalize().into()
}
fn negotiate_peer(state: &State, public: Vec<u8>, padded: bool) -> Vec<u8> {
    let prime = peer_prime();
    assert!(!public.is_empty() && public.len() <= 128);
    let public = BigUint::from_bytes_be(&public);
    assert!(public > BigUint::from(1u8) && public < &prime - BigUint::from(1u8));
    // Small independent private exponents make a leading-zero shared-secret
    // fixture inexpensive without replacing any production randomness.
    let (private, shared) = (2u32..100_000)
        .find_map(|n| {
            let private = BigUint::from(n);
            let shared = public.modpow(&private, &prime).to_bytes_be();
            (!padded || shared.len() < 128).then_some((private, shared))
        })
        .unwrap();
    let mut padded_shared = [0; 128];
    padded_shared[128 - shared.len()..].copy_from_slice(&shared);
    if padded {
        assert_eq!(padded_shared[0], 0);
    }
    let extracted = peer_hmac(&[0; 32], &padded_shared);
    let expanded = peer_hmac(&extracted, &[1]);
    *state.peer_key.lock().unwrap() = Some(expanded[..16].try_into().unwrap());
    BigUint::from(2u8).modpow(&private, &prime).to_bytes_be()
}
struct Service(Arc<State>);
#[zbus::interface(name = "org.freedesktop.Secret.Service")]
impl Service {
    async fn open_session(
        &self,
        algorithm: &str,
        input: OwnedValue,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> zbus::fdo::Result<(OwnedValue, OwnedObjectPath)> {
        pinned(&self.0, &header);
        assert_eq!(algorithm, "dh-ietf1024-sha256-aes128-cbc-pkcs7");
        self.0.opens.fetch_add(1, Ordering::SeqCst);
        let public: Vec<u8> = input.try_into().unwrap();
        let mode = *self.0.negotiation.lock().unwrap();
        let prime = peer_prime();
        let bytes = match mode {
            Negotiation::Valid | Negotiation::Padded => {
                negotiate_peer(&self.0, public, matches!(mode, Negotiation::Padded))
            }
            Negotiation::WrongType => return Ok((OwnedValue::from(7u32), path(SESSION_PATH))),
            Negotiation::Empty => vec![],
            Negotiation::Oversized => vec![2; 129],
            Negotiation::Zero => vec![0],
            Negotiation::One => vec![1],
            Negotiation::MinusOne => (&prime - BigUint::from(1u8)).to_bytes_be(),
            Negotiation::Modulus => prime.to_bytes_be(),
            Negotiation::AboveModulus => (&prime + BigUint::from(1u8)).to_bytes_be(),
            Negotiation::RootPath => {
                return Ok((Value::from(vec![2u8]).try_to_owned().unwrap(), path("/")))
            }
            Negotiation::Rejected => {
                return Err(zbus::fdo::Error::NotSupported(
                    "synthetic encrypted session rejected".into(),
                ))
            }
            Negotiation::Stalled => {
                tokio::time::sleep(Duration::from_secs(5)).await;
                vec![2]
            }
        };
        Ok((
            Value::from(bytes).try_to_owned().unwrap(),
            path(SESSION_PATH),
        ))
    }
    async fn search_items(
        &self,
        attributes: std::collections::HashMap<String, String>,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> (Vec<OwnedObjectPath>, Vec<OwnedObjectPath>) {
        assert!(header
            .primary()
            .flags()
            .contains(zbus::message::Flags::NoAutoStart));
        pinned(&self.0, &header);
        assert_eq!(attributes, self.0.expected_attributes());
        self.0.calls.fetch_add(1, Ordering::SeqCst);
        if self.0.slow.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        if !self.0.exists.load(Ordering::SeqCst) {
            return (vec![], vec![]);
        }
        if self.0.ambiguous.load(Ordering::SeqCst) {
            return (vec![path(ITEM_PATH), path(ITEM_PATH)], vec![]);
        }
        if self.0.locked.load(Ordering::SeqCst) {
            return (vec![], vec![path(ITEM_PATH)]);
        }
        (vec![path(ITEM_PATH)], vec![])
    }
    fn read_alias(&self, alias: &str) -> OwnedObjectPath {
        assert_eq!(alias, "default");
        path(COLLECTION_PATH)
    }
}
struct Collection(Arc<State>);
#[zbus::interface(name = "org.freedesktop.Secret.Collection")]
impl Collection {
    #[zbus(property)]
    fn locked(&self) -> bool {
        self.0.locked.load(Ordering::SeqCst)
    }
    fn create_item(
        &self,
        mut properties: std::collections::HashMap<String, OwnedValue>,
        secret: WireSecret,
        replace: bool,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> (OwnedObjectPath, OwnedObjectPath) {
        assert!(header
            .primary()
            .flags()
            .contains(zbus::message::Flags::NoAutoStart));
        assert!(!replace);
        assert_eq!(properties.len(), 2);
        let attributes: std::collections::HashMap<String, String> = properties
            .remove("org.freedesktop.Secret.Item.Attributes")
            .unwrap()
            .try_into()
            .unwrap();
        let expected = self.0.expected_attributes();
        assert_eq!(attributes, expected);
        let label: String = properties
            .remove("org.freedesktop.Secret.Item.Label")
            .unwrap()
            .try_into()
            .unwrap();
        assert_eq!(
            label,
            format!("Smith-next {} credential", expected["provider"])
        );
        self.0.creates.fetch_add(1, Ordering::SeqCst);
        if self.0.prompt.load(Ordering::SeqCst) {
            return (path("/"), path("/org/freedesktop/secrets/prompt/test"));
        }
        assert_eq!(secret.0.as_str(), SESSION_PATH);
        assert_eq!(secret.1.len(), 16);
        assert_ne!(secret.2, b"synthetic-keyring-canary");
        pinned(&self.0, &header);
        let key = self.0.peer_key.lock().unwrap().unwrap();
        let plain = cbc::Decryptor::<aes::Aes128>::new_from_slices(&key, &secret.1)
            .unwrap()
            .decrypt_padded_vec_mut::<Pkcs7>(&secret.2)
            .unwrap();
        assert_eq!(plain, b"synthetic-keyring-canary");
        // Return a fresh, independently encrypted secret rather than echoing the
        // client's ciphertext, so GetSecret exercises both sides of the session.
        let iv = [4u8; 16];
        let encrypted = cbc::Encryptor::<aes::Aes128>::new_from_slices(&key, &iv)
            .unwrap()
            .encrypt_padded_vec_mut::<Pkcs7>(&plain);
        assert_eq!(secret.3, "text/plain; charset=utf-8");
        // GNOME normalizes this metadata rather than echoing the submitted MIME.
        let content_type = self
            .0
            .returned_content_type
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| "text/plain".into());
        *self.0.wire.lock().unwrap() =
            Some((path(SESSION_PATH), iv.to_vec(), encrypted, content_type));
        self.0.exists.store(true, Ordering::SeqCst);
        (path(ITEM_PATH), path("/"))
    }
}
struct Item(Arc<State>);
#[zbus::interface(name = "org.freedesktop.Secret.Item")]
impl Item {
    #[zbus(property)]
    fn locked(&self) -> bool {
        self.0.locked.load(Ordering::SeqCst)
    }
    #[zbus(property)]
    fn attributes(&self) -> std::collections::HashMap<String, String> {
        self.0
            .returned_attributes
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| self.0.expected_attributes())
    }
    #[zbus(property)]
    fn created(&self) -> u64 {
        12
    }
    #[zbus(property)]
    fn modified(&self) -> u64 {
        if self.0.changed.load(Ordering::SeqCst) {
            13
        } else {
            12
        }
    }
    fn get_secret(
        &self,
        session: OwnedObjectPath,
        #[zbus(header)] header: zbus::message::Header<'_>,
    ) -> WireSecret {
        assert!(header
            .primary()
            .flags()
            .contains(zbus::message::Flags::NoAutoStart));
        pinned(&self.0, &header);
        assert_eq!(session.as_str(), SESSION_PATH);
        self.0.gets.fetch_add(1, Ordering::SeqCst);
        if let Some(attributes) = self.0.after_read_attributes.lock().unwrap().clone() {
            *self.0.returned_attributes.lock().unwrap() = Some(attributes);
        }
        self.0.wire.lock().unwrap().clone().unwrap()
    }
}
async fn open_pair(state: Arc<State>) -> (AppResult<Session>, Connection) {
    let (a, b) = tokio::net::UnixStream::pair().unwrap();
    let server = zbus::connection::Builder::unix_stream(a)
        .p2p()
        .auth_mechanism(zbus::AuthMechanism::External)
        .server(zbus::Guid::generate())
        .unwrap()
        .unique_name(":1.2")
        .unwrap()
        .name("org.freedesktop.secrets")
        .unwrap()
        .serve_at(ROOT, Service(state.clone()))
        .unwrap()
        .serve_at(COLLECTION_PATH, Collection(state.clone()))
        .unwrap()
        .serve_at(ITEM_PATH, Item(state.clone()))
        .unwrap()
        .build();
    let (server, session) = tokio::join!(
        async {
            let conn = server.await.unwrap();
            serve_bus(&conn, state);
            conn
        },
        Session::open_stream(b, unsafe { libc::geteuid() })
    );
    (session, server)
}
async fn pair(state: Arc<State>) -> (Session, Connection) {
    let (session, server) = open_pair(state).await;
    (session.unwrap(), server)
}
fn run<T, F, Fut>(f: F) -> AppResult<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = AppResult<T>>,
{
    bounded(
        Instant::now() + Duration::from_secs(2),
        &AtomicBool::new(false),
        "KEYRING_TIMEOUT",
        f,
    )
}
fn assert_plain_text_round_trip(content_type: Option<&str>, provider: Vendor, schema: bool) {
    run(|| async {
        let state = Arc::new(State::default());
        let mut identity = identity();
        identity.provider = provider;
        *state.identity.lock().unwrap() = Some(identity.clone());
        if schema {
            let mut attributes = state.expected_attributes();
            attributes.insert("xdg:schema".into(), "org.freedesktop.Secret.Generic".into());
            *state.returned_attributes.lock().unwrap() = Some(attributes);
        }
        *state.returned_content_type.lock().unwrap() = content_type.map(str::to_owned);
        let (s, _server) = pair(state.clone()).await;
        let secret = Secret::new(b"synthetic-keyring-canary".to_vec())?;
        let attempted = AtomicBool::new(false);
        let saved = s
            .save(
                &identity,
                &secret,
                &attempted,
                Instant::now() + Duration::from_secs(1),
                &AtomicBool::new(false),
            )
            .await;
        let creates = state.creates.load(Ordering::SeqCst);
        assert_eq!(creates, 1);
        assert!(attempted.load(Ordering::SeqCst));
        assert!(state.exists.load(Ordering::SeqCst));
        if let Err(e) = saved {
            panic!(
                "encrypted plain-text round trip failed: {}; provider={}; schema={schema}; creates={creates}",
                e.code, provider.name()
            );
        }
        assert_eq!(s.read(&identity).await?.expose(), secret.expose());
        assert_eq!(state.gets.load(Ordering::SeqCst), 2);
        assert_eq!(state.creates.load(Ordering::SeqCst), 1);
        Ok(())
    })
    .unwrap();
}
#[test]
fn gnome_plain_text_round_trip() {
    assert_plain_text_round_trip(None, Vendor::Openai, false);
}
#[test]
fn utf8_plain_text_round_trip() {
    assert_plain_text_round_trip(Some("text/plain; charset=utf-8"), Vendor::Openai, false);
}
#[test]
fn gnome_schema_metadata_round_trip() {
    for provider in [Vendor::Openai, Vendor::Anthropic] {
        for schema in [false, true] {
            for content_type in [None, Some("text/plain; charset=utf-8")] {
                assert_plain_text_round_trip(content_type, provider, schema);
            }
        }
    }
}
#[test]
fn invalid_item_attributes_fail_before_secret_read_without_retry() {
    let expected = State::default().expected_attributes();
    let mut cases = Vec::new();
    for value in [
        "",
        "org.freedesktop.Secret.Other",
        "org.freedesktop.Secret.Generic ",
        " org.freedesktop.Secret.Generic",
        "org.freedesktop.secret.Generic",
    ] {
        let mut attributes = expected.clone();
        attributes.insert("xdg:schema".into(), value.into());
        cases.push(attributes);
    }
    for schema in [false, true] {
        let mut base = expected.clone();
        if schema {
            base.insert("xdg:schema".into(), "org.freedesktop.Secret.Generic".into());
        }
        let mut extra = base.clone();
        extra.insert("unknown".into(), "value".into());
        cases.push(extra);
        for key in ["application", "provider", "generation", "save_request_id"] {
            let mut missing = base.clone();
            missing.remove(key);
            cases.push(missing.clone());
            // An extra field cannot stand in for a required identity at the
            // otherwise permitted cardinality, with or without schema metadata.
            missing.insert("unknown".into(), "value".into());
            cases.push(missing);
            let mut changed = base.clone();
            changed.insert(key.into(), "different".into());
            cases.push(changed);
        }
    }
    cases.push(std::collections::HashMap::from([(
        "xdg:schema".into(),
        "org.freedesktop.Secret.Generic".into(),
    )]));
    for attributes in cases {
        run(|| async {
            let state = Arc::new(State::default());
            *state.returned_attributes.lock().unwrap() = Some(attributes.clone());
            let (s, _server) = pair(state.clone()).await;
            let secret = Secret::new(b"synthetic-keyring-canary".to_vec())?;
            let attempted = AtomicBool::new(false);
            assert_eq!(
                s.save(
                    &identity(),
                    &secret,
                    &attempted,
                    Instant::now() + Duration::from_secs(1),
                    &AtomicBool::new(false),
                )
                .await
                .unwrap_err()
                .code,
                "KEYRING_CONFLICT",
                "attributes: {attributes:?}"
            );
            assert!(attempted.load(Ordering::SeqCst));
            assert!(state.exists.load(Ordering::SeqCst));
            assert_eq!(state.creates.load(Ordering::SeqCst), 1);
            assert_eq!(
                s.read(&identity()).await.err().unwrap().code,
                "KEYRING_CONFLICT",
                "attributes: {attributes:?}"
            );
            assert_eq!(state.gets.load(Ordering::SeqCst), 0);
            assert_eq!(state.creates.load(Ordering::SeqCst), 1);
            Ok(())
        })
        .unwrap();
    }
}
#[test]
fn invalid_metadata_after_secret_read_is_rejected() {
    for key in [
        "xdg:schema",
        "application",
        "provider",
        "generation",
        "save_request_id",
        "unknown",
    ] {
        run(|| async {
            let state = Arc::new(State::default());
            let mut attributes = state.expected_attributes();
            attributes.insert("xdg:schema".into(), "org.freedesktop.Secret.Generic".into());
            *state.returned_attributes.lock().unwrap() = Some(attributes.clone());
            attributes.insert(key.into(), "different".into());
            *state.after_read_attributes.lock().unwrap() = Some(attributes);
            let (s, _server) = pair(state.clone()).await;
            let secret = Secret::new(b"synthetic-keyring-canary".to_vec())?;
            let attempted = AtomicBool::new(false);
            assert_eq!(
                s.save(
                    &identity(),
                    &secret,
                    &attempted,
                    Instant::now() + Duration::from_secs(1),
                    &AtomicBool::new(false),
                )
                .await
                .unwrap_err()
                .code,
                "KEYRING_CONFLICT",
                "changed attribute: {key}"
            );
            assert!(attempted.load(Ordering::SeqCst));
            assert_eq!(state.gets.load(Ordering::SeqCst), 1);
            assert_eq!(state.creates.load(Ordering::SeqCst), 1);
            Ok(())
        })
        .unwrap();
    }
}
#[test]
fn unsupported_content_types_fail_after_create_and_on_read() {
    for content_type in [
        "",
        "application/octet-stream",
        "application/x-binary",
        "text/html",
        "text/plain; charset=iso-8859-1",
        "text/pläin",
        "text/plain; charset=ütf-8",
    ] {
        run(|| async {
            let state = Arc::new(State::default());
            *state.returned_content_type.lock().unwrap() = Some(content_type.into());
            let (s, _server) = pair(state.clone()).await;
            let secret = Secret::new(b"synthetic-keyring-canary".to_vec())?;
            let attempted = AtomicBool::new(false);
            let err = s
                .save(
                    &identity(),
                    &secret,
                    &attempted,
                    Instant::now() + Duration::from_secs(1),
                    &AtomicBool::new(false),
                )
                .await
                .unwrap_err();
            assert_eq!(err.code, "KEYRING_CONFLICT", "MIME: {content_type:?}");
            assert!(attempted.load(Ordering::SeqCst));
            assert!(state.exists.load(Ordering::SeqCst));
            assert_eq!(state.creates.load(Ordering::SeqCst), 1);
            assert_eq!(
                s.read(&identity()).await.err().unwrap().code,
                "KEYRING_CONFLICT",
                "MIME: {content_type:?}"
            );
            assert_eq!(state.creates.load(Ordering::SeqCst), 1);
            Ok(())
        })
        .unwrap();
    }
}
#[test]
fn encrypted_create_read_and_no_implicit_replacement() {
    run(|| async {
        let state = Arc::new(State::default());
        let (s, _server) = pair(state.clone()).await;
        let secret = Secret::new(b"synthetic-keyring-canary".to_vec())?;
        s.save(
            &identity(),
            &secret,
            &AtomicBool::new(false),
            Instant::now() + Duration::from_secs(1),
            &AtomicBool::new(false),
        )
        .await?;
        assert_eq!(s.read(&identity()).await?.expose(), secret.expose());
        assert_eq!(
            s.save(
                &identity(),
                &secret,
                &AtomicBool::new(false),
                Instant::now() + Duration::from_secs(1),
                &AtomicBool::new(false)
            )
            .await
            .unwrap_err()
            .code,
            "KEYRING_CONFLICT"
        );
        assert_eq!(state.creates.load(Ordering::SeqCst), 1);
        Ok(())
    })
    .unwrap();
}
#[test]
fn missing_locked_ambiguous_and_changed_items_fail_without_prompt_or_unlock() {
    for mode in 0..4 {
        run(|| async {
            let state = Arc::new(State::default());
            state.exists.store(mode != 0, Ordering::SeqCst);
            state.locked.store(mode == 1, Ordering::SeqCst);
            state.ambiguous.store(mode == 2, Ordering::SeqCst);
            state.changed.store(mode == 3, Ordering::SeqCst);
            let (s, _server) = pair(state.clone()).await;
            assert_eq!(
                s.read(&identity()).await.err().unwrap().code,
                match mode {
                    0 => "KEYRING_MISSING",
                    1 => "KEYRING_LOCKED",
                    _ => "KEYRING_CONFLICT",
                }
            );
            assert_eq!(state.creates.load(Ordering::SeqCst), 0);
            Ok(())
        })
        .unwrap();
    }
}
#[test]
fn returned_prompt_is_never_invoked_and_original_deadline_prevents_create() {
    run(|| async {
        let state = Arc::new(State::default());
        state.prompt.store(true, Ordering::SeqCst);
        let (s, _server) = pair(state.clone()).await;
        let secret = Secret::new(b"synthetic-keyring-canary".to_vec())?;
        let attempted = AtomicBool::new(false);
        assert_eq!(
            s.save(
                &identity(),
                &secret,
                &attempted,
                Instant::now() + Duration::from_secs(1),
                &AtomicBool::new(false)
            )
            .await
            .unwrap_err()
            .code,
            "KEYRING_PROMPT_REQUIRED"
        );
        assert!(attempted.load(Ordering::SeqCst));
        assert_eq!(state.creates.load(Ordering::SeqCst), 1);
        assert_eq!(
            s.save(
                &identity(),
                &secret,
                &AtomicBool::new(false),
                Instant::now() - Duration::from_millis(1),
                &AtomicBool::new(false)
            )
            .await
            .unwrap_err()
            .code,
            "KEYRING_TIMEOUT"
        );
        assert_eq!(state.creates.load(Ordering::SeqCst), 1);
        Ok(())
    })
    .unwrap();
}
#[test]
fn stalled_private_dbus_is_cancelled_with_original_budget() {
    let start = Instant::now();
    let result = bounded(
        start + Duration::from_millis(100),
        &AtomicBool::new(false),
        "KEYRING_TIMEOUT",
        || async {
            let state = Arc::new(State::default());
            state.slow.store(true, Ordering::SeqCst);
            let (s, _server) = pair(state).await;
            s.read(&identity()).await
        },
    );
    assert_eq!(result.err().unwrap().code, "KEYRING_TIMEOUT");
    assert!(start.elapsed() < Duration::from_secs(1));
}
#[test]
fn actual_session_negotiation_matches_independent_peer_with_leading_zero_shared_secret() {
    run(|| async {
        let state = Arc::new(State::default());
        *state.negotiation.lock().unwrap() = Negotiation::Padded;
        let (session, _server) = pair(state.clone()).await;
        assert_eq!(
            session.key.as_ref(),
            &state.peer_key.lock().unwrap().unwrap()
        );
        assert_eq!(state.hellos.load(Ordering::SeqCst), 1);
        assert_eq!(state.opens.load(Ordering::SeqCst), 1);
        assert_eq!(state.owner_lookups.load(Ordering::SeqCst), 2);
        assert_eq!(state.uid_lookups.load(Ordering::SeqCst), 2);
        let secret = Secret::new(b"synthetic-keyring-canary".to_vec())?;
        session
            .save(
                &identity(),
                &secret,
                &AtomicBool::new(false),
                Instant::now() + Duration::from_secs(1),
                &AtomicBool::new(false),
            )
            .await?;
        assert_eq!(session.read(&identity()).await?.expose(), secret.expose());
        assert!(state
            .targets
            .lock()
            .unwrap()
            .iter()
            .all(|name| name == ":1.2"));
        Ok(())
    })
    .unwrap();
}
#[test]
fn malformed_or_rejected_encrypted_sessions_never_fall_back_or_access_items() {
    for mode in [
        Negotiation::WrongType,
        Negotiation::Empty,
        Negotiation::Oversized,
        Negotiation::Zero,
        Negotiation::One,
        Negotiation::MinusOne,
        Negotiation::Modulus,
        Negotiation::AboveModulus,
        Negotiation::RootPath,
        Negotiation::Rejected,
    ] {
        run(|| async {
            let state = Arc::new(State::default());
            *state.negotiation.lock().unwrap() = mode;
            let (session, _server) = open_pair(state.clone()).await;
            assert_eq!(
                session.err().unwrap().code,
                if matches!(mode, Negotiation::Rejected) {
                    "KEYRING_UNAVAILABLE"
                } else {
                    "KEYRING_CONFLICT"
                }
            );
            assert_eq!(state.opens.load(Ordering::SeqCst), 1);
            assert_eq!(state.calls.load(Ordering::SeqCst), 0);
            assert_eq!(state.creates.load(Ordering::SeqCst), 0);
            assert!(state.peer_key.lock().unwrap().is_none());
            Ok(())
        })
        .unwrap();
    }
}
#[test]
fn missing_malformed_wrong_uid_and_changing_owners_fail_before_secret_access() {
    for fault in [
        BusFault::Missing,
        BusFault::WellKnownOwner,
        BusFault::BusDaemonOwner,
        BusFault::MalformedOwner,
        BusFault::WrongUid,
        BusFault::OwnerChanged,
        BusFault::UidChanged,
    ] {
        run(|| async {
            let state = Arc::new(State::default());
            *state.bus_fault.lock().unwrap() = fault;
            let (session, _server) = open_pair(state.clone()).await;
            assert_eq!(session.err().unwrap().code, "KEYRING_UNAVAILABLE");
            assert_eq!(
                state.opens.load(Ordering::SeqCst),
                usize::from(matches!(
                    fault,
                    BusFault::OwnerChanged | BusFault::UidChanged
                ))
            );
            assert_eq!(state.calls.load(Ordering::SeqCst), 0);
            assert_eq!(state.creates.load(Ordering::SeqCst), 0);
            assert!(state
                .targets
                .lock()
                .unwrap()
                .iter()
                .all(|name| name == ":1.2"));
            Ok(())
        })
        .unwrap();
    }
}
#[test]
fn session_operations_remain_pinned_without_retargeting_a_replacement_owner() {
    run(|| async {
        let state = Arc::new(State::default());
        let (session, _server) = pair(state.clone()).await;
        *state.bus_fault.lock().unwrap() = BusFault::OwnerChanged;
        let secret = Secret::new(b"synthetic-keyring-canary".to_vec())?;
        session
            .save(
                &identity(),
                &secret,
                &AtomicBool::new(false),
                Instant::now() + Duration::from_secs(1),
                &AtomicBool::new(false),
            )
            .await?;
        assert_eq!(session.read(&identity()).await?.expose(), secret.expose());
        assert_eq!(state.owner_lookups.load(Ordering::SeqCst), 2);
        assert_eq!(state.opens.load(Ordering::SeqCst), 1);
        assert!(state
            .targets
            .lock()
            .unwrap()
            .iter()
            .all(|name| name == ":1.2"));
        Ok(())
    })
    .unwrap();
}
#[test]
fn wrong_socket_peer_uid_is_rejected_before_any_bus_authentication() {
    run(|| async {
        for uid in [0, unsafe { libc::geteuid() } + 1] {
            let (mut peer, client) = tokio::net::UnixStream::pair().unwrap();
            let result = Session::open_stream(client, uid).await;
            assert_eq!(result.err().unwrap().code, "KEYRING_UNAVAILABLE");
            let mut bytes = Vec::new();
            peer.read_to_end(&mut bytes).await.unwrap();
            assert!(bytes.is_empty());
        }
        Ok(())
    })
    .unwrap();
}
#[test]
fn rejected_external_authentication_never_tries_anonymous_or_cookie_auth() {
    run(|| async {
        let (mut peer, client) = tokio::net::UnixStream::pair().unwrap();
        let server = async {
            let mut auth = Vec::new();
            while !auth.ends_with(b"\r\n") {
                let byte = peer.read_u8().await.unwrap();
                auth.push(byte);
                assert!(auth.len() <= 256);
            }
            assert!(auth.starts_with(b"\0AUTH EXTERNAL "));
            peer.write_all(b"REJECTED ANONYMOUS DBUS_COOKIE_SHA1\r\n")
                .await
                .unwrap();
            let mut rest = Vec::new();
            peer.read_to_end(&mut rest).await.unwrap();
            assert!(!rest.windows(4).any(|w| w == b"AUTH"));
            assert!(!rest.windows(5).any(|w| w == b"BEGIN"));
        };
        let ((), result) = tokio::join!(
            server,
            Session::open_stream(client, unsafe { libc::geteuid() })
        );
        assert_eq!(result.err().unwrap().code, "KEYRING_UNAVAILABLE");
        Ok(())
    })
    .unwrap();
}
#[test]
fn stalled_open_session_uses_original_budget_without_plaintext_fallback() {
    let state = Arc::new(State::default());
    *state.negotiation.lock().unwrap() = Negotiation::Stalled;
    let start = Instant::now();
    let result = bounded(
        start + Duration::from_millis(150),
        &AtomicBool::new(false),
        "KEYRING_TIMEOUT",
        || async {
            let (session, _server) = open_pair(state.clone()).await;
            session
        },
    );
    assert_eq!(result.err().unwrap().code, "KEYRING_TIMEOUT");
    assert!(start.elapsed() < Duration::from_secs(1));
    assert_eq!(state.opens.load(Ordering::SeqCst), 1);
    assert_eq!(state.calls.load(Ordering::SeqCst), 0);
    assert_eq!(state.creates.load(Ordering::SeqCst), 0);
}
#[test]
fn encrypted_session_group_and_namespaced_identity_are_fixed() {
    let prime = BigUint::parse_bytes(PRIME.as_bytes(), 16).unwrap();
    assert_eq!(prime.bits(), 1024);
    assert_eq!(ALGORITHM, "dh-ietf1024-sha256-aes128-cbc-pkcs7");
    assert_eq!(attrs(&identity()).len(), 4);
    assert_eq!(attrs(&identity())["application"], "smith-next");
}
