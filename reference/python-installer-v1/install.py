#!/usr/bin/env python3
"""Install prebuilt Smith next artifacts; never build, launch or manage services."""

import argparse
import contextlib
import ctypes
import fcntl
import hashlib
import json
import os
import re
import secrets
import stat
import sys
import time

MANIFEST = ".smith-next-manifest.json"
APP = "dev.smith.smith_client"
MAX_FILE = 128 * 1024 * 1024
MAX_TOTAL = 512 * 1024 * 1024
MAX_ENTRIES = 10000
LOCK_SECONDS = 5
DIR_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
FILE_FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC


class Unsafe(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise Unsafe(message)


def path_parts(path):
    require(isinstance(path, str) and re.fullmatch(r"/[A-Za-z0-9_./-]+", path),
            "Paths must be restricted absolute paths")
    parts = path.split("/")[1:]
    require(all(p and p not in (".", "..") for p in parts),
            "Paths must be normalized without empty, dot or parent components")
    return parts


def identity(s, directory=False):
    base = (s.st_dev, s.st_ino, s.st_uid, s.st_mode, s.st_nlink)
    return base if directory else base + (s.st_size, s.st_mtime_ns, s.st_ctime_ns)


def safe_dir(s, owned=False, private=False):
    sticky = s.st_uid == 0 and s.st_mode & stat.S_ISVTX
    require(stat.S_ISDIR(s.st_mode) and s.st_uid in (0, os.geteuid())
            and (not s.st_mode & 0o022 or sticky), "Unsafe directory ancestor")
    if owned:
        require(s.st_uid == os.geteuid() and not s.st_mode & 0o022,
                "Directory must be user-owned and not group/world writable")
    if private:
        require(stat.S_IMODE(s.st_mode) == 0o700, "Namespace must be exactly 0700")


class Filesystem:
    """All traversal and I/O is relative to verified, held directory descriptors."""

    def __init__(self, effects=None):
        self.stack = contextlib.ExitStack()
        self.trace = {}
        self.effects = effects if effects is not None else []

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.stack.close()

    def hold(self, fd):
        self.stack.callback(os.close, fd)
        return fd

    def directory(self, path, create=False, private=False):
        parts = path_parts(path)
        fd = os.open("/", DIR_FLAGS)
        current = ""
        try:
            for i, part in enumerate(parts):
                current += "/" + part
                try:
                    child = os.open(part, DIR_FLAGS, dir_fd=fd)
                except FileNotFoundError:
                    if not create:
                        self.trace[current] = None
                        return None
                    os.mkdir(part, 0o700, dir_fd=fd)
                    self.effects.append("created directory " + current)
                    os.fsync(fd)
                    child = os.open(part, DIR_FLAGS, dir_fd=fd)
                os.close(fd)
                fd = child
                s = os.fstat(fd)
                try:
                    safe_dir(s, owned=i == len(parts) - 1,
                             private=private and i == len(parts) - 1)
                except Unsafe as exc:
                    raise Unsafe(f"{exc}: {current} "
                                 f"(mode={stat.S_IMODE(s.st_mode):04o}, uid={s.st_uid})") from exc
                self.trace[current] = identity(s, True)
            result = self.hold(fd)
            fd = None
            return result
        finally:
            if fd is not None:
                os.close(fd)

    def parent(self, path, create=False):
        path_parts(path)
        parent, name = path.rsplit("/", 1)
        return self.directory(parent, create), name

    def read_at(self, fd, name, mode=None):
        f = os.open(name, FILE_FLAGS, dir_fd=fd)
        try:
            before = os.fstat(f)
            require(stat.S_ISREG(before.st_mode) and before.st_uid == os.geteuid()
                    and before.st_nlink == 1 and not before.st_mode & 0o7022
                    and before.st_size <= MAX_FILE, "Unsafe regular file: " + name)
            if mode is not None:
                require(stat.S_IMODE(before.st_mode) == mode,
                        "Incorrect file mode: " + name)
            chunks, size = [], 0
            while True:
                chunk = os.read(f, 1024 * 1024)
                if not chunk:
                    break
                size += len(chunk)
                require(size <= MAX_FILE, "File exceeds limit")
                chunks.append(chunk)
            require(identity(before) == identity(os.fstat(f)), "File changed while reading")
            require(identity(before) == identity(os.stat(name, dir_fd=fd,
                                                        follow_symlinks=False)),
                    "File replaced while reading")
            return b"".join(chunks), stat.S_IMODE(before.st_mode), identity(before)
        finally:
            os.close(f)

    def file(self, path, mode=None):
        fd, name = self.parent(path)
        if fd is None:
            self.trace[path] = None
            return None
        try:
            data, actual_mode, token = self.read_at(fd, name, mode)
        except FileNotFoundError:
            self.trace[path] = None
            return None
        self.trace[path] = (token, hashlib.sha256(data).hexdigest())
        return data, actual_mode

    def tree(self, path, installed=False):
        fd = self.directory(path)
        require(fd is not None, "Bundle/release directory missing")
        entries, contents = {}, {}
        total = 0

        def visit(parent, prefix, depth):
            nonlocal total
            require(depth <= 32, "Bundle nesting exceeds limit")
            before = identity(os.fstat(parent))
            for name in sorted(os.listdir(parent)):
                require(re.fullmatch(r"[A-Za-z0-9_.-]+", name)
                        and name not in (".", ".."), "Unsafe bundle filename")
                rel = prefix + name
                if not prefix and name == MANIFEST and installed:
                    continue
                require(name != MANIFEST, "Reserved manifest filename in source")
                s = os.stat(name, dir_fd=parent, follow_symlinks=False)
                require(s.st_uid == os.geteuid() and not s.st_mode & 0o7022,
                        "Unsafe bundle ownership or mode")
                require(len(entries) < MAX_ENTRIES, "Bundle entry limit exceeded")
                if stat.S_ISDIR(s.st_mode):
                    entries[rel] = {"type": "directory", "mode": stat.S_IMODE(s.st_mode)}
                    child = os.open(name, DIR_FLAGS, dir_fd=parent)
                    try:
                        require(identity(s) == identity(os.fstat(child)), "Bundle directory changed")
                        visit(child, rel + "/", depth + 1)
                    finally:
                        os.close(child)
                else:
                    data, mode, token = self.read_at(parent, name)
                    require(identity(s) == token, "Bundle entry changed")
                    total += len(data)
                    require(total <= MAX_TOTAL, "Bundle byte limit exceeded")
                    entries[rel] = {"type": "file", "mode": mode,
                                    "sha256": hashlib.sha256(data).hexdigest()}
                    contents[rel] = data
                    self.trace[path + "/" + rel] = token
            require(before == identity(os.fstat(parent)), "Bundle changed during traversal")
            self.trace[path + "/" + prefix] = before

        visit(fd, "", 0)
        return entries, contents

    def publish(self, path, data, mode):
        with Filesystem(self.effects) as scoped:
            fd, name = scoped.parent(path, create=True)
            self._publish_at(fd, name, path, data, mode)

    def _publish_at(self, fd, name, path, data, mode):
        temp = ".smith-next-stage-" + secrets.token_hex(12)
        f = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                    0o600, dir_fd=fd)
        try:
            with os.fdopen(f, "wb") as output:
                output.write(data)
                os.fchmod(output.fileno(), mode)
                output.flush()
                os.fsync(output.fileno())
            os.link(temp, name, src_dir_fd=fd, dst_dir_fd=fd, follow_symlinks=False)
            self.effects.append("published " + path)
        finally:
            os.unlink(temp, dir_fd=fd)
            os.fsync(fd)


def manifest_bytes(entries):
    return json.dumps({"schema": 1, "entries": entries}, sort_keys=True,
                      separators=(",", ":")).encode() + b"\n"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def verify_release(fs, path):
    require(fs.directory(path, private=True) is not None, "Release missing")
    stored = fs.file(path + "/" + MANIFEST, 0o600)
    require(stored is not None, "Release has no managed manifest")
    entries, _ = fs.tree(path, installed=True)
    require(stored[0] == manifest_bytes(entries)
            and digest(stored[0]) == path.rsplit("/", 1)[1],
            "Release manifest/content identity mismatch")


def rename_at(src_fd, src, dst_fd, dst, flags):
    # Ubuntu/Linux renameat2 supplies no-clobber publication and lossless exchange.
    libc = ctypes.CDLL(None, use_errno=True)
    rename = libc.renameat2
    rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int,
                       ctypes.c_char_p, ctypes.c_uint]
    rename.restype = ctypes.c_int
    if rename(src_fd, os.fsencode(src), dst_fd, os.fsencode(dst), flags) != 0:
        code = ctypes.get_errno()
        raise OSError(code, os.strerror(code))


def no_replace(src_fd, src, dst_fd, dst):
    rename_at(src_fd, src, dst_fd, dst, 1)


def exchange(src_fd, src, dst_fd, dst):
    rename_at(src_fd, src, dst_fd, dst, 2)


def selector_token(fd, name):
    before = os.stat(name, dir_fd=fd, follow_symlinks=False)
    require(stat.S_ISLNK(before.st_mode) and before.st_uid == os.geteuid()
            and before.st_nlink == 1, "Unknown selector object: " + name)
    target = os.readlink(name, dir_fd=fd)
    require(identity(before) == identity(os.stat(name, dir_fd=fd, follow_symlinks=False)),
            "Selector changed while reading: " + name)
    return identity(before), target


def same_moved_selector(actual, expected):
    # rename changes ctime, but inode, type, owner, mode, links, size, mtime and target
    # must still match the captured selector. This is detection, not filesystem CAS.
    return actual[0][:-1] == expected[0][:-1] and actual[1] == expected[1]


class Installer:
    def __init__(self, client=None, service=None, environ=None):
        env = os.environ if environ is None else environ
        self.client, self.service = client, service
        self.home = env.get("HOME", "")
        self.data = env.get("XDG_DATA_HOME", self.home + "/.local/share")
        self.base = self.data + "/smith-next"
        self.client_root = self.base + "/client"
        self.current = self.client_root + "/current"
        self.binary = self.base + "/bin/smith-service"
        self.effects = []

    def current_state(self, fs):
        fd, name = fs.parent(self.current)
        if fd is None:
            return None
        try:
            s = os.stat(name, dir_fd=fd, follow_symlinks=False)
        except FileNotFoundError:
            fs.trace[self.current] = None
            return None
        require(stat.S_ISLNK(s.st_mode) and s.st_uid == os.geteuid()
                and s.st_nlink == 1, "Unknown current selector")
        target = os.readlink(name, dir_fd=fd)
        require(re.fullmatch(r"releases/[0-9a-f]{64}", target), "Unmanaged current target")
        verify_release(fs, self.client_root + "/" + target)
        token = identity(s), target
        fs.trace[self.current] = token
        return token

    def inspect(self, fs):
        require(sys.platform == "linux" and os.geteuid() != 0,
                "Installer requires unprivileged Linux user")
        require(self.client or self.service, "Select --client and/or --service")
        require(fs.directory(self.data) is not None, "Existing XDG data root required")
        if self.client:
            require(fs.directory(self.home) is not None, "Existing HOME root required")
        # Private debug namespaces must remain compatible with the native helper.
        for path in (self.base, self.base + "/bin", self.client_root,
                     self.client_root + "/releases"):
            fs.directory(path, private=True)
        helper = None
        entries, contents, release = None, None, ""
        if self.client:
            entries, contents = fs.tree(self.client)
            for name in ("smith_client", "smith_backend/smith-service"):
                require(name in contents and contents[name] and entries[name]["mode"] == 0o755,
                        "Bundle requires exact 0755 " + name)
            for name in ("lib", "data", "data/flutter_assets"):
                require(entries.get(name, {}).get("type") == "directory",
                        "Incomplete Flutter bundle: " + name)
            for name in ("lib/libflutter_linux_gtk.so", "lib/libapp.so",
                         "data/icudtl.dat", "share/pixmaps/" + APP + ".png",
                         "share/applications/" + APP + ".desktop"):
                require(name in contents and contents[name], "Missing bundle input: " + name)
            helper = contents["smith_backend/smith-service"]
            release = digest(manifest_bytes(entries))
        if self.service:
            source = fs.file(self.service, 0o755)
            if source is None or not source[0]:
                raise Unsafe("Service artifact missing or empty")
            require(helper is None or helper == source[0], "Bundled/standalone helper mismatch")
            helper = source[0]
        installed = fs.file(self.binary, 0o755)
        require(installed is None or installed[0] == helper,
                "Existing service differs; upgrades require separate approval")
        current = self.current_state(fs)
        if self.service and not self.client and current is not None:
            selected = fs.file(self.client_root + "/" + current[1]
                               + "/smith_backend/smith-service", 0o755)
            require(selected is not None and selected[0] == helper,
                    "Standalone service differs from selected client helper")
        files = {}
        if self.client:
            path = self.client_root + "/releases/" + release
            if fs.directory(path) is not None:
                verify_release(fs, path)
            executable = self.current + "/smith_client"
            icon = self.current + "/share/pixmaps/" + APP + ".png"
            files[self.data + "/applications/" + APP + ".desktop"] = (
                ("[Desktop Entry]\nType=Application\nName=smith\nComment=Smith desktop client\n"
                 f"Exec={executable}\nIcon={icon}\nStartupWMClass={APP}\n"
                 "Terminal=false\nCategories=Utility;\n").encode(), 0o644)
            files[self.home + "/.local/bin/smith-next"] = (
                (f'#!/bin/sh\n# Managed by Smith next artifact installer.\nexec "{executable}" "$@"\n').encode(),
                0o755)
            for destination, (data, mode) in files.items():
                existing = fs.file(destination, mode)
                require(existing is None or existing[0] == data,
                        "Unknown destination content: " + destination)
        return {"entries": entries, "contents": contents, "release": release,
                "helper": helper, "installed": installed, "current": current, "files": files}

    def describe(self, plan, output):
        print("Artifact installation only; no build, GUI launch, unit registration or service start.",
              file=output)
        if self.client:
            print("Client source: " + self.client, file=output)
            print("Client manifest SHA256: " + plan["release"], file=output)
            print("Release: " + self.client_root + "/releases/" + plan["release"], file=output)
            print("Select current last: " + self.current, file=output)
            for path in plan["files"]:
                print("Managed file: " + path, file=output)
        print("Helper SHA256: " + digest(plan["helper"]), file=output)
        if self.service:
            print("Service source: " + self.service, file=output)
            print("Standalone binary (no overwrite): " + self.binary, file=output)
        print("Installation lock: " + self.data + "/.smith-next-install.lock", file=output)
        print("Creates named parents/staging files and a persistent private installation lock; "
              "preserves earlier releases. Multiple publications are not a global transaction.",
              file=output)
        if self.client:
            print("Selector updates use atomic exchange and retain displaced objects under "
                  + self.client_root + "/.smith-next-selector-*. Conflicts may leave current "
                  "changed; no automatic rollback or same-UID filesystem CAS is claimed.", file=output)

    @contextlib.contextmanager
    def lock(self, fs):
        fd = fs.directory(self.data)
        name = ".smith-next-install.lock"
        created = False
        try:
            lock = os.open(name, os.O_RDWR | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                           0o600, dir_fd=fd)
            created = True
        except FileExistsError:
            lock = os.open(name, os.O_RDWR | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=fd)
        try:
            s = os.fstat(lock)
            require(stat.S_ISREG(s.st_mode) and s.st_uid == os.geteuid()
                    and s.st_nlink == 1 and stat.S_IMODE(s.st_mode) == 0o600,
                    "Unsafe installation lock")
            if created:
                self.effects.append("created installation lock")
                os.fsync(lock)
                os.fsync(fd)
            deadline = time.monotonic() + LOCK_SECONDS
            while True:
                try:
                    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    break
                except BlockingIOError:
                    require(time.monotonic() < deadline, "Installation lock timeout")
                    time.sleep(0.05)
            require(identity(s) == identity(os.stat(name, dir_fd=fd, follow_symlinks=False)),
                    "Installation lock replaced")
            yield
        finally:
            os.close(lock)

    def release(self, fs, plan):
        path = self.client_root + "/releases/" + plan["release"]
        if fs.directory(path) is not None:
            verify_release(fs, path)
            return
        fd = fs.directory(self.client_root + "/releases", create=True, private=True)
        stage = ".smith-next-stage-" + secrets.token_hex(12)
        os.mkdir(stage, 0o700, dir_fd=fd)
        root = self.client_root + "/releases/" + stage
        self.effects.append("created release staging directory " + root)
        # A failed stage is deliberately retained for diagnosis; never delete unknown data.
        for rel, entry in sorted(plan["entries"].items()):
            if entry["type"] == "directory":
                with Filesystem(self.effects) as scoped:
                    scoped.directory(root + "/" + rel, create=True)
            else:
                fs.publish(root + "/" + rel, plan["contents"][rel], entry["mode"])
        fs.publish(root + "/" + MANIFEST, manifest_bytes(plan["entries"]), 0o600)
        # Directory modes are part of content identity, applied after all children are synced.
        for rel, entry in sorted(plan["entries"].items(), reverse=True):
            if entry["type"] == "directory":
                with Filesystem(self.effects) as scoped:
                    child = scoped.directory(root + "/" + rel)
                    if child is None:
                        raise Unsafe("Staged directory disappeared")
                    os.fchmod(child, entry["mode"])
                    os.fsync(child)
        stage_fd = fs.directory(root)
        os.fsync(stage_fd)
        no_replace(fd, stage, fd, plan["release"])
        self.effects.append("published immutable release " + path)
        os.fsync(fd)
        verify_release(fs, path)

    def verify_dependencies(self, fs, plan):
        verify_release(fs, self.client_root + "/releases/" + plan["release"])
        for path, expected in plan["files"].items():
            require(fs.file(path, expected[1]) == expected,
                    "Published launcher/wrapper changed: " + path)
        installed = fs.file(self.binary, 0o755)
        if self.service or plan["installed"] is not None:
            require(installed is not None and installed[0] == plan["helper"], "Service changed")
        else:
            require(installed is None or installed[0] == plan["helper"], "Service changed")

    def apply(self, fs, plan):
        if self.service and plan["installed"] is None:
            fs.directory(self.base, create=True, private=True)
            fs.directory(self.base + "/bin", create=True, private=True)
            fs.publish(self.binary, plan["helper"], 0o755)
        if not self.client:
            require(fs.file(self.binary, 0o755) == (plan["helper"], 0o755),
                    "Service changed before final verification")
            return
        fs.directory(self.base, create=True, private=True)
        fs.directory(self.client_root, create=True, private=True)
        self.release(fs, plan)
        for path, (data, mode) in plan["files"].items():
            existing = fs.file(path, mode)
            require(existing is None or existing[0] == data, "Destination changed")
            if existing is None:
                fs.publish(path, data, mode)
        fd = fs.directory(self.client_root)
        target = "releases/" + plan["release"]
        if plan["current"] is not None and plan["current"][1] == target:
            self.verify_dependencies(fs, plan)
            require(self.current_state(fs) == plan["current"], "Current selector changed")
            return
        stage = ".smith-next-selector-" + secrets.token_hex(12)
        retained = self.client_root + "/" + stage
        os.symlink(target, stage, dir_fd=fd)
        self.effects.append("created selector staging object " + retained)
        staged = selector_token(fd, stage)
        require(self.current_state(fs) == plan["current"], "Current selector changed")
        self.verify_dependencies(fs, plan)
        if plan["current"] is None:
            no_replace(fd, stage, fd, "current")
            self.effects.append("published current selector; verification pending")
        else:
            exchange(fd, stage, fd, "current")
            self.effects.append("exchanged current selector; displaced object retained at "
                                + retained + "; verification pending, no rollback")
        # Never unlink the displaced pathname, even on success: another same-UID
        # writer could replace it after inspection. Retention avoids a second race
        # and rollback could overwrite a newer current. Exchange preserves both objects.
        os.fsync(fd)
        if plan["current"] is not None:
            try:
                displaced = selector_token(fd, stage)
                require(same_moved_selector(displaced, plan["current"]),
                        "Captured selector identity mismatch")
            except (OSError, Unsafe) as exc:
                raise Unsafe("Displaced selector conflict; retained at " + retained
                             + "; current may have changed, no rollback: " + str(exc)) from exc
        require(same_moved_selector(selector_token(fd, "current"), staged),
                "Current changed after publication; retained objects require inspection")
        self.verify_dependencies(fs, plan)
        require(same_moved_selector(selector_token(fd, "current"), staged),
                "Current changed during final verification; no rollback")
        self.effects.append("selected verified current release " + plan["release"])

    def run(self, dry_run=False, yes=False, confirm=input, output=sys.stdout):
        try:
            with Filesystem(self.effects) as fs:
                plan = self.inspect(fs)
                trace = dict(fs.trace)
            self.describe(plan, output)
            if dry_run:
                print("Dry run: no mutation.", file=output)
                return 0
            if not yes and confirm("Install these exact artifacts/effects? [y/N] ").lower() != "y":
                print("Declined: no mutation.", file=output)
                return 0
            # Reopen everything after consent, then again while holding the cooperative lock.
            with Filesystem(self.effects) as fs:
                require(self.inspect(fs) == plan and fs.trace == trace,
                        "Artifacts or destinations changed after confirmation")
            with (Filesystem(self.effects) as locking, self.lock(locking),
                  Filesystem(self.effects) as fs):
                require(self.inspect(fs) == plan and fs.trace == trace,
                        "Artifacts or destinations changed before installation")
                self.apply(fs, plan)
            print("Installation complete; nothing launched.", file=output)
            return 0
        except (OSError, Unsafe, EOFError, KeyboardInterrupt, AttributeError) as exc:
            print("Installation stopped: " + str(exc), file=output)
            print("No rollback assumed; inspect reported paths before retry.", file=output)
            return 1
        finally:
            if self.effects:
                print("Completed/partial filesystem effects (including staging):", file=output)
                for effect in self.effects:
                    print("  " + effect, file=output)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--client", metavar="BUNDLE", help="complete prebuilt Linux bundle")
    parser.add_argument("--service", metavar="BINARY", help="prebuilt standalone service")
    parser.add_argument("--dry-run", action="store_true", help="read-only inspection; no locks")
    parser.add_argument("--yes", action="store_true", help="approve displayed effects, not safety bypass")
    args = parser.parse_args(argv)
    if not args.client and not args.service:
        parser.error("at least one of --client or --service is required")
    client = os.path.abspath(args.client) if args.client else None
    service = os.path.abspath(args.service) if args.service else None
    return Installer(client, service).run(args.dry_run, args.yes)


if __name__ == "__main__":
    sys.exit(main())
