//! Canonical release identities shared with the original publisher.

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const MANIFEST: &str = ".smith-next-manifest.json";
pub const APP: &str = "dev.smith.smith_client";

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    Directory,
    File,
}

// Field order is intentional: Python's sort_keys=True orders every object,
// not just the path map. Do not depend on serde_json's preserve_order feature.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Entry {
    pub mode: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(rename = "type")]
    pub kind: EntryKind,
}

pub type Entries = BTreeMap<String, Entry>;

impl Entry {
    pub(crate) fn directory(mode: u32) -> Self {
        Self {
            mode,
            sha256: None,
            kind: EntryKind::Directory,
        }
    }

    pub(crate) fn file(mode: u32, bytes: &[u8]) -> Self {
        Self {
            mode,
            sha256: Some(digest(bytes)),
            kind: EntryKind::File,
        }
    }
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Byte-identical to Python's compact, recursively sorted schema-1 JSON + LF.
pub fn manifest_bytes(entries: &Entries) -> Vec<u8> {
    #[derive(Serialize)]
    struct Manifest<'a> {
        entries: &'a Entries,
        schema: u8,
    }
    let mut bytes = serde_json::to_vec(&Manifest { entries, schema: 1 })
        .expect("a manifest contains only serializable strings and integers");
    bytes.push(b'\n');
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_python_bytes_and_identity() {
        let entries = BTreeMap::from([
            ("z".into(), Entry::directory(0o755)),
            ("A".into(), Entry::file(0o644, b"abc")),
        ]);
        let expected = b"{\"entries\":{\"A\":{\"mode\":420,\"sha256\":\"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad\",\"type\":\"file\"},\"z\":{\"mode\":493,\"type\":\"directory\"}},\"schema\":1}\n";
        assert_eq!(manifest_bytes(&entries), expected);
        assert_eq!(
            digest(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            manifest_bytes(&Entries::new()),
            b"{\"entries\":{},\"schema\":1}\n"
        );
    }
}
