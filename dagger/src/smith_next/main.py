"""Developer-only checks/builds; no installer, runtime, or release effects."""

import asyncio

import dagger
from dagger import dag, function, object_type


@object_type
class SmithNext:
    def _service(self, source: dagger.Directory) -> dagger.Container:
        return (
            dag.container()
            .from_("rust:1.98.0-bookworm")
            .with_exec(["rustup", "component", "add", "rustfmt", "clippy"])
            .with_directory("/src", source, exclude=["target", "build", "dist", ".git"])
            .with_exec(["install", "-d", "-m", "0755", "/home/smith"])
            .with_exec(["chown", "-R", "1000:1000", "/src", "/home/smith"])
            .with_env_variable("HOME", "/home/smith")
            .with_env_variable("CARGO_HOME", "/home/smith/.cargo")
            .with_user("1000:1000")
            .with_workdir("/src")
        )

    def _client(self, source: dagger.Directory, service: dagger.Directory) -> dagger.Container:
        return (
            dag.container()
            .from_("ubuntu:24.04")
            .with_env_variable("DEBIAN_FRONTEND", "noninteractive")
            .with_env_variable("CI", "true")
            .with_exec(["apt-get", "update"])
            .with_exec(
                [
                    "apt-get", "install", "--yes", "--no-install-recommends",
                    "ca-certificates", "curl", "git", "unzip", "xz-utils",
                    "clang", "cmake", "ninja-build", "pkg-config", "libgtk-3-dev",
                    "libstdc++-12-dev",
                ]
            )
            .with_exec(
                [
                    "git", "clone", "--depth", "1", "--branch", "3.47.2",
                    "https://github.com/flutter/flutter.git", "/opt/flutter",
                ]
            )
            .with_env_variable("PATH", "/opt/flutter/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin")
            .with_exec(["flutter", "--disable-analytics"])
            .with_exec(["dart", "--disable-analytics"])
            .with_exec(["flutter", "config", "--enable-linux-desktop"])
            .with_file("/opt/smith-service", self.build_service(service), permissions=0o755)
            .with_env_variable("SMITH_TEST_SERVICE", "/opt/smith-service")
            .with_directory(
                "/src", source,
                exclude=[
                                    "build", "dist", ".dart_tool", ".git", ".idea",
                                    ".pub-cache", ".pub", ".flutter-plugins-dependencies",
                                    "linux/flutter/ephemeral",
                                ],
            )
            .with_exec(["install", "-d", "-m", "0755", "/home/smith"])
            .with_exec(["chown", "-R", "1000:1000", "/src", "/opt/flutter", "/opt/smith-service", "/home/smith"])
            .with_env_variable("HOME", "/home/smith")
            .with_user("1000:1000")
            .with_workdir("/src")
            .with_exec(["flutter", "pub", "get", "--enforce-lockfile"])
        )

    def _service_tests(self, source: dagger.Directory) -> dagger.Container:
        # The supported host is Ubuntu 24.04; its Git 2.43 fixture behavior is
        # part of the service contract. Bookworm's older Git cannot exercise it.
        toolchain = dag.container().from_("rust:1.98.0-bookworm").with_exec(["rustup", "component", "add", "rustfmt", "clippy"])
        return (
            dag.container().from_("ubuntu:24.04")
            .with_exec(["apt-get", "update"])
            .with_exec(["apt-get", "install", "--yes", "--no-install-recommends", "build-essential", "ca-certificates", "git"])
            .with_directory("/usr/local/cargo", toolchain.directory("/usr/local/cargo"))
            .with_directory("/usr/local/rustup", toolchain.directory("/usr/local/rustup"))
            .with_directory("/src", source, exclude=["target", "build", "dist", ".git"])
            .with_exec(["install", "-d", "-m", "0755", "/home/smith"])
            .with_exec(["chown", "-R", "1000:1000", "/src", "/home/smith", "/usr/local/rustup"])
            .with_env_variable("HOME", "/home/smith")
            .with_env_variable("CARGO_HOME", "/home/smith/.cargo")
            .with_env_variable("RUSTUP_HOME", "/usr/local/rustup")
            .with_env_variable("PATH", "/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin")
            .with_user("1000:1000")
            .with_workdir("/src")
        )

    @function
    async def test_service(self, source: dagger.Directory) -> str:
        """Check and test an explicit service source directory."""
        return await (
            self._service_tests(source)
            .with_exec(["sh", "scripts/test.sh"])
            .stdout()
        )

    @function
    async def test_client(self, source: dagger.Directory, service: dagger.Directory) -> str:
        """Check and test an explicit client source directory."""
        cloud_fixture = (
            self._service(service)
            .with_exec(["cargo", "test", "--locked", "--test", "debug_service", "--no-run"])
            .with_exec(["sh", "-c", "find target/debug/deps -maxdepth 1 -type f -name 'debug_service-*' -executable -exec cp '{}' /tmp/smith-cloud-fixture ';'"])
            .file("/tmp/smith-cloud-fixture")
        )
        return await (
            self._client(source, service)
            .with_file("/opt/smith-cloud-fixture", cloud_fixture, permissions=0o755)
            .with_env_variable("SMITH_TEST_CLOUD_SERVICE", "/opt/smith-cloud-fixture")
            .with_exec(["sh", "scripts/test.sh"])
            .stdout()
        )

    def _installer(self, source: dagger.Directory, service: dagger.Directory) -> dagger.Container:
        return (
            dag.container()
            .from_("rust:1.98.0-bookworm")
            .with_exec(["rustup", "component", "add", "rustfmt", "clippy"])
            .with_directory("/src/installer", source, exclude=["target", "build", "dist", ".git"])
            .with_directory("/src/service", service, exclude=["target", "build", "dist", ".git"])
            .with_exec(["install", "-d", "-m", "0755", "/home/smith"])
            .with_exec(["chown", "-R", "1000:1000", "/src", "/home/smith"])
            .with_env_variable("HOME", "/home/smith")
            .with_env_variable("CARGO_HOME", "/home/smith/.cargo")
            .with_user("1000:1000")
            .with_workdir("/src/installer")
        )

    def _installer_tests(self, source: dagger.Directory, service: dagger.Directory) -> dagger.Container:
        # Compile CLI tests on the supported recipient OS without falsifying
        # platform preflight. Release artifacts retain the Bookworm ABI floor.
        toolchain = dag.container().from_("rust:1.98.0-bookworm").with_exec(["rustup", "component", "add", "rustfmt", "clippy"])
        return (
            dag.container().from_("ubuntu:24.04")
            .with_exec(["apt-get", "update"])
            .with_exec(["apt-get", "install", "--yes", "--no-install-recommends", "build-essential", "ca-certificates"])
            .with_directory("/usr/local/cargo", toolchain.directory("/usr/local/cargo"))
            .with_directory("/usr/local/rustup", toolchain.directory("/usr/local/rustup"))
            .with_directory("/src/installer", source, exclude=["target", "build", "dist", ".git"])
            .with_directory("/src/service", service, exclude=["target", "build", "dist", ".git"])
            .with_exec(["install", "-d", "-m", "0755", "/home/smith"])
            .with_exec(["chown", "-R", "1000:1000", "/src", "/home/smith", "/usr/local/rustup"])
            .with_env_variable("HOME", "/home/smith")
            .with_env_variable("CARGO_HOME", "/home/smith/.cargo")
            .with_env_variable("RUSTUP_HOME", "/usr/local/rustup")
            .with_env_variable("PATH", "/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin")
            .with_user("1000:1000").with_workdir("/src/installer")
        )

    def _installer_ui(self, source: dagger.Directory, service: dagger.Directory) -> dagger.Container:
        return (
            self._installer(source, service)
            .with_user("root")
            .with_exec(["apt-get", "update"])
            .with_exec(["apt-get", "install", "--yes", "--no-install-recommends", "libgtk-3-dev"])
            .with_user("1000:1000")
        )

    def _installer_ui_tests(self, source: dagger.Directory, service: dagger.Directory) -> dagger.Container:
        return (
            self._installer_tests(source, service)
            .with_user("root")
            .with_exec(["apt-get", "update"])
            .with_exec(["apt-get", "install", "--yes", "--no-install-recommends", "libgtk-3-dev", "xvfb", "xauth"])
            .with_user("1000:1000")
        )

    @function
    async def test_installer_ui(self, source: dagger.Directory, service: dagger.Directory) -> str:
        """Check the optional Rust GTK shell in an isolated Xvfb display."""
        return await self._installer_ui_tests(source, service).with_exec(["sh", "scripts/test-ui.sh"]).stdout()

    @function
    async def test_installer(self, source: dagger.Directory, service: dagger.Directory) -> str:
        """Run Rust quality/parity and a no-interpreter Ubuntu execution gate."""
        await asyncio.gather(
            self._installer_tests(source, service).with_exec(["sh", "scripts/test.sh"]).stdout(),
            self.test_native_runtime(source, service),
            self.test_installer_ui(source, service),
        )
        return "Native installer quality/parity, isolated GTK UI and interpreter-free Ubuntu CLI runtime checks passed."

    @function
    async def test_native_runtime(self, installer: dagger.Directory, service: dagger.Directory) -> str:
        """Execute the release ELF in Ubuntu without Python or builder toolchains."""
        return await (
            dag.container().from_("ubuntu:24.04")
            .with_file("/fixture/smith-installer", self.build_installer_binary(installer, service), permissions=0o755)
            .with_file("/fixture/smith-service", self.build_service(service), permissions=0o755)
            .with_exec(["install", "-d", "-m", "0700", "/home/smith/.local/share"])
            .with_exec(["chown", "-R", "1000:1000", "/fixture", "/home/smith"])
            .with_exec(["/usr/bin/test", "!", "-e", "/usr/bin/python3"])
            .with_exec(["/usr/bin/test", "!", "-e", "/usr/local/bin/python3"])
            .with_env_variable("HOME", "/home/smith")
            .with_env_variable("XDG_DATA_HOME", "/home/smith/.local/share")
            .with_env_variable("PATH", "/unavailable")
            .with_user("1000:1000")
            .with_exec(["/fixture/smith-installer", "--help"])
            .with_exec(["/fixture/smith-installer", "--version"])
            .with_exec(["/fixture/smith-installer", "--artifacts-only", "--service", "/fixture/smith-service", "--dry-run"])
            .with_exec(["/usr/bin/test", "!", "-e", "/home/smith/.local/share/smith-next"])
            .with_exec(["/usr/bin/test", "!", "-e", "/home/smith/.local/share/.smith-next-install.lock"])
            .stdout()
        )

    @function
    async def test_package(self, source: dagger.Directory, installer: dagger.Directory, service: dagger.Directory) -> str:
        """Test builder metadata and compiled native package entrypoints under private roots."""
        inputs = dag.directory().with_file("package.py", source.file("package.py")).with_file("runtime.py", source.file("runtime.py")).with_file("tests/test_package.py", source.file("tests/test_package.py"))
        return await (
            dag.container()
            .from_("ubuntu:24.04")
            .with_exec(["apt-get", "update"])
            .with_exec(["apt-get", "install", "--yes", "--no-install-recommends", "python3", "libgtk-3-0t64"])
            .with_directory("/src", inputs)
            .with_file("/opt/smith-installer", self.build_installer_binary(installer, service), permissions=0o755)
            .with_file("/opt/smith-installer-ui", self.build_installer_ui(installer, service), permissions=0o755)
            .with_exec(["install", "-d", "-m", "0755", "/home/smith"])
            .with_exec(["chown", "-R", "1000:1000", "/src", "/opt/smith-installer", "/opt/smith-installer-ui", "/home/smith"])
            .with_env_variable("HOME", "/home/smith")
            .with_env_variable("PYTHONDONTWRITEBYTECODE", "1")
            .with_env_variable("SMITH_TEST_INSTALLER", "/opt/smith-installer")
                        .with_env_variable("SMITH_TEST_INSTALLER_UI", "/opt/smith-installer-ui")
            .with_user("1000:1000")
            .with_workdir("/src")
            .with_exec(["python3", "-m", "unittest", "discover", "-s", "tests", "-v"])
            .stdout()
        )

    @function
    async def test(self, service: dagger.Directory, client: dagger.Directory, installer: dagger.Directory, tooling: dagger.Directory) -> str:
        """Run application/native-installer quality and builder-only package checks."""
        await asyncio.gather(self.test_service(service), self.test_client(client, service), self.test_installer(installer, service), self.test_package(tooling, installer, service))
        return "Text service/client, native Rust installer and builder-only package tests passed."

    @function
    def build_service(self, source: dagger.Directory) -> dagger.File:
        """Return a release binary; export explicitly to service/dist/."""
        return (
            self._service(source)
            .with_exec(["sh", "scripts/build.sh"])
            .file("/src/target/release/smith-service")
        )

    @function
    def build_runner(self) -> dagger.Directory:
        """Assemble SHA-pinned CPU and Vulkan runners; never execute the Vulkan binary."""
        return (
            dag.container().from_("ubuntu:24.04")
            .with_exec(["apt-get", "update"])
            .with_exec(["apt-get", "install", "--yes", "--no-install-recommends", "curl", "ca-certificates", "python3", "libgomp1=14.2.0-4ubuntu2~24.04.1"])
            .with_file("/tools/runtime.py", dag.current_module().source().file("runtime.py"))
            .with_exec(["install", "-d", "-m", "0755", "/out"])
            .with_exec(["curl", "--fail", "--silent", "--show-error", "--location", "--proto", "=https", "--proto-redir", "=https", "--max-time", "180", "--output", "/tmp/runner-cpu.tar.gz", "https://github.com/ggml-org/llama.cpp/releases/download/b11429/llama-b11429-bin-ubuntu-x64.tar.gz"])
            .with_exec(["curl", "--fail", "--silent", "--show-error", "--location", "--proto", "=https", "--proto-redir", "=https", "--max-time", "180", "--output", "/tmp/runner-vulkan.tar.gz", "https://github.com/ggml-org/llama.cpp/releases/download/b11429/llama-b11429-bin-ubuntu-vulkan-x64.tar.gz"])
            .with_exec(["python3", "-B", "/tools/runtime.py", "--archive", "/tmp/runner-cpu.tar.gz", "--output", "/out/cpu", "--backend", "cpu"])
            .with_exec(["python3", "-B", "/tools/runtime.py", "--archive", "/tmp/runner-vulkan.tar.gz", "--output", "/out/vulkan", "--backend", "vulkan"])
            .with_exec(["env", "-i", "/out/cpu/llama-server", "--version"])
            .directory("/out")
        )

    @function
    def build_client(self, source: dagger.Directory, service: dagger.Directory) -> dagger.Directory:
        """Return the complete Linux bundle; export explicitly to client/dist/."""
        return (
            self._client(source, service)
            .with_directory("/opt/smith-runtime", self.build_runner())
            .with_exec(["sh", "scripts/build.sh", "/opt/smith-service", "/opt/smith-runtime"])
            .directory("/src/build/linux/x64/release/bundle")
        )

    @function
    def build_installer_binary(self, source: dagger.Directory, service: dagger.Directory) -> dagger.File:
        """Return the native installer executable independently of desktop packaging."""
        return self._installer(source, service).with_exec(["sh", "scripts/build.sh"]).file("/src/installer/target/release/smith-installer")

    @function
    def build_installer_ui(self, source: dagger.Directory, service: dagger.Directory) -> dagger.File:
        """Return the optional native GTK3 installer executable."""
        return self._installer_ui(source, service).with_exec(["sh", "scripts/build-ui.sh"]).file("/src/installer/target/release/smith-installer-ui")

    @function
    def build_installer(self, installer: dagger.Directory, client: dagger.Directory, service: dagger.Directory, tooling: dagger.Directory) -> dagger.Directory:
        """Return a native recipient executable plus paired payload; no host installation."""
        return (
            dag.container()
            .from_("ubuntu:24.04")
            .with_exec(["apt-get", "update"])
            .with_exec(["apt-get", "install", "--yes", "--no-install-recommends", "python3", "libgtk-3-0t64"])
            .with_file("/out/smith-installer", self.build_installer_binary(installer, service), permissions=0o755)
            .with_file("/out/smith-installer-ui", self.build_installer_ui(installer, service), permissions=0o755)
            .with_file("/tools/package.py", tooling.file("package.py"))
                        .with_file("/tools/runtime.py", tooling.file("runtime.py"))
            .with_file("/tools/README.txt", tooling.file("INSTALLER.md"))
            .with_directory("/out/payload/client", self.build_client(client, service))
            .with_file("/out/payload/service/smith-service", self.build_service(service), permissions=0o755)
            .with_exec(["install", "-d", "-m", "0755", "/home/smith"])
            .with_exec(["chown", "-R", "1000:1000", "/tools", "/out", "/home/smith"])
            .with_env_variable("HOME", "/home/smith")

            .with_env_variable("PYTHONDONTWRITEBYTECODE", "1")
            .with_user("1000:1000")
            .with_exec(["python3", "/tools/package.py", "--root", "/out", "--readme", "/tools/README.txt"])
            .with_exec(["/out/smith-installer", "--help"])
            .with_exec(["/out/smith-installer-ui", "--help"])
            .directory("/out")
        )
