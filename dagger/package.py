"""Builder-only native package metadata. No host installer or provider behavior."""

import argparse
import hashlib
import json
import os
import re
import shlex
import stat
from pathlib import Path

import runtime

APP = "dev.smith.smith_client"
MAX_FILE = 128 * 1024 * 1024
POLICY = "reviewed-text-payload-paths-v1"
CLIENT_FILES = {
    "smith_client", "smith_backend/smith-service", "lib/libapp.so",
    "lib/libflutter_linux_gtk.so", "data/icudtl.dat",
    "data/flutter_assets/AssetManifest.bin", "data/flutter_assets/FontManifest.json",
    "data/flutter_assets/NativeAssetsManifest.json", "data/flutter_assets/NOTICES.Z",
    "data/flutter_assets/version.json", "data/flutter_assets/fonts/MaterialIcons-Regular.otf",
    "data/flutter_assets/pubspec.yaml",
    "data/flutter_assets/shaders/ink_sparkle.frag",
    "data/flutter_assets/shaders/stretch_effect.frag",
    "share/applications/" + APP + ".desktop", "share/pixmaps/" + APP + ".png",
}
# Exact reviewed UI inputs only: directory membership never authorizes new assets.
UI_ASSET_FILES = {
    "data/flutter_assets/assets/README.md",
    "data/flutter_assets/assets/fonts/jetbrains-mono/JetBrainsMono-Regular.ttf",
    "data/flutter_assets/assets/fonts/jetbrains-mono/JetBrainsMono-Medium.ttf",
    "data/flutter_assets/assets/fonts/jetbrains-mono/JetBrainsMono-SemiBold.ttf",
    "data/flutter_assets/assets/fonts/jetbrains-mono/OFL.txt",
    "data/flutter_assets/assets/fonts/ibm-plex-mono/IBMPlexMono-Regular.ttf",
    "data/flutter_assets/assets/fonts/ibm-plex-mono/IBMPlexMono-Medium.ttf",
    "data/flutter_assets/assets/fonts/ibm-plex-mono/IBMPlexMono-SemiBold.ttf",
    "data/flutter_assets/assets/fonts/ibm-plex-mono/OFL.txt",
    "data/flutter_assets/assets/icons/chevron-right.svg",
    "data/flutter_assets/assets/icons/message-square.svg",
    "data/flutter_assets/assets/icons/settings.svg",
    "data/flutter_assets/assets/icons/archive.svg",
    "data/flutter_assets/assets/icons/plus.svg",
    "data/flutter_assets/assets/icons/ellipsis-vertical.svg",
    "data/flutter_assets/assets/icons/pencil.svg",
    "data/flutter_assets/assets/icons/trash-2.svg",
    "data/flutter_assets/assets/icons/send.svg",
    "data/flutter_assets/assets/icons/x.svg",
    "data/flutter_assets/assets/icons/menu.svg",
    "data/flutter_assets/assets/icons/rotate-ccw.svg",
    "data/flutter_assets/assets/icons/info.svg",
    "data/flutter_assets/assets/icons/check.svg",
    "data/flutter_assets/assets/icons/chevron-down.svg",
    "data/flutter_assets/assets/icons/layers.svg",
    "data/flutter_assets/assets/icons/git-branch.svg",
}
CLIENT_FILES |= UI_ASSET_FILES
COMMON_RUNTIME_FILES = set(runtime.FILES) | {
    "libgomp.so.1", "LICENSE.llama.cpp", "LICENSE.libgomp", "LICENSE.GPL-3"
}
BACKEND_RUNTIME_FILES = {
    "cpu": COMMON_RUNTIME_FILES,
    "vulkan": COMMON_RUNTIME_FILES | {"libggml-vulkan.so"},
}
RUNTIME_FILES = {
    backend + "/" + name
    for backend, names in BACKEND_RUNTIME_FILES.items()
    for name in names
}
CLIENT_FILES |= {"smith_backend/runtime/" + name for name in RUNTIME_FILES}
CLIENT_DIRS = {str(parent) for name in CLIENT_FILES for parent in Path(name).parents
               if str(parent) != "."}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def check_location(root, *, create=False):
    """Gate exports without following links; preparation may create one output leaf."""
    spelling = os.fspath(root)
    require(isinstance(spelling, str)
            and re.fullmatch(r"/?[A-Za-z0-9_./-]+", spelling)
            and all(part and part not in (".", "..")
                    for part in spelling.removeprefix("/").split("/")),
            "Build location requires normalized spelling without empty, dot or parent components")
    root = Path(spelling).absolute()
    require(re.fullmatch(r"/[A-Za-z0-9_./-]+", str(root))
            and all(part not in (".", "..") for part in root.parts[1:]),
            "Build location requires a restricted absolute path without parent components")
    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
    fd = os.open("/", flags)
    current = Path("/")
    try:
        for index, part in enumerate(root.parts[1:]):
            current /= part
            final = index == len(root.parts) - 2
            created = False
            try:
                child = os.open(part, flags, dir_fd=fd)
            except FileNotFoundError as error:
                require(final and create,
                        f"Build location blocked: missing directory {current}; "
                        "only the final generated output directory may be created")
                try:
                    os.mkdir(part, mode=0o700, dir_fd=fd)
                    created = True
                except FileExistsError:
                    pass  # A competing creator must pass the same no-follow checks.
                try:
                    child = os.open(part, flags, dir_fd=fd)
                except OSError as error:
                    raise ValueError(f"Build location blocked: {current}; "
                                     "expected a safe non-symlink directory") from error
            except OSError as error:
                raise ValueError(f"Build location blocked: {current}; "
                                 "expected an accessible non-symlink directory") from error
            os.close(fd)
            fd = child
            info = os.fstat(fd)
            detail = (f"Build location blocked: {current} "
                      f"(mode={stat.S_IMODE(info.st_mode):04o}, uid={info.st_uid}). ")
            require(info.st_uid == os.geteuid() or (not final and info.st_uid == 0),
                    detail + "Expected current-user ownership (or root for ancestors); "
                    "choose a safe output location. Existing permissions were not changed.")
            sticky_ancestor = not final and info.st_uid == 0 and info.st_mode & stat.S_ISVTX
            if info.st_mode & 0o022 and not sticky_ancestor:
                remedy = ("After reviewing shared access, remove unintended write access "
                          f"on this directory only: chmod go-w -- {shlex.quote(str(current))}. "
                          if info.st_uid == os.geteuid() else "")
                raise ValueError(detail + "Group/world-writable directories are unsafe. "
                                 + remedy + "Or choose a safe output location. "
                                 "Existing permissions were not changed; no automatic repair or retry.")
            require(not created or stat.S_IMODE(info.st_mode) == 0o700,
                    detail + "New generated output must remain private (0700); "
                    "no automatic permission repair.")
    finally:
        os.close(fd)
    return root


def identity(info):
    return (info.st_dev, info.st_ino, info.st_uid, info.st_mode, info.st_nlink,
            info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def file_bytes(path):
    before = path.lstat()
    require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
            and not before.st_mode & 0o7022 and before.st_size <= MAX_FILE,
            "Unsafe or oversized package file")
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, "rb") as handle:
        require(handle.fileno() >= 0 and identity(os.fstat(handle.fileno())) == identity(before),
                "Package file changed before read")
        data = handle.read(MAX_FILE + 1)
        require(len(data) <= MAX_FILE and identity(os.fstat(handle.fileno())) == identity(before)
                and identity(path.lstat()) == identity(before), "Package file changed during read")
    return data


def check_native(path):
    data = file_bytes(path)
    require(stat.S_IMODE(path.lstat().st_mode) == 0o755 and len(data) >= 64
            and data[:7] == b"\x7fELF\x02\x01\x01"
            and int.from_bytes(data[16:18], "little") in (2, 3)
            and int.from_bytes(data[18:20], "little") == 62,
            "Expected native Linux x86_64 ELF, not an interpreter entrypoint")
    return data


def tree(root):
    require(root.is_dir() and not root.is_symlink(), "Missing native payload directory")
    entries, contents = {}, {}
    total = 0
    for path in sorted(root.rglob("*")):
        name = path.relative_to(root).as_posix()
        info = path.lstat()
        require(not path.is_symlink() and not info.st_mode & 0o7022, "Unsafe package input")
        mode = stat.S_IMODE(info.st_mode)
        if stat.S_ISDIR(info.st_mode):
            entries[name] = {"type": "directory", "mode": mode}
        else:
            data = file_bytes(path)
            total += len(data)
            require(total <= 512 * 1024 * 1024 and len(entries) < 10000, "Payload exceeds bounds")
            entries[name] = {"type": "file", "mode": mode,
                             "sha256": hashlib.sha256(data).hexdigest()}
            contents[name] = data
    return entries, contents


def inventory(root):
    entries = {}
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root).as_posix()
        info = path.lstat()
        require(not path.is_symlink() and not info.st_mode & 0o7022, "Unsafe package mode/link")
        if stat.S_ISDIR(info.st_mode):
            continue
        data = file_bytes(path)
        if relative not in ("manifest.json", "SHA256SUMS"):
            entries[relative] = {"sha256": hashlib.sha256(data).hexdigest(),
                                 "mode": stat.S_IMODE(info.st_mode)}
    return entries


def check_payload(root):
    require({p.name for p in root.iterdir()} <= {
        "payload", "smith-installer", "smith-installer-ui", "README.txt", "manifest.json", "SHA256SUMS"
    }, "Unexpected recipient package input")
    require({p.name for p in (root / "payload").iterdir()} == {"client", "service"}
            and {p.name for p in (root / "payload/service").iterdir()} == {"smith-service"},
            "Only paired native payloads may be packaged")
    check_native(root / "smith-installer")
    check_native(root / "smith-installer-ui")
    entries, contents = tree(root / "payload/client")
    require(set(contents) <= CLIENT_FILES and all(
        name in CLIENT_FILES if entry["type"] == "file" else name in CLIENT_DIRS
        for name, entry in entries.items()
    ), "Unreviewed client payload input; model/voice/reference assets are excluded")
    for name in sorted(UI_ASSET_FILES):
        require(contents.get(name), "Missing or empty required UI asset: " + name)
    for name in ("smith_client", "smith_backend/smith-service"):
        require(entries.get(name, {}).get("mode") == 0o755 and contents.get(name),
                "Missing executable in complete client payload")
        check_native(root / "payload/client" / name)
    for name in ("lib/libflutter_linux_gtk.so", "lib/libapp.so", "data/icudtl.dat",
                 "data/flutter_assets/pubspec.yaml",
                 "share/applications/" + APP + ".desktop", "share/pixmaps/" + APP + ".png"):
        require(contents.get(name), "Incomplete client payload")
    for name in ("lib", "data", "data/flutter_assets"):
        require(entries.get(name, {}).get("type") == "directory", "Missing Flutter payload directory")
    for backend, expected in BACKEND_RUNTIME_FILES.items():
        prefix = "smith_backend/runtime/" + backend + "/"
        actual = {name.removeprefix(prefix) for name in contents if name.startswith(prefix)}
        require(entries.get("smith_backend/runtime/" + backend, {}).get("type") == "directory"
                and actual == expected, "Incomplete or mixed native runtime payload")
        check_native(root / "payload/client" / prefix / "llama-server")
    service = check_native(root / "payload/service/smith-service")
    require(service == contents["smith_backend/smith-service"], "Standalone/bundled service mismatch")
    encoded = json.dumps({"schema": 1, "entries": entries}, sort_keys=True,
                         separators=(",", ":")).encode() + b"\n"
    return hashlib.sha256(encoded).hexdigest(), hashlib.sha256(service).hexdigest()


def write_new(path, content):
    with path.open("x", encoding="utf-8") as handle:
        handle.write(content)
    path.chmod(0o644)


def build(root, readme):
    root = check_location(root)
    # Native executable and paired outputs are supplied explicitly by Dagger.
    write_new(root / "README.txt", readme.read_text())
    client_digest, helper_digest = check_payload(root)
    record = {"schema_version": 1, "product": "smith-installer",
              "target": "Ubuntu 24.04 x86_64; native Rust ELF",
              "installer_runtime": "native", "installer_ui": "gtk3",
              "model_runner": "llama.cpp-b11429-cpu+vulkan",
              "models_bundled": False, "voice_bundled": False,
              "exclusion_policy": POLICY, "client_manifest_sha256": client_digest,
              "service_sha256": helper_digest, "files": inventory(root)}
    write_new(root / "manifest.json", json.dumps(record, sort_keys=True, indent=2) + "\n")
    write_new(root / "SHA256SUMS", checksum_text(root, record["files"]))
    verify(root)


def checksum_text(root, files):
    return ("".join(f"{meta['sha256']}  {name}\n" for name, meta in sorted(files.items()))
            + hashlib.sha256(file_bytes(root / "manifest.json")).hexdigest() + "  manifest.json\n")


def verify(root):
    root = check_location(root)
    record = json.loads(file_bytes(root / "manifest.json"))
    require(record["schema_version"] == 1 and record["product"] == "smith-installer"
            and record["installer_runtime"] == "native" and record["installer_ui"] == "gtk3"
            and record["model_runner"] == "llama.cpp-b11429-cpu+vulkan"
            and record["models_bundled"] is False and record["voice_bundled"] is False
            and record["exclusion_policy"] == POLICY, "Unsupported native recipient package")
    require(record["files"] == inventory(root), "Package content/mode manifest mismatch")
    client_digest, helper_digest = check_payload(root)
    require(record["client_manifest_sha256"] == client_digest
            and record["service_sha256"] == helper_digest, "Package payload mismatch")
    require(file_bytes(root / "SHA256SUMS").decode() == checksum_text(root, record["files"]),
            "Package checksum list mismatch")
    print(f"Native recipient package verified: {len(record['files'])} files; reviewed text-only payload paths")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True)
    parser.add_argument("--readme", type=Path)
    action = parser.add_mutually_exclusive_group()
    action.add_argument("--verify", action="store_true")
    action.add_argument("--check-location", action="store_true",
                        help="read-only ancestor and output-directory permission check")
    action.add_argument("--prepare-location", action="store_true",
                        help="check ancestors and create only an absent output leaf as 0700")
    args = parser.parse_args()
    try:
        if args.check_location or args.prepare_location:
            root = check_location(args.root, create=args.prepare_location)
            print(f"Build output location checked: {root}; not installation approval")
        elif args.verify:
            verify(args.root)
        else:
            if args.readme is None:
                parser.error("--readme is required for assembly")
            build(args.root, args.readme)
    except (ValueError, OSError) as error:
        parser.exit(1, f"Package/build check failed: {error}\n")


if __name__ == "__main__":
    main()
