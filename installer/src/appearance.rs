//! GUI-only, installer-owned resources. Import from `ui.rs`, never the CLI/library.
//!
//! Call `load_fonts` on the GUI thread before creating widgets or applying CSS,
//! after the help/version dispatch. Keep its handle through GTK shutdown. The
//! process-lifetime registration also prevents stale Fontconfig paths if a caller
//! accidentally drops the handle. No payload lookup, font installation or download.
#![cfg(feature = "gui")]

use gtk::gdk_pixbuf::{Pixbuf, PixbufLoader};
use gtk::prelude::*;
use std::ffi::{c_int, c_void, CString};
use std::fs::File;
use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd};
use std::sync::OnceLock;

pub const FONT_FAMILY: &str = "JetBrains Mono";
pub const ICON_SIZE: i32 = 24;

const FONTS: [&[u8]; 3] = [
    include_bytes!("../assets/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf"),
    include_bytes!("../assets/fonts/jetbrains-mono/JetBrainsMono-Medium.ttf"),
    include_bytes!("../assets/fonts/jetbrains-mono/JetBrainsMono-SemiBold.ttf"),
];
const SEALS: c_int =
    libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;

struct Registration {
    _files: [File; 3],
    result: Result<(), String>,
}

// Fontconfig retains filenames, not our descriptors. Retain exactly three sealed
// files even on partial registration failure: closing/reusing an fd would make a
// registered /proc/self/fd path point at unrelated data. Never clear other app fonts.
static REGISTRATION: OnceLock<Result<Registration, String>> = OnceLock::new();

#[must_use = "retain the font resources for the GUI lifetime"]
pub struct FontAssets {
    _registration: &'static Registration,
}

/// Register the three embedded 400/500/600 faces, once per process.
///
/// Uses Linux sealed, close-on-exec memfds and Fontconfig application fonts, not
/// persistent files or font caches. Repeat calls reuse the same descriptors; an
/// initialization failure is retained, not retried. Pango's caller-thread map is
/// refreshed so a previously cached system font cannot hide the bundled family.
pub fn load_fonts() -> Result<FontAssets, String> {
    let registration = REGISTRATION
        .get_or_init(|| {
            let files = [
                memory_font(FONTS[0])?,
                memory_font(FONTS[1])?,
                memory_font(FONTS[2])?,
            ];
            let result = register_fonts(&files);
            Ok(Registration {
                _files: files,
                result,
            })
        })
        .as_ref()
        .map_err(Clone::clone)?;
    registration.result.clone()?;
    refresh_font_map()?;
    Ok(FontAssets {
        _registration: registration,
    })
}

fn memory_font(bytes: &[u8]) -> Result<File, String> {
    // SAFETY: static NUL-terminated name; success returns a new owned descriptor.
    let fd = unsafe {
        libc::memfd_create(
            c"smith-installer-font".as_ptr(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
        )
    };
    if fd < 0 {
        return Err("Cannot create in-memory installer font (Linux memfd required)".into());
    }
    // SAFETY: this descriptor was just created and has no other owner.
    let mut file = unsafe { File::from_raw_fd(fd) };
    file.write_all(bytes)
        .map_err(|_| "Cannot populate in-memory installer font")?;
    // SAFETY: the file is live and owned here; no writable mapping exists.
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_ADD_SEALS, SEALS) } < 0 {
        return Err("Cannot seal in-memory installer font".into());
    }
    Ok(file)
}

fn register_fonts(files: &[File; 3]) -> Result<(), String> {
    // SAFETY: Fontconfig owns its process-local current configuration. GTK uses
    // this configuration on the supported Ubuntu/PangoFc desktop backend.
    let config = unsafe { native::FcConfigGetCurrent() };
    if config.is_null() {
        return Err("Cannot initialize installer Fontconfig".into());
    }
    for file in files {
        let path = CString::new(format!("/proc/self/fd/{}", file.as_raw_fd()))
            .expect("integer-only memfd path");
        // SAFETY: config is live, path is NUL-terminated, and the sealed file is
        // retained by REGISTRATION even if this or a later registration fails.
        if unsafe { native::FcConfigAppFontAddFile(config, path.as_ptr().cast()) } == 0 {
            return Err(format!(
                "Cannot register embedded {FONT_FAMILY}; readable /proc/self/fd is required"
            ));
        }
    }
    Ok(())
}

fn refresh_font_map() -> Result<(), String> {
    // SAFETY: PangoCairo returns a borrowed caller-thread default map. Check its
    // runtime type before using the PangoFc API; no GTK/display initialization.
    unsafe {
        let map = native::pango_cairo_font_map_get_default();
        if map.is_null()
            || gtk::glib::gobject_ffi::g_type_check_instance_is_a(
                map.cast(),
                native::pango_fc_font_map_get_type(),
            ) == 0
        {
            return Err("Installer fonts require the Ubuntu Pango Fontconfig backend".into());
        }
        native::pango_fc_font_map_config_changed(map.cast());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Icon {
    Archive,
    Trash,
    Check,
    X,
}

impl Icon {
    /// Exact reviewed source names only; no theme, filesystem or payload lookup.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "archive" => Some(Self::Archive),
            "trash-2" => Some(Self::Trash),
            "check" => Some(Self::Check),
            "x" => Some(Self::X),
            _ => None,
        }
    }

    fn png(self) -> &'static [u8] {
        match self {
            Self::Archive => include_bytes!("../assets/icons/archive.png"),
            Self::Trash => include_bytes!("../assets/icons/trash-2.png"),
            Self::Check => include_bytes!("../assets/icons/check.png"),
            Self::X => include_bytes!("../assets/icons/x.png"),
        }
    }
}

fn icon_pixbuf(icon: Icon) -> Result<Pixbuf, String> {
    // Select PNG explicitly; recipients never need an SVG decoder or icon theme.
    let loader = PixbufLoader::with_type("png").map_err(|_| "GTK PNG decoder unavailable")?;
    let written = loader.write(icon.png());
    let closed = loader.close();
    written
        .and(closed)
        .map_err(|_| "Cannot decode embedded installer icon")?;
    let pixbuf = loader.pixbuf().ok_or("Embedded installer icon is empty")?;
    if (pixbuf.width(), pixbuf.height()) != (ICON_SIZE, ICON_SIZE) {
        return Err("Embedded installer icon must be 24 × 24 pixels".into());
    }
    Ok(pixbuf)
}

/// A decorative 24px image in fixed #E3F0EE, created on the initialized GTK thread.
/// Callers own labels/accessibility, contrast, tooltips and at least 48px targets.
/// Missing PNG support is a recipient prerequisite error, not a theme fallback.
pub fn icon_image(icon: Icon) -> gtk::Image {
    let pixbuf = icon_pixbuf(icon).expect("GTK must decode the checked embedded installer PNGs");
    gtk::Image::from_pixbuf(Some(&pixbuf))
}

// These are already GTK3 runtime dependencies on supported Ubuntu. Keep the FFI
// private and minimal rather than introducing a new Rust font/renderer crate.
mod native {
    use super::{c_int, c_void};

    #[link(name = "fontconfig")]
    extern "C" {
        pub fn FcConfigGetCurrent() -> *mut c_void;
        pub fn FcConfigAppFontAddFile(config: *mut c_void, file: *const u8) -> c_int;
    }

    #[link(name = "pangocairo-1.0")]
    extern "C" {
        pub fn pango_cairo_font_map_get_default() -> *mut gtk::pango::ffi::PangoFontMap;
    }

    #[link(name = "pangoft2-1.0")]
    extern "C" {
        pub fn pango_fc_font_map_get_type() -> gtk::glib::ffi::GType;
        pub fn pango_fc_font_map_config_changed(map: *mut c_void);
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use gtk::glib::translate::{from_glib_none, ToGlibPtr};
    use sha2::{Digest, Sha256};
    use std::ffi::{c_char, CStr};
    use std::os::unix::fs::FileExt;

    #[link(name = "pangoft2-1.0")]
    extern "C" {
        fn pango_fc_font_get_pattern(font: *mut gtk::pango::ffi::PangoFont) -> *mut c_void;
    }
    #[link(name = "fontconfig")]
    extern "C" {
        fn FcPatternGetString(
            pattern: *mut c_void,
            object: *const c_char,
            index: c_int,
            value: *mut *mut u8,
        ) -> c_int;
        fn FcPatternGetInteger(
            pattern: *mut c_void,
            object: *const c_char,
            index: c_int,
            value: *mut c_int,
        ) -> c_int;
    }

    #[test]
    fn font_bytes_are_pinned_and_memfds_are_sealed_and_reused() {
        let hashes = [
            "a0bf60ef0f83c5ed4d7a75d45838548b1f6873372dfac88f71804491898d138f",
            "31c92d01a8a08528b718a43addf0ad3df0af2ca4b7b3290a452f70f358e14d3d",
            "1b3bfa1ed5665a4ce3f9feb68d2d4e40e70bf8b4b7d9a3edd418f321b4e166a0",
        ];
        let assets = load_fonts().unwrap();
        for ((bytes, expected), file) in FONTS.iter().zip(hashes).zip(&assets._registration._files)
        {
            assert_eq!(format!("{:x}", Sha256::digest(bytes)), expected);
            assert_eq!(file.metadata().unwrap().len(), bytes.len() as u64);
            let mut actual = vec![0; bytes.len()];
            file.read_exact_at(&mut actual, 0).unwrap();
            assert_eq!(&actual, bytes);
            // SAFETY: read-only descriptor queries on the retained live file.
            assert_eq!(
                unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GET_SEALS) },
                SEALS
            );
            assert_ne!(
                unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC,
                0
            );
            assert_eq!(
                file.write_at(b"changed", 0).unwrap_err().raw_os_error(),
                Some(libc::EPERM)
            );
            assert_eq!(
                file.set_len(0).unwrap_err().raw_os_error(),
                Some(libc::EPERM)
            );
        }
        let registration = assets._registration as *const Registration;
        drop(assets);
        assert_eq!(
            load_fonts().unwrap()._registration as *const Registration,
            registration
        );
    }

    #[test]
    fn pango_resolves_real_400_500_600_faces_from_memory_not_installed_fonts() {
        let assets = load_fonts().unwrap();
        // SAFETY: Pango owns this live caller-thread default map; take a reference.
        let map: gtk::pango::FontMap =
            unsafe { from_glib_none(native::pango_cairo_font_map_get_default()) };
        let context = map.create_context();
        for ((weight, fc_weight), file) in [
            (gtk::pango::Weight::Normal, 80),
            (gtk::pango::Weight::Medium, 100),
            (gtk::pango::Weight::Semibold, 180),
        ]
        .into_iter()
        .zip(&assets._registration._files)
        {
            let mut description = gtk::pango::FontDescription::new();
            description.set_family(FONT_FAMILY);
            description.set_weight(weight);
            description.set_size(12 * gtk::pango::SCALE);
            let font = context
                .load_font(&description)
                .expect("embedded face resolves");
            assert_eq!(font.describe().family().as_deref(), Some(FONT_FAMILY));
            assert_eq!(font.describe().weight(), weight);
            // Inspect the actual native pattern, not just the requested Pango
            // description: installed fonts/synthetic weights must not pass.
            unsafe {
                let pattern = pango_fc_font_get_pattern(font.to_glib_none().0);
                assert!(!pattern.is_null());
                let mut path = std::ptr::null_mut();
                assert_eq!(
                    FcPatternGetString(pattern, c"file".as_ptr(), 0, &mut path),
                    0
                );
                assert!(!path.is_null());
                assert_eq!(
                    CStr::from_ptr(path.cast()).to_str().unwrap(),
                    format!("/proc/self/fd/{}", file.as_raw_fd())
                );
                let mut actual_weight = -1;
                assert_eq!(
                    FcPatternGetInteger(pattern, c"weight".as_ptr(), 0, &mut actual_weight),
                    0
                );
                assert_eq!(actual_weight, fc_weight);
            }
        }
    }

    #[test]
    fn png_assets_are_pinned_nonempty_and_24px_without_display_or_svg_loader() {
        for (name, icon, hash) in [
            (
                "archive",
                Icon::Archive,
                "e4c2371d6d98f6e40832c9614c7cfdfa7511447c3a1a8ca572d0e27342daf705",
            ),
            (
                "trash-2",
                Icon::Trash,
                "b9c1e094fffc7541b6a218cc311a75f56f9b518397aa1a84285dcd253c79e881",
            ),
            (
                "check",
                Icon::Check,
                "b4a43b74f7ed1ca223375bb8b9d38c9b005f74ff384ce2b24f7622dc8e6d7050",
            ),
            (
                "x",
                Icon::X,
                "c6743c71813935b3ff51229d70a9e3429e44d595d324d1216d00e52f028e1319",
            ),
        ] {
            assert_eq!(Icon::from_name(name), Some(icon));
            assert_eq!(format!("{:x}", Sha256::digest(icon.png())), hash);
            let pixbuf = icon_pixbuf(icon).unwrap();
            assert!(pixbuf.has_alpha());
            assert_eq!(pixbuf.n_channels(), 4);
            assert_eq!(pixbuf.bits_per_sample(), 8);
            let bytes = pixbuf.read_pixel_bytes();
            let visible = bytes
                .as_ref()
                .chunks_exact(4)
                .filter(|pixel| pixel[3] > 0)
                .count();
            assert!(
                visible > 0 && visible < 24 * 24,
                "must contain both strokes and transparency"
            );
        }
        for unknown in ["", "download", "trash", "../x", "x.svg", "X"] {
            assert_eq!(Icon::from_name(unknown), None);
        }
    }

    /// Parent integration hook: call from the existing sole GTK smoke test,
    /// not a second `#[test]` (GTK must remain on its initialization thread).
    pub fn assert_gtk_images() {
        assert!(gtk::is_initialized_main_thread());
        let _fonts = load_fonts().unwrap();
        for icon in [Icon::Archive, Icon::Trash, Icon::Check, Icon::X] {
            let image = icon_image(icon);
            let pixbuf = image
                .pixbuf()
                .expect("real PNG image, not an icon theme lookup");
            assert_eq!((pixbuf.width(), pixbuf.height()), (ICON_SIZE, ICON_SIZE));
            assert!(!image.can_focus());
        }
    }
}
