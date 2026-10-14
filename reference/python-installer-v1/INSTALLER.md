# Smith recipient package

This package contains `smith-installer` (an executable Python stdlib zipapp), the complete Linux desktop payload, the matching service binary, `manifest.json` and `SHA256SUMS`. Keep the whole directory together. It is independent of the repository, Dagger, Docker, Rust/Cargo, Flutter, pip and virtual environments.

Requirements: Ubuntu x86_64, Python 3.12+, Ubuntu GTK3/Flutter runtime libraries, a trusted non-root account with existing safe HOME/XDG data/config/runtime roots and a usable systemd user manager. Ollama must be separately installed and already running at `127.0.0.1:11434`. This installer installs Smith artifacts and its isolated user-unit runner; it does NOT install/start Ollama, OS libraries, Python, GPU drivers or system services. Missing prerequisites are reported, not automatically provisioned. No model weights or voice providers are bundled.

Obtain the package from a trusted source and verify its independently retained digest/provenance. From the extracted package directory, `sha256sum -c SHA256SUMS` checks accidental changes, not authenticity against someone who can rewrite the manifest and checksums.

Read-only preflight:

    ./smith-installer --dry-run

Install Smith artifacts, then separately review any needed registration/start effects:

    ./smith-installer

Select an exact observed installed model (replace MODEL:TAG with the actual eligible local tag):

    ./smith-installer --model MODEL:TAG

For a missing explicitly chosen model, request a separate download confirmation:

    ./smith-installer --model MODEL:TAG --pull-model

Optionally request a separately confirmed minimal inference test:

    ./smith-installer --model MODEL:TAG --verify-inference

The installer never invents a recommended/default model. Observe existing tags first; select a model compatible with the workstation's available memory and the provider. Download consent requires typing the exact model. The external Ollama daemon may use internet and persistent disk; sizes and daemon continuation/partial cache after cancellation may be unknown. There are no retries or cache deletion. Model inventory/configuration is not proof of inference or GPU acceleration. The optional fixed direct test does not create a saved conversation turn.

Artifact publication, native user-unit registration/reload, Start, saved model preference, download and inference have separate confirmations. Declining initial publication makes no writes; declining later effects leaves earlier accepted artifacts/actions in place. Failures report partial/uncertain effects and do not claim rollback. There is no global --yes bypass.

After success, find **smith** in the app menu or run `~/.local/bin/smith-next`. Smith opens the conversation directly; installation state flags are not chat readiness. The application does not install/start services, pull models, enable login startup or run voice. Closing the desktop detaches; accepted work may continue. No login autostart/lingering or logout/reboot guarantee is added.

Existing different service/helper bytes deliberately block this installer: upgrades require a separately approved stop/backup/replacement/start plan. Never delete binaries or state to bypass a mismatch. Existing conversation data and voice-v1 metadata remain compatible with the text-only service; old speech jobs are inert, not re-executed.

Keep the extracted installer package for repair. The app's Open installer action shows the terminal invocation; it does not assume a terminal or silently launch an interactive installer as a headless child. No managed smith-installer command is installed by this first packaging slice; use the executable beside its payload. An offline model pack remains future work with separate license/provenance/import review.

This is a development candidate, not a production release. Source/fake/build checks are not installed-host inference, clean-host dependency, graphics, network-denial, model licensing or GPU compatibility evidence.
