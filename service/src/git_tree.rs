//! Host-side Git object identities for source acquisition. The exporter is untrusted:
//! the host recomputes blob/tree IDs and the commit object ID itself, so received
//! files are bound to the pinned commit rather than to exporter claims.
//! Entries carry Git tree modes: regular/executable/symlink blobs and submodule
//! (gitlink) commit IDs. Unknown modes are refused rather than normalized.
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;

pub(crate) type ObjectId = [u8; 20];
const MAX_DEPTH: usize = 64;
/// Host-enforced, independent of exporter claims; bounds memory held per entry.
pub(crate) const MAX_PATH: usize = 4096;
const MAX_COMPONENT: usize = 255;
pub(crate) const REGULAR: u32 = 0o100644;
pub(crate) const EXECUTABLE: u32 = 0o100755;
pub(crate) const SYMLINK: u32 = 0o120000;
pub(crate) const GITLINK: u32 = 0o160000;

pub(crate) fn object_id(kind: &str, body: &[u8]) -> ObjectId {
    let mut hasher = Sha1::new();
    hasher.update(format!("{kind} {}\0", body.len()).as_bytes());
    hasher.update(body);
    hasher.finalize().into()
}
pub(crate) fn hex(id: &ObjectId) -> String {
    id.iter().map(|b| format!("{b:02x}")).collect()
}
pub(crate) fn parse(value: &[u8]) -> Option<ObjectId> {
    if value.len() != 40 {
        return None;
    }
    let mut id = [0u8; 20];
    for (i, pair) in value.chunks(2).enumerate() {
        let digit = |c: u8| match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            _ => None,
        };
        id[i] = digit(pair[0])? << 4 | digit(pair[1])?;
    }
    Some(id)
}
/// Verifies the raw commit object hashes to `commit` and returns its root tree.
pub(crate) fn commit_tree(commit: &str, raw: &[u8]) -> Option<ObjectId> {
    if hex(&object_id("commit", raw)) != commit {
        return None;
    }
    let rest = raw.strip_prefix(b"tree ")?;
    if rest.get(40) != Some(&b'\n') {
        return None;
    }
    parse(&rest[..40])
}

enum Node {
    Entry { mode: u32, id: ObjectId },
    Tree(Tree),
}
#[derive(Default)]
pub(crate) struct Tree {
    entries: BTreeMap<String, Node>,
}
fn component(part: &str) -> bool {
    !part.is_empty()
        && part.len() <= MAX_COMPONENT
        && part != "."
        && part != ".."
        && !part.contains('\0')
}
impl Tree {
    /// Adds one entry. Rejects unknown modes, malformed paths, duplicates and
    /// file/directory conflicts.
    pub(crate) fn insert(&mut self, path: &str, mode: u32, id: ObjectId) -> bool {
        let parts: Vec<&str> = path.split('/').collect();
        if !matches!(mode, REGULAR | EXECUTABLE | SYMLINK | GITLINK)
            || path.len() > MAX_PATH
            || parts.len() > MAX_DEPTH
            || !parts.iter().all(|p| component(p))
        {
            return false;
        }
        let (name, parents) = parts.split_last().expect("split yields one part");
        let mut tree = self;
        for part in parents {
            let node = tree
                .entries
                .entry((*part).into())
                .or_insert_with(|| Node::Tree(Tree::default()));
            match node {
                Node::Tree(child) => tree = child,
                Node::Entry { .. } => return false,
            }
        }
        if tree.entries.contains_key(*name) {
            return false;
        }
        tree.entries
            .insert((*name).into(), Node::Entry { mode, id });
        true
    }
    pub(crate) fn id(&self) -> ObjectId {
        // Git orders entries by name bytes, comparing tree names as if suffixed by '/'.
        let mut entries: Vec<(Vec<u8>, &str, &Node)> = self
            .entries
            .iter()
            .map(|(name, node)| {
                let mut key = name.as_bytes().to_vec();
                if matches!(node, Node::Tree(_)) {
                    key.push(b'/');
                }
                (key, name.as_str(), node)
            })
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        let mut body = Vec::new();
        for (_, name, node) in entries {
            let (mode, id) = match node {
                Node::Entry { mode, id } => (format!("{mode:o}"), *id),
                Node::Tree(tree) => ("40000".into(), tree.id()),
            };
            body.extend_from_slice(mode.as_bytes());
            body.push(b' ');
            body.extend_from_slice(name.as_bytes());
            body.push(0);
            body.extend_from_slice(&id);
        }
        object_id("tree", &body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected IDs were produced by `git hash-object`/`git write-tree` and are fixed constants.
    #[test]
    fn blob_and_empty_tree_ids_match_git() {
        assert_eq!(
            hex(&object_id("blob", b"hello\n")),
            "ce013625030ba8dba906f756967f9e9ca394464a"
        );
        assert_eq!(
            hex(&Tree::default().id()),
            "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
        );
    }
    #[test]
    fn nested_tree_order_and_modes_match_git() {
        // a.c, a/b (tree sorts as "a/"), exec.sh executable.
        let mut tree = Tree::default();
        assert!(tree.insert("a/b", REGULAR, object_id("blob", b"b\n")));
        assert!(tree.insert("a.c", REGULAR, object_id("blob", b"c\n")));
        assert!(tree.insert("exec.sh", EXECUTABLE, object_id("blob", b"#!/bin/sh\n")));
        assert_eq!(hex(&tree.id()), "cf07e13c34537ca7d60bccde6d94226c7821e71c");
    }
    #[test]
    fn symlink_and_submodule_entries_match_git() {
        let mut tree = Tree::default();
        assert!(tree.insert("f", REGULAR, object_id("blob", b"x\n")));
        assert!(tree.insert("link", SYMLINK, object_id("blob", b"f")));
        assert!(tree.insert("sub", GITLINK, [0x11; 20]));
        assert_eq!(hex(&tree.id()), "0c94cb3d16cf40714439f9b8ad4592645202b891");
    }
    #[test]
    fn rejects_duplicates_conflicts_and_malformed_paths() {
        let id = object_id("blob", b"");
        let mut tree = Tree::default();
        assert!(tree.insert("dir/file", REGULAR, id));
        assert!(!tree.insert("dir/file", REGULAR, id));
        assert!(!tree.insert("dir", REGULAR, id));
        assert!(!tree.insert("dir/file/child", REGULAR, id));
        for bad in ["", "/abs", "a//b", "a/./b", "a/../b", "trailing/", "nul\0"] {
            assert!(!tree.insert(bad, REGULAR, id), "{bad:?}");
        }
        assert!(!tree.insert(&"a".repeat(256), REGULAR, id));
        let long = vec!["a".repeat(255); 17].join("/");
        assert!(long.len() > MAX_PATH && !tree.insert(&long, REGULAR, id));
        assert!(tree.insert(&"a".repeat(255), REGULAR, id));
        for mode in [0o40000, 0o100664, 0o100600, 0] {
            assert!(!tree.insert("other", mode, id), "{mode:o}");
        }
    }
    #[test]
    fn commit_object_must_hash_to_pin_and_name_a_tree() {
        let raw = b"tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\nauthor A <a@invalid> 0 +0000\ncommitter A <a@invalid> 0 +0000\n\nm\n";
        let pin = hex(&object_id("commit", raw));
        assert_eq!(
            commit_tree(&pin, raw).map(|t| hex(&t)).as_deref(),
            Some("4b825dc642cb6eb9a060e54bf8d69288fbee4904")
        );
        assert_eq!(commit_tree(&"0".repeat(40), raw), None);
        let upper = b"tree 4B825DC642CB6EB9A060E54BF8D69288FBEE4904\n";
        assert_eq!(commit_tree(&hex(&object_id("commit", upper)), upper), None);
        let short = b"tree 4b825dc6\n";
        assert_eq!(commit_tree(&hex(&object_id("commit", short)), short), None);
    }
}
