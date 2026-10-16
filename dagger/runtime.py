"""Builder-only pinned CPU/Vulkan runtime assembly; never downloads model weights."""
import argparse
import hashlib
import shutil
import tarfile
from pathlib import Path, PurePosixPath

PREFIX = "llama-b11429/"
CPU = ("alderlake", "cannonlake", "cascadelake", "cooperlake", "haswell", "icelake", "ivybridge", "piledriver", "sandybridge", "sapphirerapids", "skylakex", "sse42", "x64", "zen4")
FILES = ("llama-server", "libllama-server-impl.so", "libllama-common.so.0", "libmtmd.so.0", "libllama.so.0", "libggml.so.0", "libggml-base.so.0") + tuple("libggml-cpu-" + cpu + ".so" for cpu in CPU)
BACKENDS = {
    "cpu": {
        "url": "https://github.com/ggml-org/llama.cpp/releases/download/b11429/llama-b11429-bin-ubuntu-x64.tar.gz",
        "size": 17693462,
        "sha256": "f6d25dde8f51133143d1453da4fd5f73b145127177612a283bf7995957af3392",
        "files": FILES,
    },
    "vulkan": {
        "url": "https://github.com/ggml-org/llama.cpp/releases/download/b11429/llama-b11429-bin-ubuntu-vulkan-x64.tar.gz",
        "size": 31636673,
        "sha256": "632c4e98feba2b94407a2130e3133e0c3aefb0ea1ab41337e926d8bfafdd0b74",
        "files": FILES + ("libggml-vulkan.so",),
    },
}
VULKAN_LIBRARY_SIZE = 44315144
MEMBER_LIMIT = 16 * 1024 * 1024


def assemble(archive, output, backend):
    selected = BACKENDS[backend]
    assert archive.stat().st_size == selected["size"], "Unexpected archive size"
    assert hashlib.sha256(archive.read_bytes()).hexdigest() == selected["sha256"], "Pinned runner checksum mismatch"
    output.mkdir(mode=0o755)
    with tarfile.open(archive, "r:gz") as tar:
        members = tar.getmembers()
        assert len(members) < 256 and sum(m.size for m in members) < 128 * 1024 * 1024
        lookup = {}
        for member in members:
            name = member.name.rstrip("/")
            assert name == PREFIX.rstrip("/") or name.startswith(PREFIX), "Unexpected archive path"
            assert ".." not in PurePosixPath(name).parts and not name.startswith("/")
            assert name not in lookup, "Duplicate archive path"
            lookup[name] = member

        def content(name):
            member = lookup[PREFIX + name]
            for _ in range(3):
                if member.isfile():
                    if name == "libggml-vulkan.so":
                        assert member.size == VULKAN_LIBRARY_SIZE
                    else:
                        assert member.size <= MEMBER_LIMIT
                    extracted = tar.extractfile(member)
                    assert extracted is not None, "Archive member has no content"
                    return extracted.read()
                assert member.issym(), "Unexpected archive type"
                target = member.linkname
                assert target == PurePosixPath(target).name and target not in (".", "..")
                member = lookup[PREFIX + target]
            raise ValueError("Archive link chain exceeds bound")

        for name in selected["files"]:
            destination = output / name
            destination.write_bytes(content(name))
            destination.chmod(0o755)
        (output / "LICENSE.llama.cpp").write_bytes(content("LICENSE"))
    for source, name in (
        ("/usr/lib/x86_64-linux-gnu/libgomp.so.1", "libgomp.so.1"),
        ("/usr/share/doc/libgomp1/copyright", "LICENSE.libgomp"),
        ("/usr/share/common-licenses/GPL-3", "LICENSE.GPL-3"),
    ):
        shutil.copyfile(source, output / name)
        (output / name).chmod(0o755 if name.endswith(".so.1") else 0o644)
    (output / "LICENSE.llama.cpp").chmod(0o644)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--backend", choices=tuple(BACKENDS), required=True)
    args = parser.parse_args()
    assemble(args.archive, args.output, args.backend)
