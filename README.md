# Smith

Local-first autonomy substrate for your Ubuntu workstation, with a Flutter desktop, persistent Rust service, and native installer.

- **Local or cloud:** bundled CPU/Vulkan runtimes for curated local models, plus optional OpenAI and Anthropic APIs.
- **Switch models per chat:** choose LOCAL or CLOUD with explicit consent before sharing conversation context.
- **Persistent conversations:** closing the desktop leaves the service and accepted work running.

**Status:** development candidate for Ubuntu 24.04 LTS x86_64. Agent execution and broader orchestration are not available yet.

## Install

Follow the [installation guide](dagger/INSTALLER.md) to verify and run the complete installer package. No developer toolchain is needed on the recipient workstation.

Cloud APIs require separate API billing—not ChatGPT or Claude subscriptions—and an unlocked GNOME Keyring. Local model weights are downloaded separately.

## Develop

From the repository root, with Docker and Dagger available:

```sh
make help
make test
make build
```

Code lives in `client/`, `service/`, and `installer/`. Build tooling lives in `dagger/`.

## Documentation

[Scope](docs/SCOPE.md) · [Architecture](docs/ARCHITECTURE.md) · [Host requirements](docs/HOST_PROFILE.md) · [Validation](VALIDATION.md)

Contributing? Start with [AGENTS.md](AGENTS.md).

[License](LICENSE) · [Security](SECURITY.md)
