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

    @function
    async def test_service(self, source: dagger.Directory) -> str:
        """Check and test an explicit service source directory."""
        return await (
            self._service(source)
            .with_exec(["sh", "scripts/test.sh"])
            .stdout()
        )

    @function
    async def test_client(self, source: dagger.Directory, service: dagger.Directory) -> str:
        """Check and test an explicit client source directory."""
        return await (
            self._client(source, service)
            .with_exec(["sh", "scripts/test.sh"])
            .stdout()
        )

    @function
    async def test_installer(self, source: dagger.Directory) -> str:
        """Test the native installer under isolated roots, without host effects."""
        inputs = dag.directory().with_file("install.py", source.file("install.py")).with_directory("tests", source.directory("tests"))
        return await (
            dag.container()
            .from_("ubuntu:24.04")
            .with_exec(["apt-get", "update"])
            .with_exec(["apt-get", "install", "--yes", "--no-install-recommends", "python3"])
            .with_directory("/src", inputs)
            .with_exec(["install", "-d", "-m", "0755", "/home/smith"])
            .with_exec(["chown", "-R", "1000:1000", "/src", "/home/smith"])
            .with_env_variable("HOME", "/home/smith")
            .with_env_variable("PYTHONDONTWRITEBYTECODE", "1")
            .with_user("1000:1000")
            .with_workdir("/src")
            .with_exec(["python3", "-m", "unittest", "discover", "-s", "tests", "-v"])
            .stdout()
        )

    @function
    async def test_sidecars(self, source: dagger.Directory) -> str:
        """Build the voice runtime and run fake-only tests without models or ports."""
        return await (
            source.docker_build()
            .with_directory("/app/tests", source.directory("tests"))
            .with_file("/app/compose.yaml", source.file("compose.yaml"))
            .with_workdir("/app")
            .with_exec(["python", "-m", "pip", "check"])
            .with_exec(["python", "-m", "unittest", "discover", "-s", "tests", "-v"])
            .stdout()
        )

    @function
    async def test(self, service: dagger.Directory, client: dagger.Directory, installer: dagger.Directory, sidecars: dagger.Directory) -> str:
        """Run independent component, installer and voice checks/tests concurrently."""
        await asyncio.gather(self.test_service(service), self.test_client(client, service), self.test_installer(installer), self.test_sidecars(sidecars))
        return "Service, client, isolated installer and voice sidecar tests passed."

    @function
    def build_service(self, source: dagger.Directory) -> dagger.File:
        """Return a release binary; export explicitly to service/dist/."""
        return (
            self._service(source)
            .with_exec(["sh", "scripts/build.sh"])
            .file("/src/target/release/smith-service")
        )

    @function
    def build_client(self, source: dagger.Directory, service: dagger.Directory) -> dagger.Directory:
        """Return the complete Linux bundle; export explicitly to client/dist/."""
        return (
            self._client(source, service)
            .with_exec(["sh", "scripts/build.sh", "/opt/smith-service"])
            .directory("/src/build/linux/x64/release/bundle")
        )
