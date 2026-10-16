//! Narrow Secret Service subset for prepared Ubuntu GNOME Keyring sessions.
//! No session-bus environment lookup, activation, unlock, Prompt, collection creation,
//! replacement, deletion or plaintext session. Every call targets a pinned unique owner.
use super::*;
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use num_bigint::BigUint;
use rand::RngCore;
use std::collections::BTreeMap;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::{proxy::MethodFlags, Connection, Proxy};

pub struct SecretService;
#[cfg(test)]
#[path = "cloud_keyring_tests.rs"]
mod tests;
const ROOT: &str = "/org/freedesktop/secrets";
const SERVICE: &str = "org.freedesktop.Secret.Service";
const ITEM: &str = "org.freedesktop.Secret.Item";
const COLLECTION: &str = "org.freedesktop.Secret.Collection";
const ALGORITHM: &str = "dh-ietf1024-sha256-aes128-cbc-pkcs7";
// RFC 2409 group 2, required by the Secret Service encrypted-session algorithm.
const PRIME: &str = concat!(
    "FFFFFFFFFFFFFFFFC90FDAA22168C234C4C6628B80DC1CD129024E08",
    "8A67CC74020BBEA63B139B22514A08798E3404DDEF9519B3CD3A431B302B0A6DF25F1437",
    "4FE1356D6D51C245E485B576625E7EC6F44C42E9A637ED6B0BFF5CB6F406B7EDEE386B",
    "FB5A899FA5AE9F24117C4B1FE649286651ECE65381FFFFFFFFFFFFFFFF"
);
type WireSecret = (OwnedObjectPath, Vec<u8>, Vec<u8>, String);
fn unavailable() -> AppError {
    error("KEYRING_UNAVAILABLE")
}
fn conflict() -> AppError {
    error("KEYRING_CONFLICT")
}
fn attrs(identity: &CredentialRef) -> BTreeMap<&str, &str> {
    BTreeMap::from([
        ("application", "smith-next"),
        ("provider", identity.provider.name()),
        ("generation", identity.generation.as_str()),
        ("save_request_id", identity.save_request_id.as_str()),
    ])
}
async fn call<B, R>(
    c: &Connection,
    owner: &str,
    path: &str,
    interface: &str,
    method: &str,
    body: &B,
) -> AppResult<R>
where
    B: serde::Serialize + zbus::zvariant::DynamicType,
    R: for<'de> zbus::zvariant::DynamicDeserialize<'de>,
{
    Proxy::new(c, owner, path, interface)
        .await
        .map_err(|_| unavailable())?
        .call_with_flags(method, MethodFlags::NoAutoStart.into(), body)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(unavailable)
}
struct Session {
    conn: Connection,
    owner: String,
    path: OwnedObjectPath,
    key: Zeroizing<[u8; 16]>,
}
impl Session {
    async fn open() -> AppResult<Self> {
        // Fixed login-scoped user bus; never honor TCP or autolaunch addresses.
        let uid = unsafe { libc::geteuid() };
        if uid == 0 {
            return Err(unavailable());
        }
        let path = format!("/run/user/{uid}/bus");
        let meta = std::fs::symlink_metadata(&path).map_err(|_| unavailable())?;
        if meta.uid() != uid || !meta.file_type().is_socket() {
            return Err(unavailable());
        }
        let stream = tokio::net::UnixStream::connect(path)
            .await
            .map_err(|_| unavailable())?;
        Self::open_stream(stream, uid).await
    }
    // Only the socket is injected by private tests. Authentication, owner/UID
    // checks and encrypted negotiation are identical to the fixed-path caller.
    async fn open_stream(stream: tokio::net::UnixStream, uid: u32) -> AppResult<Self> {
        if uid == 0 || stream.peer_cred().map_err(|_| unavailable())?.uid() != uid {
            return Err(unavailable());
        }
        let conn = zbus::connection::Builder::unix_stream(stream)
            .auth_mechanism(zbus::AuthMechanism::External)
            .max_queued(8)
            .build()
            .await
            .map_err(|_| unavailable())?;
        let owner = Self::checked_owner(&conn, uid).await?;
        let prime = BigUint::parse_bytes(PRIME.as_bytes(), 16).ok_or_else(unavailable)?;
        let mut random = Zeroizing::new([0u8; 128]);
        rand::rngs::OsRng
            .try_fill_bytes(random.as_mut())
            .map_err(|_| unavailable())?;
        let private = BigUint::from_bytes_be(random.as_ref());
        if private <= BigUint::from(1u8) {
            return Err(unavailable());
        }
        let public = BigUint::from(2u8).modpow(&private, &prime).to_bytes_be();
        let (remote, path): (OwnedValue, OwnedObjectPath) = call(
            &conn,
            &owner,
            ROOT,
            SERVICE,
            "OpenSession",
            &(ALGORITHM, Value::from(public)),
        )
        .await?;
        let remote: Vec<u8> = remote.try_into().map_err(|_| conflict())?;
        if remote.is_empty() || remote.len() > 128 || path.as_str() == "/" {
            return Err(conflict());
        }
        let remote = BigUint::from_bytes_be(&remote);
        if remote <= BigUint::from(1u8) || remote >= &prime - BigUint::from(1u8) {
            return Err(conflict());
        }
        let shared = Zeroizing::new(remote.modpow(&private, &prime).to_bytes_be());
        let mut padded = Zeroizing::new([0u8; 128]);
        padded[128 - shared.len()..].copy_from_slice(&shared);
        let mut key = Zeroizing::new([0u8; 16]);
        hkdf::Hkdf::<sha2::Sha256>::new(None, padded.as_ref())
            .expand(&[], key.as_mut())
            .map_err(|_| conflict())?;
        // Refuse owner changes during negotiation rather than activating or
        // retargeting a replacement service. Subsequent calls pin this identity.
        if Self::checked_owner(&conn, uid).await? != owner {
            return Err(unavailable());
        }
        Ok(Self {
            conn,
            owner,
            path,
            key,
        })
    }
    async fn checked_owner(conn: &Connection, uid: u32) -> AppResult<String> {
        let owner: String = call(
            conn,
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "GetNameOwner",
            &("org.freedesktop.secrets",),
        )
        .await?;
        if !owner.starts_with(':') || zbus::names::UniqueName::try_from(owner.as_str()).is_err() {
            return Err(unavailable());
        }
        let owner_uid: u32 = call(
            conn,
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "GetConnectionUnixUser",
            &(&owner,),
        )
        .await?;
        if owner_uid != uid {
            return Err(unavailable());
        }
        Ok(owner)
    }
    async fn call<B, R>(&self, path: &str, interface: &str, method: &str, body: &B) -> AppResult<R>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
        R: for<'de> zbus::zvariant::DynamicDeserialize<'de>,
    {
        call(&self.conn, &self.owner, path, interface, method, body).await
    }
    async fn properties(
        &self,
        path: &str,
        interface: &str,
    ) -> AppResult<BTreeMap<String, OwnedValue>> {
        self.call(
            path,
            "org.freedesktop.DBus.Properties",
            "GetAll",
            &(interface,),
        )
        .await
    }
    async fn search(
        &self,
        identity: &CredentialRef,
    ) -> AppResult<(Vec<OwnedObjectPath>, Vec<OwnedObjectPath>)> {
        self.call(ROOT, SERVICE, "SearchItems", &(attrs(identity),))
            .await
    }
    async fn checked_properties(&self, path: &str, identity: &CredentialRef) -> AppResult<u64> {
        let mut p = self.properties(path, ITEM).await?;
        let locked =
            bool::try_from(p.remove("Locked").ok_or_else(conflict)?).map_err(|_| conflict())?;
        if locked {
            return Err(error("KEYRING_LOCKED"));
        }
        let a: std::collections::HashMap<String, String> = p
            .remove("Attributes")
            .ok_or_else(conflict)?
            .try_into()
            .map_err(|_| conflict())?;
        // GNOME's standard schema metadata is not part of Smith's identity.
        let expected_count = match a.get("xdg:schema").map(String::as_str) {
            None => 4,
            Some("org.freedesktop.Secret.Generic") => 5,
            _ => return Err(conflict()),
        };
        if a.len() != expected_count
            || attrs(identity)
                .iter()
                .any(|(k, v)| a.get(*k).map(String::as_str) != Some(*v))
        {
            return Err(conflict());
        }
        let created =
            u64::try_from(p.remove("Created").ok_or_else(conflict)?).map_err(|_| conflict())?;
        let modified =
            u64::try_from(p.remove("Modified").ok_or_else(conflict)?).map_err(|_| conflict())?;
        // Immutable Smith items are never edited. Observable edits fail closed.
        if modified != created {
            return Err(conflict());
        }
        Ok(modified)
    }
    async fn read(&self, identity: &CredentialRef) -> AppResult<Secret> {
        let (unlocked, locked) = self.search(identity).await?;
        if unlocked.len() + locked.len() > 1 {
            return Err(conflict());
        }
        if !locked.is_empty() {
            return Err(error("KEYRING_LOCKED"));
        }
        let path = unlocked.first().ok_or_else(|| error("KEYRING_MISSING"))?;
        let before = self.checked_properties(path.as_str(), identity).await?;
        let (session, iv, encrypted, content_type): WireSecret = self
            .call(path.as_str(), ITEM, "GetSecret", &(&self.path,))
            .await?;
        // GNOME Keyring normalizes the returned MIME to text/plain. Secret::new
        // still enforces bounded printable ASCII for either supported spelling.
        if session != self.path
            || iv.len() != 16
            || encrypted.is_empty()
            || encrypted.len() > 4112
            || !matches!(
                content_type.as_str(),
                "text/plain" | "text/plain; charset=utf-8"
            )
        {
            return Err(conflict());
        }
        let plain = cbc::Decryptor::<aes::Aes128>::new_from_slices(self.key.as_ref(), &iv)
            .map_err(|_| conflict())?
            .decrypt_padded_vec_mut::<Pkcs7>(&encrypted)
            .map_err(|_| conflict())?;
        let secret = Secret::new(plain).map_err(|_| conflict())?;
        if before != self.checked_properties(path.as_str(), identity).await? {
            return Err(conflict());
        }
        let (after, locked) = self.search(identity).await?;
        if after != unlocked || !locked.is_empty() {
            return Err(conflict());
        }
        Ok(secret)
    }
    async fn save(
        &self,
        identity: &CredentialRef,
        secret: &Secret,
        attempted: &AtomicBool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> AppResult<()> {
        let (unlocked, locked) = self.search(identity).await?;
        if !unlocked.is_empty() || !locked.is_empty() {
            return Err(conflict());
        }
        let collection: OwnedObjectPath =
            self.call(ROOT, SERVICE, "ReadAlias", &("default",)).await?;
        if collection.as_str() == "/" {
            return Err(unavailable());
        }
        let mut p = self.properties(collection.as_str(), COLLECTION).await?;
        let locked =
            bool::try_from(p.remove("Locked").ok_or_else(conflict)?).map_err(|_| conflict())?;
        if locked {
            return Err(error("KEYRING_LOCKED"));
        }
        let mut iv = [0u8; 16];
        rand::rngs::OsRng
            .try_fill_bytes(&mut iv)
            .map_err(|_| unavailable())?;
        let encrypted = cbc::Encryptor::<aes::Aes128>::new_from_slices(self.key.as_ref(), &iv)
            .map_err(|_| conflict())?
            .encrypt_padded_vec_mut::<Pkcs7>(secret.expose());
        let label = format!("Smith-next {} credential", identity.provider.name());
        let properties = BTreeMap::from([
            (
                "org.freedesktop.Secret.Item.Label",
                Value::from(label.as_str()),
            ),
            (
                "org.freedesktop.Secret.Item.Attributes",
                Value::from(
                    attrs(identity)
                        .into_iter()
                        .collect::<std::collections::HashMap<_, _>>(),
                ),
            ),
        ]);
        let wire = (
            &self.path,
            iv.as_slice(),
            encrypted.as_slice(),
            "text/plain; charset=utf-8",
        );
        if cancelled.load(Ordering::SeqCst) {
            return Err(error("INTERRUPTED"));
        }
        if Instant::now() >= deadline {
            return Err(error("KEYRING_TIMEOUT"));
        }
        attempted.store(true, Ordering::SeqCst);
        let (created, prompt): (OwnedObjectPath, OwnedObjectPath) = self
            .call(
                collection.as_str(),
                COLLECTION,
                "CreateItem",
                &(properties, wire, false),
            )
            .await?;
        if prompt.as_str() != "/" {
            return Err(error("KEYRING_PROMPT_REQUIRED"));
        }
        if created.as_str() == "/" {
            return Err(conflict());
        }
        let read = self.read(identity).await?;
        if read.expose() != secret.expose() {
            return Err(conflict());
        }
        let (items, locked) = self.search(identity).await?;
        if items.as_slice() != [created] || !locked.is_empty() {
            return Err(conflict());
        }
        Ok(())
    }
}
impl CredentialStore for SecretService {
    fn read(
        &self,
        identity: &CredentialRef,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> AppResult<Secret> {
        bounded(
            deadline.min(Instant::now() + KEYRING_BUDGET),
            cancelled,
            "KEYRING_TIMEOUT",
            || async { Session::open().await?.read(identity).await },
        )
    }
    fn save(
        &self,
        identity: &CredentialRef,
        secret: &Secret,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> AppResult<()> {
        let attempted = AtomicBool::new(false);
        bounded(
            deadline.min(Instant::now() + KEYRING_BUDGET),
            cancelled,
            "KEYRING_TIMEOUT",
            || async {
                Session::open()
                    .await?
                    .save(identity, secret, &attempted, deadline, cancelled)
                    .await
            },
        )
        .map_err(|e| {
            if attempted.load(Ordering::SeqCst) {
                error("CREDENTIAL_OUTCOME_UNKNOWN")
            } else {
                e
            }
        })
    }
}
