#!/usr/bin/env python3
"""Builder-only candidate preparation; no publication, installation or approval."""

import argparse
import contextlib
import hashlib
import io
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import types
from pathlib import Path, PurePosixPath

VERSION = r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)"
TARGET = "Ubuntu 24.04 LTS x86_64"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def load_package():
    """Load the adjacent builder, including its local runtime dependency, not PATH."""
    directory = Path(__file__).resolve().parents[1] / "dagger"
    modules = {}
    previous = sys.modules.get("runtime")
    try:
        for name in ("runtime", "package"):
            path = directory / (name + ".py")
            module = types.ModuleType("smith_release_" + name)
            module.__file__ = str(path)
            exec(compile(path.read_bytes(), str(path), "exec"), module.__dict__)
            modules[name] = module
            if name == "runtime":
                sys.modules["runtime"] = module
    finally:
        if previous is None:
            sys.modules.pop("runtime", None)
        else:
            sys.modules["runtime"] = previous
    return modules["package"]


def git(repo, *args):
    env = dict(os.environ, GIT_OPTIONAL_LOCKS="0", GIT_NO_REPLACE_OBJECTS="1",
               GIT_TERMINAL_PROMPT="0")
    # Ambient Git routing must not redirect checks away from --repo.
    for name in tuple(env):
        if name.startswith("GIT_") and name not in {
            "GIT_OPTIONAL_LOCKS", "GIT_NO_REPLACE_OBJECTS", "GIT_TERMINAL_PROMPT"
        }:
            del env[name]
    result = subprocess.run(["git", "--no-pager", "-C", str(repo), *args],
                            env=env, capture_output=True, text=True, timeout=30)
    require(result.returncode == 0, "Git source validation failed: " + " ".join(args))
    return result.stdout.strip()


def source_identity(repo, tag, commit):
    require(re.fullmatch("release/v" + VERSION, tag),
            "Expected canonical stable tag release/vMAJOR.MINOR.PATCH")
    require(re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", commit), "Expected full commit SHA")
    require(Path(git(repo, "rev-parse", "--show-toplevel")).resolve() == repo.resolve(),
            "--repo must be the repository root")
    ref = "refs/tags/" + tag
    require(git(repo, "cat-file", "-t", ref) == "tag", "Release tag must be annotated")
    require(git(repo, "rev-parse", ref + "^{commit}") == commit
            == git(repo, "rev-parse", "HEAD"), "Tag, expected commit and HEAD must match")
    git(repo, "merge-base", "--is-ancestor", commit, "refs/remotes/origin/main")
    require(not git(repo, "status", "--porcelain=v1", "--untracked-files=no",
                    "--ignore-submodules=none"), "Tracked source must be clean")
    version = tag.removeprefix("release/v")
    components = {}
    for component in ("installer", "service"):
        with (repo / component / "Cargo.toml").open("rb") as handle:
            package = tomllib.load(handle)["package"]
        require(package["name"] == "smith-" + component and package["version"] == version,
                component + " Cargo version does not match tag")
        components[component] = package["version"]
    for component, names in (("installer", ("installer", "service")),
                             ("service", ("service",))):
        with (repo / component / "Cargo.lock").open("rb") as handle:
            packages = tomllib.load(handle)["package"]
        for name in names:
            matches = [p for p in packages if p["name"] == "smith-" + name]
            require(len(matches) == 1 and matches[0]["version"] == components[name]
                    and "source" not in matches[0], "Cargo lock component version mismatch")
    pubspec = (repo / "client/pubspec.yaml").read_text(encoding="utf-8")
    lines = [line for line in pubspec.splitlines() if re.match(r"^version\s*:", line)]
    require(len(lines) == 1 and re.fullmatch(
        r"version: (" + VERSION + r"(?:\+[0-9]+)?)[ \t]*", lines[0]),
        "Expected one standalone client version line (optional numeric +build)")
    components["client"] = lines[0].split(": ", 1)[1].strip()
    require(components["client"].split("+", 1)[0] == version, "Client version does not match tag")
    return {"source_sha": commit, "tag": tag,
            "tag_object_sha": git(repo, "rev-parse", ref),
            "git_tree": git(repo, "rev-parse", commit + "^{tree}"),
            "version": version, "components": components}


def snapshot(root, package):
    info = root.lstat()
    require(stat.S_ISDIR(info.st_mode) and not info.st_mode & 0o7022,
            "Unsafe package root")
    entries, _ = package.tree(root)
    return stat.S_IMODE(info.st_mode), entries


def create_archive(root, archive, stem, package):
    mode, entries = snapshot(root, package)
    with tarfile.open(archive, "w:gz", format=tarfile.PAX_FORMAT) as handle:
        for name, entry in [("", {"type": "directory", "mode": mode}), *entries.items()]:
            info = tarfile.TarInfo(stem + ("/" + name if name else ""))
            info.mode = entry["mode"]
            if entry["type"] == "directory":
                info.type = tarfile.DIRTYPE
                handle.addfile(info)
            else:
                data = package.file_bytes(root / name)
                require(hashlib.sha256(data).hexdigest() == entry["sha256"],
                        "Package changed during archiving")
                info.size = len(data)
                handle.addfile(info, io.BytesIO(data))
    require(snapshot(root, package) == (mode, entries), "Package changed during archiving")
    return mode, entries


def extract_archive(archive, destination, stem):
    """Validate the entire member table before manually extracting into a new root."""
    require(not os.path.lexists(destination), "Extraction destination must be absent")
    with tarfile.open(archive, "r:gz") as handle:
        members = {}
        total = 0
        for member in handle:
            name = member.name
            parts = PurePosixPath(name).parts
            require(parts and parts[0] == stem and name == "/".join(parts)
                    and all(p not in (".", "..") for p in parts)
                    and "\\" not in name and not any(ord(c) < 32 or ord(c) == 127 for c in name),
                    "Unsafe archive path")
            require(name not in members, "Duplicate archive member")
            require(member.type in (tarfile.REGTYPE, tarfile.DIRTYPE)
                    and not member.linkname and not member.mode & ~0o777
                    and not member.mode & 0o022, "Unsafe archive type/mode")
            require(0 <= member.size <= 128 * 1024 * 1024
                    and (member.isfile() or member.size == 0), "Unsafe archive size")
            total += member.size
            require(total <= 512 * 1024 * 1024 and len(members) < 10000,
                    "Archive exceeds package bounds")
            members[name] = member
        require(stem in members and members[stem].isdir(), "Missing archive root")
        for name in members:
            if name != stem:
                parent = str(PurePosixPath(name).parent)
                require(parent in members and members[parent].isdir(), "Missing archive parent")
        destination.mkdir(mode=0o700)
        for name, member in sorted(members.items(), key=lambda item: (item[0].count("/"), item[0])):
            path = destination / name
            if member.isdir():
                path.mkdir(mode=0o700)
            else:
                with handle.extractfile(member) as source, path.open("xb") as target:
                    shutil.copyfileobj(source, target)
                path.chmod(member.mode)
        for name, member in sorted(members.items(), reverse=True):
            if member.isdir():
                (destination / name).chmod(member.mode)
    return destination / stem


def digest(path):
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


def prepare(repo, tag, commit, root, output, run_url=None):
    identity = source_identity(repo, tag, commit)
    require(not os.path.lexists(output), "Output path must be new and absent")
    require(output.parent.is_dir(), "Output parent must exist")
    require(not output.resolve().is_relative_to(root.resolve()), "Output cannot be inside package")
    package = load_package()
    with contextlib.redirect_stdout(io.StringIO()):
        package.verify(root)
    stem = "smith-installer_v" + identity["version"]
    with tempfile.TemporaryDirectory(prefix="smith-release-") as temporary:
        staging = Path(temporary)
        archive = staging / (stem + ".tar.gz")
        expected = create_archive(root, archive, stem, package)
        extracted = extract_archive(archive, staging / "extracted", stem)
        require(snapshot(extracted, package) == expected, "Archive content/mode round trip mismatch")
        with contextlib.redirect_stdout(io.StringIO()):
            package.verify(extracted)
        # Recheck the source binding before accepting the candidate.
        require(source_identity(repo, tag, commit) == identity, "Source identity changed")
        archive_hash = digest(archive)
        record = {"schema_version": 1, **identity, "target": TARGET, "run_url": run_url,
                  "archive": archive.name, "archive_sha256": archive_hash,
                  "package_manifest_sha256": digest(extracted / "manifest.json"),
                  "self_check": {"source_identity": "passed", "package_verification": "passed",
                                 "archive_roundtrip": "passed", "native_execution": "not_run",
                                 "hermetic_build": "not_established", "release_approval": "not_granted",
                                 "package_source_binding": "supplied_package_not_proven_by_this_helper"}}
        checksum = staging / (archive.name + ".sha256")
        checksum.write_text(archive_hash + "  " + archive.name + "\n", encoding="utf-8")
        build_record = staging / "build-record.json"
        build_record.write_text(json.dumps(record, sort_keys=True, indent=2) + "\n", encoding="utf-8")
        output.mkdir(mode=0o700)
        for path in (archive, checksum, build_record):
            with path.open("rb") as source, (output / path.name).open("xb") as target:
                shutil.copyfileobj(source, target)
            (output / path.name).chmod(0o644)
    return stem


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--tag", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--check-only", action="store_true")
    parser.add_argument("--package", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--run-url")
    args = parser.parse_args(argv)
    if not args.check_only and (args.package is None or args.output is None):
        parser.error("--package and --output are required unless --check-only")
    try:
        if args.check_only:
            identity = source_identity(args.repo, args.tag, args.commit)
            stem = "smith-installer_v" + identity["version"]
        else:
            stem = prepare(args.repo, args.tag, args.commit, args.package, args.output, args.run_url)
    except (ValueError, OSError, KeyError, TypeError, tarfile.TarError, subprocess.SubprocessError) as error:
        parser.exit(1, "Candidate preparation refused: " + str(error) + "\n")
    print(stem)


if __name__ == "__main__":
    main()
