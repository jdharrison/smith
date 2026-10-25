//! Descriptor-relative, single-use reader. Authorization is performed by the host before `read`.
use rand::{rngs::OsRng, RngCore};
use serde_json::Value;
use std::collections::HashSet;
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::sync::Mutex;

type Result<T> = std::result::Result<T, &'static str>;
const DENIED: &str = "read_denied";
const OUTPUT: usize = 2048;

fn open_at(parent: &File, name: &str, flags: i32) -> Result<File> {
    let name = CString::new(name).map_err(|_| DENIED)?;
    // SAFETY: parent is live, name is NUL terminated; the returned fd has unique ownership.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
        )
    };
    if fd < 0 {
        return Err(DENIED);
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

pub(crate) fn directory(path: &Path) -> Result<File> {
    let mut fd = File::open("/").map_err(|_| DENIED)?;
    for component in path.components() {
        match component {
            std::path::Component::RootDir => (),
            std::path::Component::Normal(name) => {
                fd = open_at(
                    &fd,
                    name.to_str().ok_or(DENIED)?,
                    libc::O_RDONLY | libc::O_DIRECTORY,
                )?;
            }
            _ => return Err(DENIED),
        }
    }
    Ok(fd)
}

fn entries(fd: &File, max: usize) -> Result<Vec<String>> {
    // A new open-file description avoids sharing readdir offsets with the fixed root descriptor.
    let owned = open_at(fd, ".", libc::O_RDONLY | libc::O_DIRECTORY)?;
    use std::os::fd::IntoRawFd;
    let raw = owned.into_raw_fd();
    let dir = unsafe { libc::fdopendir(raw) };
    if dir.is_null() {
        unsafe {
            libc::close(raw);
        }
        return Err(DENIED);
    }
    struct Dir(*mut libc::DIR);
    impl Drop for Dir {
        fn drop(&mut self) {
            unsafe {
                libc::closedir(self.0);
            }
        }
    }
    let dir = Dir(dir);
    let mut names = Vec::new();
    loop {
        unsafe {
            *libc::__errno_location() = 0;
        }
        let entry = unsafe { libc::readdir(dir.0) };
        if entry.is_null() {
            if unsafe { *libc::__errno_location() } != 0 {
                return Err(DENIED);
            }
            break;
        }
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }
            .to_str()
            .map_err(|_| DENIED)?;
        if name == "." || name == ".." {
            continue;
        }
        if name.chars().any(char::is_control) || name.contains('\\') {
            return Err(DENIED);
        }
        names.push(name.to_owned());
        if names.len() > max {
            return Err(DENIED);
        }
    }
    names.sort();
    Ok(names)
}

#[derive(Default)]
struct Budget {
    calls: HashSet<String>,
    bytes: usize,
}
pub(crate) struct Reader {
    roots: Vec<File>,
    budget: Mutex<Budget>,
    stage: File,
    stage_path: String,
}
pub(crate) struct Staged {
    pub path: String,
    pub text: String,
    pub title: String,
}
impl Reader {
    /// The caller has sealed the roots before constructing this reader.
    pub(crate) fn new(roots: Vec<File>) -> Result<Self> {
        Self::with_stage(roots, Path::new("/tmp"), "smith-read-results")
    }
    fn with_stage(roots: Vec<File>, parent: &Path, name: &str) -> Result<Self> {
        // Validate every ancestor, not just the private leaf. Only root-owned sticky /tmp
        // may be shared; no-follow traversal below independently rejects substituted links.
        let mut prefix = std::path::PathBuf::from("/");
        for component in parent.components().skip(1) {
            prefix.push(component);
            let meta = std::fs::symlink_metadata(&prefix).map_err(|_| DENIED)?;
            let sticky_tmp =
                prefix == Path::new("/tmp") && meta.uid() == 0 && meta.mode() & 0o1000 != 0;
            if !meta.is_dir()
                || meta.file_type().is_symlink()
                || (meta.uid() != 0 && meta.uid() != unsafe { libc::geteuid() })
                || (meta.mode() & 0o022 != 0 && !sticky_tmp)
            {
                return Err(DENIED);
            }
        }
        let parent_fd = directory(parent)?;
        let name_c = CString::new(name).map_err(|_| DENIED)?;
        if unsafe { libc::mkdirat(parent_fd.as_raw_fd(), name_c.as_ptr(), 0o700) } != 0 {
            return Err(DENIED);
        }
        let stage = open_at(&parent_fd, name, libc::O_RDONLY | libc::O_DIRECTORY)?;
        let meta = stage.metadata().map_err(|_| DENIED)?;
        if meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o777 != 0o700 {
            return Err(DENIED);
        }
        Ok(Self {
            roots,
            budget: Mutex::new(Budget::default()),
            stage,
            stage_path: parent.join(name).to_str().ok_or(DENIED)?.into(),
        })
    }
    pub(crate) fn read(&self, call: &str, args: &Value) -> Result<Staged> {
        // Reserve the full possible output before *any* operation, including argument/path errors.
        {
            let mut budget = self.budget.lock().map_err(|_| DENIED)?;
            if call.is_empty()
                || call.len() > 128
                || budget.calls.len() >= 16
                || budget.bytes + OUTPUT > 32768
                || !budget.calls.insert(call.into())
            {
                return Err(DENIED);
            }
            budget.bytes += OUTPUT;
        }
        let object = args.as_object().ok_or(DENIED)?;
        if object
            .keys()
            .any(|k| !["filePath", "offset", "limit"].contains(&k.as_str()))
        {
            return Err(DENIED);
        }
        let path = args["filePath"].as_str().ok_or(DENIED)?;
        let parts: Vec<_> = path.split('/').collect();
        if path.len() > 4096
            || parts.len() < 4
            || parts.len() > 36
            || parts[..3] != ["", "workspace", "repos"]
        {
            return Err(DENIED);
        }
        let index: usize = parts[3]
            .strip_prefix('r')
            .ok_or(DENIED)?
            .parse()
            .map_err(|_| DENIED)?;
        if parts[3] != format!("r{index}") {
            return Err(DENIED);
        }
        let mut fd = self
            .roots
            .get(index)
            .ok_or(DENIED)?
            .try_clone()
            .map_err(|_| DENIED)?;
        for (i, part) in parts[4..].iter().enumerate() {
            if part.is_empty()
                || *part == "."
                || *part == ".."
                || part.eq_ignore_ascii_case(".git")
                || part.contains('\\')
                || part.chars().any(char::is_control)
            {
                return Err(DENIED);
            }
            fd = open_at(
                &fd,
                part,
                libc::O_RDONLY
                    | if i + 5 < parts.len() {
                        libc::O_DIRECTORY
                    } else {
                        0
                    },
            )?;
        }
        let number = |key: &str, default: u64, max: u64| -> Result<usize> {
            let n = match args.get(key) {
                Some(v) => v.as_u64().ok_or(DENIED)?,
                None => default,
            };
            if !(1..=max).contains(&n) {
                return Err(DENIED);
            }
            Ok(n as usize)
        };
        let offset = number("offset", 1, 20000)?;
        let limit = number("limit", 30, 32)?.min(30);
        let meta = fd.metadata().map_err(|_| DENIED)?;
        let title = format!("{}/{}", parts[3], parts[4..].join("/"));
        if title.len() > 1024 {
            return Err(DENIED);
        }
        let mut text = format!("{title}\n");
        let lines: Vec<String> = if meta.is_dir() {
            entries(&fd, 256)?
                .into_iter()
                .filter(|n| !n.eq_ignore_ascii_case(".git"))
                .collect()
        } else if meta.is_file() && meta.nlink() == 1 && meta.len() <= 256 * 1024 {
            let mut bytes = Vec::new();
            (&mut fd)
                .take(256 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| DENIED)?;
            if bytes.len() > 256 * 1024 {
                return Err(DENIED);
            }
            let source = std::str::from_utf8(&bytes).map_err(|_| DENIED)?;
            if source
                .chars()
                .any(|c| c.is_control() && !['\n', '\r', '\t'].contains(&c))
                || attachment(path)
            {
                return Err(DENIED);
            }
            source.lines().map(str::to_owned).collect()
        } else {
            return Err(DENIED);
        };
        if offset > lines.len().max(1) {
            return Err(DENIED);
        }
        for (i, line) in lines.iter().enumerate().skip(offset - 1).take(limit) {
            let rendered = format!("{}: {}\n", i + 1, line);
            if text.len() + rendered.len() > OUTPUT {
                let left = OUTPUT.saturating_sub(text.len() + 5);
                let mut end = left.min(rendered.len());
                while !rendered.is_char_boundary(end) {
                    end -= 1;
                }
                text.push_str(&rendered[..end]);
                text.push_str("...\n");
                break;
            }
            text.push_str(&rendered);
        }
        let mut random = [0u8; 24];
        OsRng.try_fill_bytes(&mut random).map_err(|_| DENIED)?;
        let name: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let name_c = CString::new(name.as_str()).map_err(|_| DENIED)?;
        let raw = unsafe {
            libc::openat(
                self.stage.as_raw_fd(),
                name_c.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if raw < 0 {
            return Err(DENIED);
        }
        let mut file = unsafe { File::from_raw_fd(raw) };
        file.write_all(text.as_bytes()).map_err(|_| DENIED)?;
        file.sync_all().map_err(|_| DENIED)?;
        Ok(Staged {
            path: format!("{}/{name}", self.stage_path),
            text,
            title,
        })
    }
}
fn attachment(path: &str) -> bool {
    let ext = Path::new(path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    [
        "png", "jpg", "jpeg", "gif", "webp", "svg", "pdf", "zip", "gz", "tar", "wasm", "exe", "so",
        "o", "a", "bin", "mp3", "mp4", "woff", "woff2", "ttf",
    ]
    .contains(&ext.as_str())
}

/// Account and seal all expanded content (including Git metadata) before private context arrives.
pub(crate) fn seal(roots: &[File]) -> Result<()> {
    fn walk(fd: &File, depth: usize, count: &mut usize, bytes: &mut u64) -> Result<()> {
        *count += 1;
        if depth > 32 || *count > 20000 {
            return Err(DENIED);
        }
        let meta = fd.metadata().map_err(|_| DENIED)?;
        if meta.is_dir() {
            for name in entries(fd, 20000)? {
                let child = open_at(fd, &name, libc::O_RDONLY)?;
                walk(&child, depth + 1, count, bytes)?;
            }
        } else if meta.is_file() && meta.nlink() == 1 {
            *bytes = bytes.checked_add(meta.len()).ok_or(DENIED)?;
            if *bytes > 128 * 1024 * 1024 {
                return Err(DENIED);
            }
        } else {
            return Err(DENIED);
        }
        fd.set_permissions(std::fs::Permissions::from_mode(if meta.is_dir() {
            0o500
        } else {
            0o400
        }))
        .map_err(|_| DENIED)
    }
    let (mut count, mut bytes) = (0, 0);
    for fd in roots {
        walk(fd, 0, &mut count, &mut bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::os::unix::fs::symlink;
    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            let mut bytes = [0; 16];
            OsRng.fill_bytes(&mut bytes);
            let p = std::env::temp_dir()
                .join(format!("smith-reader-{:032x}", u128::from_le_bytes(bytes)));
            std::fs::create_dir(&p).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700)).unwrap();
            Self(p)
        }
        fn reader(&self) -> Reader {
            Reader::with_stage(vec![directory(&self.0).unwrap()], &self.0, "stage").unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o700));
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn text_range_staging_and_single_use() {
        let f = Fixture::new();
        std::fs::write(f.0.join("a"), "one\ntwo\nthree\n").unwrap();
        let r = f.reader();
        let args = json!({"filePath":"/workspace/repos/r0/a","offset":2,"limit":1});
        let s = r.read("../../not-a-path", &args).unwrap();
        assert_eq!(s.text, "r0/a\n2: two\n");
        assert!(!s.path.contains("not-a-path"));
        assert_eq!(std::fs::metadata(&s.path).unwrap().mode() & 0o777, 0o600);
        assert_eq!(std::fs::read_to_string(s.path).unwrap(), s.text);
        assert!(r.read("../../not-a-path", &args).is_err());
        assert!(Reader::with_stage(vec![], &f.0, "stage").is_err());
    }
    #[test]
    fn exact_checkout_root_enumeration_uses_fixed_descriptor_and_bounded_ranges() {
        let f = Fixture::new();
        let root = f.0.join("checkout");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(root.join(".git")).unwrap();
        std::fs::create_dir(root.join("src")).unwrap();
        std::fs::write(root.join("README.md"), "safe").unwrap();
        let reader = Reader::with_stage(vec![directory(&root).unwrap()], &f.0, "stage").unwrap();
        let result = reader
            .read("root", &json!({"filePath":"/workspace/repos/r0"}))
            .unwrap();
        assert_eq!(result.title, "r0/");
        assert_eq!(result.text, "r0/\n1: README.md\n2: src\n");
        assert_eq!(std::fs::read_to_string(result.path).unwrap(), result.text);
        let range = reader
            .read(
                "range",
                &json!({"filePath":"/workspace/repos/r0","offset":2,"limit":1}),
            )
            .unwrap();
        assert_eq!(range.text, "r0/\n2: src\n");
        for (i, path) in [
            "/workspace/repos",
            "/workspace/repos/r1",
            "/workspace/repos/r0/",
            "/workspace/repos/r0/..",
            "/workspace/repos/r0/.git",
        ]
        .iter()
        .enumerate()
        {
            assert!(reader
                .read(&format!("bad_{i}"), &json!({"filePath":path}))
                .is_err());
        }
    }
    #[test]
    fn rejects_links_binary_special_and_bad_ranges() {
        let f = Fixture::new();
        std::fs::write(f.0.join("binary"), b"a\0b").unwrap();
        std::fs::write(f.0.join("text"), "safe").unwrap();
        symlink("text", f.0.join("link")).unwrap();
        std::fs::create_dir(f.0.join(".git")).unwrap();
        let fifo = CString::new(f.0.join("fifo").to_str().unwrap()).unwrap();
        unsafe {
            libc::mkfifo(fifo.as_ptr(), 0o600);
        }
        let r = f.reader();
        for (i, name) in ["binary", "link", ".git/config", "../text", "fifo"]
            .iter()
            .enumerate()
        {
            assert!(r
                .read(
                    &format!("c{i}"),
                    &json!({"filePath":format!("/workspace/repos/r0/{name}")})
                )
                .is_err());
        }
        assert!(r
            .read(
                "range",
                &json!({"filePath":"/workspace/repos/r0/text","offset":0})
            )
            .is_err());
    }
    #[test]
    fn quotas_are_reserved_before_failed_io_and_output_is_bounded() {
        let f = Fixture::new();
        std::fs::write(f.0.join("long"), "é".repeat(10000)).unwrap();
        let r = f.reader();
        let s = r
            .read("ok", &json!({"filePath":"/workspace/repos/r0/long"}))
            .unwrap();
        assert!(s.text.len() <= 2048);
        assert!(s.text.lines().count() <= 32);
        for i in 1..16 {
            assert!(r
                .read(
                    &format!("c{i}"),
                    &json!({"filePath":"/workspace/repos/r0/missing"})
                )
                .is_err());
        }
        assert!(r
            .read("last", &json!({"filePath":"/workspace/repos/r0/long"}))
            .is_err());
    }
    #[test]
    fn concurrent_reservations_cannot_exceed_quota_or_reuse_calls() {
        let f = Fixture::new();
        std::fs::write(f.0.join("a"), "safe\n").unwrap();
        let reader = f.reader();
        let successes = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..24)
                .map(|i| {
                    let reader = &reader;
                    scope.spawn(move || {
                        reader
                            .read(
                                &format!("call_{i}"),
                                &json!({"filePath":"/workspace/repos/r0/a"}),
                            )
                            .is_ok()
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| usize::from(h.join().unwrap()))
                .sum::<usize>()
        });
        assert_eq!(successes, 16);
        assert_eq!(reader.budget.lock().unwrap().bytes, 32768);
    }
    #[test]
    fn expanded_tree_seals_and_rejects_symlinks() {
        let f = Fixture::new();
        std::fs::write(f.0.join("a"), "safe").unwrap();
        seal(&[directory(&f.0).unwrap()]).unwrap();
        assert_eq!(std::fs::metadata(&f.0).unwrap().mode() & 0o777, 0o500);
        assert_eq!(
            std::fs::metadata(f.0.join("a")).unwrap().mode() & 0o777,
            0o400
        );
        let other = Fixture::new();
        symlink("/", other.0.join("escape")).unwrap();
        assert!(seal(&[directory(&other.0).unwrap()]).is_err());
    }
    #[test]
    fn untrusted_stage_parent_and_existing_stage_refuse() {
        let f = Fixture::new();
        std::fs::set_permissions(&f.0, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(Reader::with_stage(vec![], &f.0, "stage").is_err());
        std::fs::set_permissions(&f.0, std::fs::Permissions::from_mode(0o700)).unwrap();
        symlink("/tmp", f.0.join("stage")).unwrap();
        assert!(Reader::with_stage(vec![], &f.0, "stage").is_err());
    }
    #[test]
    fn directories_and_depth_are_bounded() {
        let f = Fixture::new();
        std::fs::create_dir(f.0.join("many")).unwrap();
        for i in 0..257 {
            std::fs::write(f.0.join("many").join(i.to_string()), "").unwrap();
        }
        let r = f.reader();
        assert!(r
            .read("dir", &json!({"filePath":"/workspace/repos/r0/many"}))
            .is_err());
        assert!(r
            .read(
                "deep",
                &json!({"filePath":format!("/workspace/repos/r0/{}", vec!["a";33].join("/"))})
            )
            .is_err());
    }
}
