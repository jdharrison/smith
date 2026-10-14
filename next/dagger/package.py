"""Builder-only native package metadata. No host installer or provider behavior."""

import argparse
import hashlib
import json
import os
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
    "data/flutter_assets/shaders/ink_sparkle.frag",
    "data/flutter_assets/shaders/stretch_effect.frag",
    "share/applications/" + APP + ".desktop", "share/pixmaps/" + APP + ".png",
}
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
    for name in ("smith_client", "smith_backend/smith-service"):
        require(entries.get(name, {}).get("mode") == 0o755 and contents.get(name),
                "Missing executable in complete client payload")
        check_native(root / "payload/client" / name)
    for name in ("lib/libflutter_linux_gtk.so", "lib/libapp.so", "data/icudtl.dat",
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
    root = root.absolute()
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
    root = root.absolute()
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
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--readme", type=Path)
    parser.add_argument("--verify", action="store_true")
    args = parser.parse_args()
    if args.verify:
        verify(args.root)
    else:
        if args.readme is None:
            parser.error("--readme is required for assembly")
        build(args.root, args.readme)


if __name__ == "__main__":
    main()
