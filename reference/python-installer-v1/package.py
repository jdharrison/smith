"""Assemble/verify an explicit recipient payload; never install or contact providers."""

import argparse
import hashlib
import json

import stat
import zipapp
from pathlib import Path

import install


# This dependency-free Flutter slice exports only these reviewed runtime assets.
# New plugins/assets require an explicit packaging-contract review, not an
# unchecked copy of arbitrary model, voice or reference directories.
CLIENT_FILES = {
    "smith_client", "smith_backend/smith-service", "lib/libapp.so",
    "lib/libflutter_linux_gtk.so", "data/icudtl.dat",
    "data/flutter_assets/AssetManifest.bin", "data/flutter_assets/FontManifest.json",
    "data/flutter_assets/NativeAssetsManifest.json", "data/flutter_assets/NOTICES.Z",
    "data/flutter_assets/version.json", "data/flutter_assets/fonts/MaterialIcons-Regular.otf",
    "data/flutter_assets/shaders/ink_sparkle.frag",
    "data/flutter_assets/shaders/stretch_effect.frag",
    "share/applications/" + install.APP + ".desktop",
    "share/pixmaps/" + install.APP + ".png",
}
CLIENT_DIRS = {str(parent) for name in CLIENT_FILES for parent in Path(name).parents
               if str(parent) != "."}


def inventory(root):
    entries = {}
    for path in sorted(root.rglob("*")):
        info = path.lstat()
        relative = path.relative_to(root).as_posix()
        install.require(not path.is_symlink(), "Package symlinks are not supported")
        install.require(not info.st_mode & 0o7022, "Unsafe package mode")
        if path.is_dir():
            continue
        install.require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1,
                        "Package files must be regular and single-link")
        if relative in ("manifest.json", "SHA256SUMS"):
            continue
        install.require(info.st_size <= install.MAX_FILE, "Package file exceeds limit")
        entries[relative] = {
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "mode": stat.S_IMODE(info.st_mode),
        }
    return entries


def check_payload(root):
    install.require(set(p.name for p in root.iterdir()) <= {
        "payload", "smith-installer", "README.txt", "manifest.json", "SHA256SUMS"
    }, "Unexpected recipient package input")
    install.require(set(p.name for p in (root / "payload").iterdir()) == {"client", "service"}
                    and set(p.name for p in (root / "payload/service").iterdir()) == {"smith-service"},
                    "Only paired native payloads may be packaged")
    with install.Filesystem() as fs:
        entries, contents = fs.tree(str(root / "payload/client"))
        install.require(set(contents) <= CLIENT_FILES and all(
            name in CLIENT_FILES if entry["type"] == "file" else name in CLIENT_DIRS
            for name, entry in entries.items()
        ), "Unreviewed client payload input; model/voice/reference assets are excluded")
        for name in ("smith_client", "smith_backend/smith-service"):
            install.require(entries.get(name, {}).get("mode") == 0o755 and contents.get(name),
                            "Missing executable in complete client payload")
        for name in ("lib/libflutter_linux_gtk.so", "lib/libapp.so", "data/icudtl.dat",
                     "share/applications/" + install.APP + ".desktop",
                     "share/pixmaps/" + install.APP + ".png"):
            install.require(contents.get(name), "Incomplete client payload")
        for name in ("lib", "data", "data/flutter_assets"):
            install.require(entries.get(name, {}).get("type") == "directory",
                            "Missing Flutter payload directory")
        service = fs.file(str(root / "payload/service/smith-service"), 0o755)
        install.require(service and service[0] == contents["smith_backend/smith-service"],
                        "Standalone/bundled service mismatch")
    return install.digest(install.manifest_bytes(entries)), install.digest(service[0])


def write_new(path, content):
    with path.open("x", encoding="utf-8") as handle:
        handle.write(content)
    path.chmod(0o644)


def build(code, root, readme):
    code, root = code.absolute(), root.absolute()
    install.require(set(p.name for p in code.iterdir()) == {"install.py", "installer.py"},
                    "Only explicit installer code may enter the zipapp")
    entry = code / "__main__.py"
    write_new(entry, "import installer\nraise SystemExit(installer.main())\n")
    executable = root / "smith-installer"
    install.require(not executable.exists(), "Installer output already exists")
    zipapp.create_archive(code, executable, interpreter="/usr/bin/env python3", compressed=True)
    executable.chmod(0o755)
    write_new(root / "README.txt", readme.read_text())
    client_digest, helper_digest = check_payload(root)
    files = inventory(root)
    record = {
        "schema_version": 1,
        "product": "smith-installer",
        "target": "Ubuntu x86_64; Python >=3.12",
        "models_bundled": False,
        "voice_bundled": False,
        "exclusion_policy": "reviewed-text-payload-paths-v1",
        "client_manifest_sha256": client_digest,
        "service_sha256": helper_digest,
        "files": files,
    }
    write_new(root / "manifest.json", json.dumps(record, sort_keys=True, indent=2) + "\n")
    write_new(root / "SHA256SUMS",
        "".join(f"{meta['sha256']}  {name}\n" for name, meta in sorted(files.items()))
        + hashlib.sha256((root / "manifest.json").read_bytes()).hexdigest() + "  manifest.json\n")
    verify(root)


def verify(root):
    root = root.absolute()
    record = json.loads((root / "manifest.json").read_text())
    install.require(record["schema_version"] == 1 and record["product"] == "smith-installer"
                    and record["models_bundled"] is False and record["voice_bundled"] is False
                    and record["exclusion_policy"] == "reviewed-text-payload-paths-v1",
                    "Unsupported recipient package")
    install.require(record["files"] == inventory(root), "Package content/mode manifest mismatch")
    client_digest, helper_digest = check_payload(root)
    install.require(record["client_manifest_sha256"] == client_digest
                    and record["service_sha256"] == helper_digest, "Package payload mismatch")
    expected = "".join(f"{meta['sha256']}  {name}\n" for name, meta in sorted(record["files"].items()))
    expected += hashlib.sha256((root / "manifest.json").read_bytes()).hexdigest() + "  manifest.json\n"
    install.require((root / "SHA256SUMS").read_text() == expected, "Package checksum list mismatch")
    print(f"Recipient package verified: {len(record['files'])} files; reviewed text-only payload paths")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--code", type=Path)
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--readme", type=Path)
    parser.add_argument("--verify", action="store_true")
    args = parser.parse_args()
    if args.verify:
        verify(args.root)
    else:
        if args.code is None or args.readme is None:
            parser.error("--code and --readme are required for assembly")
        build(args.code, args.root, args.readme)


if __name__ == "__main__":
    main()
