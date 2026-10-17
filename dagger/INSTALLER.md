# Install Smith

Smith is a development candidate for **Ubuntu 24.04 LTS x86_64**, running as your normal user—not root.

## Before you start

You need a graphical desktop, compatible native libraries, safe user-owned XDG directories, and a working systemd user manager. The CLI needs no display or GTK. No Python, Rust, Flutter, Docker, or Dagger is required.

The installer does not provision OS libraries, GPU drivers, user managers, or keyrings. See [host requirements](../docs/HOST_PROFILE.md).

## Acquire, verify, extract and run

Obtain the complete installer package from a trusted source. Tagged GitHub Actions candidates are retained for 14 days; they are not automatically published releases.

Compare the archive's SHA-256 against an **independently trusted digest** before extracting. A checksum bundled beside the archive does not establish authenticity.

Replace `X.Y.Z` with your version, then extract into a fresh directory:

```sh
sha256sum smith-installer_vX.Y.Z.tar.gz
sha256sum -c smith-installer_vX.Y.Z.tar.gz.sha256
tar -xzf smith-installer_vX.Y.Z.tar.gz
cd smith-installer_vX.Y.Z
sha256sum -c SHA256SUMS
./smith-installer-ui
```

Keep the entire extracted folder, including both installers, `payload/`, manifests, checksums, and instructions, for maintenance.

## Visual installation

Choose **INSTALL / UPGRADE**, then follow the prompts:

1. Install the paired desktop and service. Registration, login **Enable**, and **Start** have separate confirmations. Enable does not start the service immediately.
2. Choose a local model. Fresh GUI setup preselects Qwen3-14B; review memory needs or choose a smaller model. Weights are downloaded separately. Download, selection, and inference checks require separate consent.
3. Optionally configure **Providers**: OpenAI API, Anthropic API, or both. API billing is separate from ChatGPT/Claude subscriptions.
4. After successful installation, choose **Launch Smith**, or open **smith** from the app menu.

Cloud keys require an existing unlocked GNOME Keyring. Enter them only through masked installer or hidden terminal input. Save and potentially billable Validate are separate actions. Chats offer LOCAL/CLOUD selection with context-sharing consent; no automatic cloud fallback.

## CLI alternative

```sh
./smith-installer --dry-run
./smith-installer --models
./smith-installer --model qwen3-4b:q4_k_m --download-model --verify-inference
./smith-installer --providers
```

Dry-run is read-only artifact preflight. The model command performs full setup, not model-only maintenance. There is no blanket recipient `--yes`.

## Maintenance

Rerun the retained visual installer for **Upgrade**, **Manage models**, **Manage providers**, or **Uninstall**. Use Upgrade for differing installed binaries; do not delete files to bypass checks. Back up state before migration; binary-only downgrade is not rollback.

Uninstall removes active integration but retains conversations, models, preferences, backups, and keyring items. It does not revoke API credentials. Failures or cancellation preserve earlier effects; no automatic rollback occurs.

Closing the desktop leaves the service running. See [validation](../VALIDATION.md) for tested behavior and remaining limits.
