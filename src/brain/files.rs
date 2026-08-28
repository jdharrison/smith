//! Host-isolation filesystem find + OS preview.
//!
//! Sealed mode must not call this. Roots are workspaces + Pictures/Documents, or `host.roots`.

use crate::{host_file_roots, load_config, path_is_under_roots};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const MAX_HITS: usize = 20;
const MAX_WALK: usize = 8_000;
const MAX_DEPTH: u32 = 12;

const SKIP_DIR_NAMES: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "__pycache__",
    ".cache",
    "$Recycle.Bin",
    "System Volume Information",
    "Windows",
];

const IMAGE_EXTS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "tif", "tiff", "heic", "avif",
];

#[derive(Debug, Clone)]
pub struct FileHit {
    pub path: PathBuf,
    pub name: String,
}

impl FileHit {
    pub fn is_image(&self) -> bool {
        self.path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| IMAGE_EXTS.iter().any(|x| e.eq_ignore_ascii_case(x)))
            .unwrap_or(false)
    }
}

pub fn current_roots() -> Result<Vec<PathBuf>, String> {
    let cfg = load_config()?;
    Ok(host_file_roots(&cfg))
}

/// Case-insensitive filename match (`*` allowed). Capped walk.
pub fn find_files(query: &str) -> Result<Vec<FileHit>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Err("file query is empty".to_string());
    }
    let needle = q.replace('\\', "/");
    let needle_l = needle.to_ascii_lowercase();
    let globish = needle_l.contains('*');

    let roots = current_roots()?;
    if roots.is_empty() {
        return Err(
            "no host file roots (add a workspace or set host.roots in config)".to_string(),
        );
    }

    let mut hits = Vec::new();
    let mut walked = 0usize;
    for root in &roots {
        walk(
            root,
            0,
            &needle_l,
            globish,
            &mut hits,
            &mut walked,
        );
        if hits.len() >= MAX_HITS || walked >= MAX_WALK {
            break;
        }
    }
    Ok(hits)
}

fn walk(
    dir: &Path,
    depth: u32,
    needle_l: &str,
    globish: bool,
    hits: &mut Vec<FileHit>,
    walked: &mut usize,
) {
    if hits.len() >= MAX_HITS || *walked >= MAX_WALK || depth > MAX_DEPTH {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for ent in entries.flatten() {
        if hits.len() >= MAX_HITS || *walked >= MAX_WALK {
            return;
        }
        *walked += 1;
        let path = ent.path();
        let name = ent.file_name().to_string_lossy().to_string();
        let ft = match ent.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if ft.is_dir() {
            if SKIP_DIR_NAMES.iter().any(|s| name.eq_ignore_ascii_case(s)) {
                continue;
            }
            if name.starts_with('.') && name != "." {
                continue;
            }
            walk(&path, depth + 1, needle_l, globish, hits, walked);
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        let name_l = name.to_ascii_lowercase();
        let matched = if globish {
            glob_match(needle_l, &name_l)
        } else {
            name_l == needle_l || name_l.contains(needle_l)
        };
        if matched {
            hits.push(FileHit {
                path: path.canonicalize().unwrap_or(path),
                name,
            });
        }
    }
}

fn glob_match(pat_l: &str, name_l: &str) -> bool {
    let pat = pat_l.rsplit('/').next().unwrap_or(pat_l);
    if let Some(stem) = pat.strip_prefix('*') {
        return name_l.ends_with(stem);
    }
    if let Some(stem) = pat.strip_suffix('*') {
        return name_l.starts_with(stem);
    }
    if let Some((a, b)) = pat.split_once('*') {
        return name_l.starts_with(a) && name_l.ends_with(b);
    }
    name_l == pat || name_l.contains(pat)
}

pub fn open_path(path: &Path) -> Result<(), String> {
    let roots = current_roots()?;
    if !path_is_under_roots(path, &roots) {
        return Err("path is outside host file roots".to_string());
    }
    if !path.is_file() {
        return Err(format!("not a file: {}", path.display()));
    }
    open_os(path)
}

fn open_os(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", &path.to_string_lossy()])
            .creation_flags(CREATE_NO_WINDOW);
        cmd.spawn()
            .map_err(|e| format!("failed to open: {}", e))?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("failed to open: {}", e))?;
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("failed to open: {}", e))?;
        Ok(())
    }
}

pub fn format_hits(hits: &[FileHit]) -> String {
    if hits.is_empty() {
        return "(no matches under host file roots)".to_string();
    }
    let mut out = String::new();
    for (i, h) in hits.iter().enumerate() {
        out.push_str(&format!("{}. {}\n   {}\n", i + 1, h.name, h.display_path()));
    }
    out
}

impl FileHit {
    fn display_path(&self) -> String {
        self.path.display().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_suffix_and_contains() {
        assert!(glob_match("x.jpg", "x.jpg"));
        assert!(glob_match("*.jpg", "photo.jpg"));
        assert!(glob_match("photo*", "photo.jpg"));
        assert!(!glob_match("*.png", "photo.jpg"));
    }

    #[test]
    fn skip_list_includes_git() {
        assert!(SKIP_DIR_NAMES.contains(&".git"));
    }
}
