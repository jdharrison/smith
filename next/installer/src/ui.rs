//! Optional GTK3 recipient shell. The worker owns all publication/Core state;
//! this binary only displays reviews and sends explicit, request-bound decisions.
pub mod appearance;

use gtk::glib::{self, ControlFlow, Propagation};
use gtk::prelude::*;
use smith_installer::launch::{DesktopLauncher, LaunchResult, NativeLauncher};
use smith_installer::providers::{Action as ProviderAction, Catalog as ProviderCatalog, Provider};
use smith_installer::secret::Secret;
use smith_installer::wizard::{Choices, Command, Event, Report, Scope, Session, StepState};
use smith_installer::{consent, Result};
use std::cell::RefCell;
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(100);
const EVENTS_PER_TICK: usize = 8;
const EFFECT_TEXT_LIMIT: usize = 128 * 1024;
const LOG_LIMIT: usize = 64 * 1024;
const TEXT_LIMIT: usize = 4096;
const REOPEN_GUIDANCE: &str = "Cancellation remains active for this installer process. Close and reopen the installer before management, upgrade or uninstall. Accepted/uncertain effects remain; no cancellation flag was cleared.";
const HELP: &str = "Smith graphical installer — prepared Ubuntu 24.04 LTS x86_64\n\n\
Usage: smith-installer-ui [--bundle ROOT]\n\
       smith-installer-ui --help | --version | --licenses\n\n\
  --bundle ROOT  Complete recipient package (default: actual executable directory)\n\
  -h, --help     Print this finite document; no display or operational discovery\n\
  -V, --version  Print version; no display or operational discovery\n\
      --licenses Print bundled appearance notices; no display or discovery\n\n\
Requires compatible GTK3 libraries and a graphical session for installation.\n\
Includes the Smith app, menu/icon, user command and standalone service binary.\n\
Install approves only the exact displayed artifact review. Register Smith service\n\
and Start now are requested by default, with separate inline confirmations.\n\
Model choice/download and a separately confirmed service-backed check follow.\n\
Then optional Providers configures OpenAI/Anthropic independently; Save is not Validate.\n\
Manage models/providers bypasses artifact publication; exact-model removal is service-mediated.\n\
CPU/Vulkan model runtimes and CPU fallback are bundled; no Ollama dependency. No OS/GPU provisioning, implicit upgrades or lingering changes.\n\
No automatic app launch or rollback; stopping preserves accepted/uncertain effects.\n\
The headless smith-installer CLI remains separate; --providers manages optional API keys.\n";
const CSS: &str = "
window.smith-installer, window.smith-stop-dialog {
    background-color: #031514;
    color: #E3F0EE;
    font-family: 'JetBrains Mono';
    font-size: 1.05em;
}
.smith-installer scrolledwindow, .smith-installer viewport, .smith-installer stack {
    background-color: transparent;
}
.smith-installer label, .smith-stop-dialog label { color: #E3F0EE; }
.smith-installer .brand { color: #00E9C6; font-size: 1.57em; font-weight: 600; }
.smith-installer .title { font-size: 1.57em; font-weight: 600; }
.smith-installer .section { font-weight: 500; color: #00E9C6; }
.smith-installer .secondary { color: #99B5B2; }
.smith-installer label selection, .smith-stop-dialog label selection {
    background-color: #063D34;
    color: #E3F0EE;
}
.smith-installer .metadata { color: #99B5B2; font-size: 0.86em; }
.smith-installer .panel {
    background-color: #04201D;
    border: 1px solid #164A43;
    border-radius: 6px;
    padding: 16px;
}
.smith-installer button, .smith-stop-dialog button {
    background-image: none;
    background-color: #092A27;
    color: #E3F0EE;
    border: 1px solid #0B6557;
    border-radius: 6px;
    box-shadow: none;
    padding: 8px 12px;
    min-height: 30px;
    min-width: 30px;
}
.smith-installer button.suggested-action { background-color: #063D34; border-color: #00E9C6; }
.smith-installer button.suggested-action label { color: #00E9C6; }
.smith-installer button.landing-action { font-size: 1.14em; font-weight: 500; }
.smith-installer button.destructive-action { border-color: #FF4D58; }
.smith-installer button:hover, .smith-stop-dialog button:hover { background-color: #063D34; }
.smith-installer button:disabled { background-color: #04201D; color: #789290; border-color: #164A43; }
.smith-installer button:disabled label { color: #789290; }
.smith-installer button:focus, .smith-stop-dialog button:focus {
    outline: 2px solid #00FFE0;
    outline-offset: 2px;
}
.smith-installer check { background-color: #04201D; border: 1px solid #0B6557; }
.smith-installer check:checked { background-color: #00E9C6; color: #031514; }
.smith-installer checkbutton:disabled label { color: #789290; }
.smith-installer progressbar trough { background-color: #092A27; min-height: 6px; border-radius: 4px; }
.smith-installer progressbar progress { background-color: #00E9C6; min-height: 6px; border-radius: 4px; }
.smith-installer separator { background-color: #164A43; min-height: 1px; }
";

#[derive(Debug, PartialEq, Eq)]
enum Launch {
    Help,
    Version,
    Licenses,
    Gui { bundle: Option<PathBuf> },
}

// Parsing is deliberately independent of executable/display/root discovery.
fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Launch> {
    let mut args = args.into_iter();
    let (mut bundle, mut help, mut version, mut licenses) = (None, false, false, false);
    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("--help" | "-h") if !help => help = true,
            Some("--version" | "-V") if !version => version = true,
            Some("--licenses") if !licenses => licenses = true,
            Some("--bundle") if bundle.is_none() => {
                let value = args.next().ok_or("--bundle requires ROOT")?;
                if value.is_empty() || value.to_str().is_some_and(|s| s.starts_with('-')) {
                    return Err("--bundle requires a nonempty ROOT".into());
                }
                bundle = Some(PathBuf::from(value));
            }
            Some(value) if value.starts_with("--bundle=") && bundle.is_none() => {
                let value = &value["--bundle=".len()..];
                if value.is_empty() {
                    return Err("--bundle requires a nonempty ROOT".into());
                }
                bundle = Some(PathBuf::from(value));
            }
            _ => return Err("Unknown, repeated or invalid argument. Use --help for usage.".into()),
        }
    }
    Ok(if help {
        Launch::Help
    } else if version {
        Launch::Version
    } else if licenses {
        Launch::Licenses
    } else {
        Launch::Gui { bundle }
    })
}

fn dispatch(
    launch: Launch,
    document: impl FnOnce(&str) -> Result<()>,
    graphical: impl FnOnce(Option<PathBuf>) -> Result<i32>,
) -> Result<i32> {
    match launch {
        Launch::Help => document(HELP).map(|()| 0),
        Launch::Version => document(&format!(
            "smith-installer-ui {}\n",
            env!("CARGO_PKG_VERSION")
        ))
        .map(|()| 0),
        Launch::Licenses => document(concat!(
            "Smith installer appearance notices\n\n",
            include_str!("../assets/fonts/jetbrains-mono/OFL.txt"),
            "\n\n",
            include_str!("../assets/ICONS-NOTICE.txt"),
        ))
        .map(|()| 0),
        Launch::Gui { bundle } => graphical(bundle),
    }
}

fn display_preflight(display: Option<OsString>, wayland: Option<OsString>) -> Result<()> {
    if display.is_some_and(|s| !s.is_empty()) || wayland.is_some_and(|s| !s.is_empty()) {
        Ok(())
    } else {
        Err("No graphical session (DISPLAY or WAYLAND_DISPLAY). Use the separate smith-installer CLI, or open this installer from your desktop session.".into())
    }
}

fn main() -> ExitCode {
    let launch = match parse(std::env::args_os().skip(1)) {
        Ok(launch) => launch,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "smith-installer-ui: {error}");
            return ExitCode::from(2);
        }
    };
    let result = dispatch(
        launch,
        |text| {
            io::stdout()
                .lock()
                .write_all(text.as_bytes())
                .map_err(|_| "Cannot write installer documentation".into())
        },
        run_gui,
    );
    match result {
        Ok(code) => ExitCode::from(code as u8),
        Err(error) => {
            let _ = writeln!(
                io::stderr().lock(),
                "smith-installer-ui: {}",
                text(&error, TEXT_LIMIT)
            );
            ExitCode::from(1)
        }
    }
}

// This private seam permits GTK tests without constructing an effect worker.
// There is no production fake-session flag, environment override or demo mode.
trait SessionIo {
    fn send(&self, command: Command) -> Result<()>;
    fn try_event(&self) -> Result<Option<Event>>;
    fn is_finished(&self) -> bool;
    fn join(&mut self) -> Result<()>;
}
impl SessionIo for Session {
    fn send(&self, command: Command) -> Result<()> {
        Session::send(self, command)
    }
    fn try_event(&self) -> Result<Option<Event>> {
        Session::try_event(self)
    }
    fn is_finished(&self) -> bool {
        Session::is_finished(self)
    }
    fn join(&mut self) -> Result<()> {
        Session::join(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Landing,
    Inspecting,
    Review,
    Model,
    Providers,
    Working,
    Confirmation,
    Stopping,
    Finishing,
    Terminal,
}
struct Pending {
    request_id: u64,
    title: String,
}

#[derive(Clone)]
enum AfterReview {
    InstallOrUpgrade,
    Command(Command),
}

struct Widgets {
    window: gtk::Window,
    heading: gtk::Label,
    introduction: gtk::Label,
    stack: gtk::Stack,
    landing: gtk::Box,
    register: gtk::CheckButton,
    start: gtk::CheckButton,
    recipe: gtk::Label,
    review: gtk::Label,
    stage: gtk::Label,
    elapsed: gtk::Label,
    progress: gtk::ProgressBar,
    confirmation: gtk::Box,
    confirmation_title: gtk::Label,
    effects: gtk::Label,
    results: gtk::Box,
    outcome: gtk::Label,
    inference: gtk::Label,
    launch_status: gtk::Label,
    details: gtk::Label,
    footer_shell: gtk::Box,
    refresh: gtk::Button,
    secondary: gtk::Button,
    primary: gtk::Button,
    manage: gtk::Button,
    upgrade: gtk::Button,
    uninstall: gtk::Button,
    model: gtk::ComboBoxText,
    remove: gtk::CheckButton,
    model_facts: gtk::Label,
    provider_choice: gtk::ComboBoxText,
    provider_action: gtk::ComboBoxText,
    provider_model: gtk::ComboBoxText,
    provider_model_label: gtk::Label,
    provider_key: gtk::Entry,
    provider_key_label: gtk::Label,
    provider_reveal: gtk::CheckButton,
    provider_status: gtk::Label,
    manage_providers: gtk::Button,
    result_providers: gtk::Button,
}

fn label(value: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(value));
    label.set_xalign(0.0);
    label.set_yalign(0.0);
    label.set_line_wrap(true);
    label.set_line_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.set_selectable(true);
    label.set_hexpand(true);
    label.set_width_chars(1);
    label.set_max_width_chars(64);
    label
}
fn section(value: &str) -> gtk::Label {
    let label = label(value);
    label.style_context().add_class("section");
    label
}

impl Widgets {
    fn new() -> Self {
        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_title("smith - installer");
        window.set_default_size(720, 560);
        window.set_resizable(true);
        window.style_context().add_class("smith-installer");

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 16);
        header.set_border_width(16);
        let brand = label("> SMITH");
        brand.set_selectable(false);
        brand.style_context().add_class("brand");
        let version = label(concat!("INSTALLER · v", env!("CARGO_PKG_VERSION")));
        version.set_xalign(1.0);
        version.style_context().add_class("metadata");
        for caption in [&brand, &version] {
            caption.set_line_wrap_mode(gtk::pango::WrapMode::Word);
            caption.set_width_chars(-1);
        }
        header.pack_start(&brand, true, true, 0);
        header.pack_end(&version, true, true, 0);
        root.pack_start(&header, false, false, 0);
        root.pack_start(
            &gtk::Separator::new(gtk::Orientation::Horizontal),
            false,
            false,
            0,
        );
        let scroll = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 16);
        body.set_border_width(24);
        let heading = label("Set up Smith");
        heading.style_context().add_class("title");
        let introduction = label("Install or update Smith. Uninstall keeps your data.");
        introduction.style_context().add_class("secondary");
        body.pack_start(&heading, false, false, 0);
        body.pack_start(&introduction, false, false, 0);

        let stack = gtk::Stack::new();
        stack.set_homogeneous(false);
        let landing = gtk::Box::new(gtk::Orientation::Vertical, 16);
        let upgrade = gtk::Button::with_label("INSTALL / UPGRADE");
        let uninstall = gtk::Button::with_label("UNINSTALL");
        upgrade.set_image(Some(&appearance::icon_image(appearance::Icon::Archive)));
        uninstall.set_image(Some(&appearance::icon_image(appearance::Icon::Trash)));
        uninstall.style_context().add_class("destructive-action");
        for button in [&upgrade, &uninstall] {
            button.set_size_request(-1, 88);
            button.set_always_show_image(true);
            button.style_context().add_class("landing-action");
            landing.pack_start(button, false, false, 0);
        }
        stack.add_named(&landing, "landing");
        let choices = gtk::Box::new(gtk::Orientation::Vertical, 12);
        choices.style_context().add_class("panel");
        choices.pack_start(&section("Package contents"), false, false, 0);
        let register = gtk::CheckButton::with_label("Register Smith service");
        register.set_active(true);
        let start = gtk::CheckButton::with_label("Start now");
        start.set_active(true);
        let recipe = label("");
        let review = label("Reading the package and preparing a read-only review…");
        choices.pack_start(&review, false, false, 0);
        let manage_providers = gtk::Button::with_label("Manage providers");
        manage_providers.set_no_show_all(true);
        manage_providers.set_halign(gtk::Align::Start);
        choices.pack_start(&manage_providers, false, false, 0);
        stack.add_named(&choices, "review");
        let model_page = gtk::Box::new(gtk::Orientation::Vertical, 12);
        model_page.style_context().add_class("panel");
        let model = gtk::ComboBoxText::new();
        model.set_size_request(-1, 48);
        // A catalog row must wrap rather than impose its full single-line
        // width on the toplevel, especially at 200% text size.
        for cell in model.cells() {
            if let Ok(cell) = cell.downcast::<gtk::CellRendererText>() {
                cell.set_wrap_mode(gtk::pango::WrapMode::WordChar);
                cell.set_wrap_width(240);
                model.connect_size_allocate(move |_, allocation| {
                    let width = (allocation.width() - 64).clamp(80, 560);
                    if cell.wrap_width() != width {
                        cell.set_wrap_width(width);
                    }
                });
            }
        }
        let remove = gtk::CheckButton::with_label("Remove this installed model (destructive)");
        remove.set_size_request(-1, 48);
        if let Some(child) = remove.child().and_then(|w| w.downcast::<gtk::Label>().ok()) {
            child.set_line_wrap(true);
            child.set_line_wrap_mode(gtk::pango::WrapMode::Word);
            child.set_max_width_chars(28);
        }
        let model_facts = label("");
        model_page.pack_start(&section("Local model"), false, false, 0);
        model_page.pack_start(&model, false, false, 0);
        model_page.pack_start(&model_facts, false, false, 0);
        model_page.pack_start(&remove, false, false, 0);
        model_page.pack_start(&label("Download, selection changes, testing and removal each need separate approval. CPU performance is unmeasured."), false, false, 0);
        stack.add_named(&model_page, "model");

        let providers_page = gtk::Box::new(gtk::Orientation::Vertical, 12);
        providers_page.style_context().add_class("panel");
        let provider_choice = gtk::ComboBoxText::new();
        for provider in Provider::ALL {
            provider_choice.append(Some(provider.id()), provider.label());
        }
        provider_choice.set_active(Some(0));
        let provider_action = gtk::ComboBoxText::new();
        for (id, title) in [
            ("save", "Save / replace API key"),
            ("validate", "Validate model (API request)"),
            ("probe", "Observe local keyring (no API)"),
            ("refresh", "Refresh cached status (no keyring/API)"),
            ("observe", "Observe original request (no resend)"),
        ] {
            provider_action.append(Some(id), title);
        }
        provider_action.set_active(Some(0));
        let provider_model = gtk::ComboBoxText::new();
        let provider_key = gtk::Entry::new();
        provider_key.set_visibility(false);
        provider_key.set_input_purpose(gtk::InputPurpose::Password);
        // Keep one overflow sentinel: GTK must not truncate an oversized paste
        // into an apparently valid 4096-byte credential.
        provider_key.set_max_length(4097);
        provider_key
            .set_placeholder_text(Some("New API key — never loaded from saved credentials"));
        let provider_key_label = gtk::Label::with_mnemonic("API _key");
        provider_key_label.set_xalign(0.0);
        provider_key_label.set_mnemonic_widget(Some(&provider_key));
        let provider_reveal = gtk::CheckButton::with_label("Show newly entered key");
        let provider_status = label("");
        providers_page.pack_start(&section("Optional cloud providers"), false, false, 0);
        providers_page.pack_start(&provider_status, false, false, 0);
        providers_page.pack_start(&label("API credentials/billing are separate from consumer subscriptions. Keys remain in the existing keyring after replacement and uninstall; no automatic unlock or removal."), false, false, 0);
        let provider_label = gtk::Label::with_mnemonic("_Provider");
        provider_label.set_xalign(0.0);
        provider_label.set_mnemonic_widget(Some(&provider_choice));
        let action_label = gtk::Label::with_mnemonic("Ac_tion");
        action_label.set_xalign(0.0);
        action_label.set_mnemonic_widget(Some(&provider_action));
        let provider_model_label = gtk::Label::with_mnemonic("_Validation model");
        provider_model_label.set_xalign(0.0);
        provider_model_label.set_mnemonic_widget(Some(&provider_model));
        providers_page.pack_start(&provider_label, false, false, 0);
        providers_page.pack_start(&provider_choice, false, false, 0);
        providers_page.pack_start(&action_label, false, false, 0);
        providers_page.pack_start(&provider_action, false, false, 0);
        providers_page.pack_start(&provider_model_label, false, false, 0);
        providers_page.pack_start(&provider_model, false, false, 0);
        providers_page.pack_start(&provider_key_label, false, false, 0);
        providers_page.pack_start(&provider_key, false, false, 0);
        providers_page.pack_start(&provider_reveal, false, false, 0);
        stack.add_named(&providers_page, "providers");

        let working = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let stage = label("Preparing installation…");
        let elapsed = label("");
        elapsed.style_context().add_class("metadata");
        let progress = gtk::ProgressBar::new();
        working.pack_start(&stage, false, false, 0);
        working.pack_start(&progress, false, false, 0);
        working.pack_start(&elapsed, false, false, 0);
        let confirmation = gtk::Box::new(gtk::Orientation::Vertical, 12);
        confirmation.set_no_show_all(true);
        confirmation.style_context().add_class("panel");
        let confirmation_title = section("");
        let effects = label("");
        confirmation.pack_start(&confirmation_title, false, false, 0);
        confirmation.pack_start(&label("Approve these effects only. Stop here keeps earlier changes. Expired approvals are not renewed."), false, false, 0);
        confirmation.pack_start(&effects, false, false, 0);
        working.pack_start(&confirmation, false, false, 0);
        stack.add_named(&working, "working");

        let terminal = gtk::Box::new(gtk::Orientation::Vertical, 12);
        terminal.style_context().add_class("panel");
        let results = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let outcome = label("");
        let inference = label("");
        inference.style_context().add_class("metadata");
        terminal.pack_start(&results, false, false, 0);
        terminal.pack_start(&outcome, false, false, 0);
        terminal.pack_start(&inference, false, false, 0);
        let launch_status = label("");
        launch_status.set_no_show_all(true);
        terminal.pack_start(&launch_status, false, false, 0);
        let manage = gtk::Button::with_label("Manage models");
        manage.set_no_show_all(true);
        manage.set_halign(gtk::Align::Start);
        terminal.pack_start(&manage, false, false, 0);
        let result_providers = gtk::Button::with_label("Manage providers");
        result_providers.set_no_show_all(true);
        result_providers.set_halign(gtk::Align::Start);
        terminal.pack_start(&result_providers, false, false, 0);
        stack.add_named(&terminal, "terminal");
        body.pack_start(&stack, false, false, 0);

        // Retain bounded diagnostics for report ordering and tests, but do not
        // attach them to the normal installer page. A future explicit debug
        // surface can expose them without creating below-the-fold UI here.
        let details = label("");
        scroll.add(&body);
        let content = body.clone();
        scroll.connect_size_allocate(move |_, allocation| {
            let margin = ((allocation.width() - 688) / 2).max(0);
            let gutter = if allocation.width() < 600 { 16 } else { 24 };
            if content.margin_start() != margin || content.border_width() != gutter {
                let content = content.clone();
                glib::idle_add_local_once(move || {
                    content.set_margin_start(margin);
                    content.set_margin_end(margin);
                    content.set_border_width(gutter);
                });
            }
        });
        root.pack_start(&scroll, true, true, 0);
        let footer_shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
        footer_shell.set_no_show_all(true);
        footer_shell.pack_start(
            &gtk::Separator::new(gtk::Orientation::Horizontal),
            false,
            false,
            0,
        );
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        footer.set_border_width(16);
        let refresh = gtk::Button::with_label("Refresh review");
        let secondary = gtk::Button::with_label("Cancel");
        if let Some(child) = secondary
            .child()
            .and_then(|w| w.downcast::<gtk::Label>().ok())
        {
            child.set_line_wrap(true);
            child.set_line_wrap_mode(gtk::pango::WrapMode::Word);
            child.set_max_width_chars(28);
        }
        let primary = gtk::Button::with_label("Install");
        primary.style_context().add_class("suggested-action");
        primary.set_sensitive(false);
        footer.pack_start(&refresh, false, false, 0);
        footer.pack_start(
            &gtk::Box::new(gtk::Orientation::Horizontal, 0),
            true,
            true,
            0,
        );
        footer.pack_start(&secondary, false, false, 0);
        footer.pack_start(&primary, false, false, 0);
        // Keep both actions reachable with long labels or enlarged text.
        let actions = footer.clone();
        let first = secondary.clone();
        let last = primary.clone();
        footer_shell.connect_size_allocate(move |_, allocation| {
            let needed = first.preferred_width().1 + last.preferred_width().1 + 44;
            let orientation = if allocation.width() < needed {
                gtk::Orientation::Vertical
            } else {
                gtk::Orientation::Horizontal
            };
            if actions.orientation() != orientation {
                // Changing request geometry inside allocation can lose GTK's
                // resize invalidation, leaving the taller footer clipped.
                let actions = actions.clone();
                glib::idle_add_local_once(move || actions.set_orientation(orientation));
            }
        });
        footer.show_all();
        footer_shell.children()[0].show();
        footer_shell.pack_start(&footer, false, false, 0);
        root.pack_start(&footer_shell, false, false, 0);
        window.add(&root);
        Self {
            window,
            heading,
            introduction,
            stack,
            landing,
            register,
            start,
            recipe,
            review,
            stage,
            elapsed,
            progress,
            confirmation,
            confirmation_title,
            effects,
            results,
            outcome,
            inference,
            launch_status,
            details,
            footer_shell,
            refresh,
            secondary,
            primary,
            manage,
            upgrade,
            uninstall,
            model,
            remove,
            model_facts,
            provider_choice,
            provider_action,
            provider_model,
            provider_model_label,
            provider_key,
            provider_key_label,
            provider_reveal,
            provider_status,
            manage_providers,
            result_providers,
        }
    }

    fn choices(&self) -> Choices {
        // The normal Smith recipe is fixed, not a set of user-configurable
        // lifecycle checkboxes. Exact effects still receive separate consent.
        Choices::smith_install()
    }

    fn recipe(&self) {
        self.recipe.set_text("Register service, then separately enable it at login, then separately Start it now. Model choice and service-backed check follow. Lingering is unchanged.");
    }

    fn primary_label(&self, value: &str) {
        self.primary.set_label(value);
        if let Some(label) = self
            .primary
            .child()
            .and_then(|child| child.downcast::<gtk::Label>().ok())
        {
            label.set_line_wrap(true);
            // WordChar gives the footer a one-character minimum width, which
            // can request a many-line minimum height before responsive reflow.
            label.set_line_wrap_mode(gtk::pango::WrapMode::Word);
            label.set_width_chars(-1);
            label.set_max_width_chars(28);
        }
    }
}

struct Controller {
    ui: Widgets,
    session: Option<Box<dyn SessionIo>>,
    phase: Phase,
    review_id: Option<u64>,
    review_refreshable: bool,
    pending: Option<Pending>,
    choices: Option<Choices>,
    report: Option<Report>,
    fault: Option<String>,
    started: Option<Instant>,
    log: String,
    log_full: bool,
    log_dirty: bool,
    numeric_progress: bool,
    cancel_pending: bool,
    stopped_by_user: bool,
    close_after_join: bool,
    quit: bool,
    stop_dialog: Option<gtk::MessageDialog>,
    dialog_closes_window: bool,
    bundle: Option<PathBuf>,
    action_after_review: Option<AfterReview>,
    active_scope: Scope,
    model_request: Option<u64>,
    provider_request: Option<u64>,
    provider_catalog: Option<ProviderCatalog>,
    provider_pending: bool,
    provider_confirmation: bool,
    cancellation: Box<dyn Fn() -> bool>,
    restart_blocked: bool,
    launcher: Box<dyn DesktopLauncher>,
    launch_pending: Option<LaunchResult>,
    launch_requested: bool,
    close_after_launch: bool,
}

impl Controller {
    fn new(
        ui: Widgets,
        session: Option<Box<dyn SessionIo>>,
        launcher: Box<dyn DesktopLauncher>,
    ) -> Self {
        let controller = Self {
            ui,
            session,
            phase: Phase::Landing,
            review_id: None,
            review_refreshable: false,
            pending: None,
            choices: None,
            report: None,
            fault: None,
            started: None,
            log: String::new(),
            log_full: false,
            log_dirty: false,
            numeric_progress: false,
            cancel_pending: false,
            stopped_by_user: false,
            close_after_join: false,
            quit: false,
            stop_dialog: None,
            dialog_closes_window: false,
            bundle: None,
            action_after_review: None,
            active_scope: Scope::Installation,
            model_request: None,
            provider_request: None,
            provider_catalog: None,
            provider_pending: false,
            provider_confirmation: false,
            cancellation: Box::new(consent::cancelled),
            restart_blocked: false,
            launcher,
            launch_pending: None,
            launch_requested: false,
            close_after_launch: false,
        };
        controller.render();
        controller
    }

    fn log(&mut self, message: &str) {
        if self.log_full {
            return;
        }
        let message = text(message, TEXT_LIMIT);
        if self.log.len() + message.len() + 1 > LOG_LIMIT {
            self.log
                .push_str("\n[Further progress omitted; see final step results.]\n");
            self.log_full = true;
        } else {
            self.log.push_str(&message);
            self.log.push('\n');
        }
        self.log_dirty = true;
    }

    fn flush_log(&mut self) {
        if self.log_dirty && self.phase != Phase::Terminal {
            self.ui.details.set_text(&self.log);
            self.log_dirty = false;
        }
    }

    fn fence_progress(&mut self) {
        self.numeric_progress = false;
        self.ui.progress.set_fraction(0.0);
    }

    fn send(&self, command: Command) -> Result<()> {
        self.session
            .as_ref()
            .ok_or("Installer worker is unavailable")?
            .send(command)
    }

    fn choose_install_or_upgrade(&mut self) {
        if self.phase != Phase::Landing {
            return;
        }
        if !self.review_refreshable {
            self.action_after_review = Some(AfterReview::InstallOrUpgrade);
            self.phase = Phase::Inspecting;
            self.active_scope = Scope::Installation;
            self.started = Some(Instant::now());
            self.render();
        } else if self.review_id.is_some() {
            self.phase = Phase::Review;
            self.ui.heading.set_text("Install Smith");
            self.ui.introduction.set_text("Install the app and service below. Registration, login startup and Start are approved separately.");
            self.render();
        } else {
            self.independent_action(Command::Upgrade);
        }
    }

    fn choose_uninstall(&mut self) {
        if self.phase != Phase::Landing {
            return;
        }
        if self.review_refreshable {
            self.independent_action(Command::Uninstall);
        } else {
            self.action_after_review = Some(AfterReview::Command(Command::Uninstall));
            self.phase = Phase::Inspecting;
            self.active_scope = Scope::Uninstall;
            self.started = Some(Instant::now());
            self.render();
        }
    }

    fn finish_inspection(&mut self, action: AfterReview) {
        debug_assert_eq!(self.phase, Phase::Inspecting);
        self.phase = Phase::Landing;
        match action {
            AfterReview::InstallOrUpgrade => self.choose_install_or_upgrade(),
            AfterReview::Command(command) => self.independent_action(command),
        }
    }

    fn install(&mut self) {
        if self.phase != Phase::Review {
            return;
        }
        let Some(request_id) = self.review_id.take() else {
            return;
        };
        let choices = self.ui.choices();
        self.choices = Some(choices);
        self.active_scope = Scope::Installation;
        self.started = Some(Instant::now());
        self.phase = Phase::Working;
        self.ui.stage.set_text("Installing the reviewed artifacts…");
        self.render();
        if let Err(error) = self.send(Command::Install {
            request_id,
            choices,
        }) {
            self.fail(&format!("Cannot submit Install: {error}"));
        }
    }

    fn management_blocked(&self) -> bool {
        self.restart_blocked || (self.cancellation)()
    }
    fn manage(&mut self) {
        self.independent_action(Command::ManageModels);
    }
    fn independent_action(&mut self, command: Command) {
        if self.management_blocked() || self.launch_pending.is_some() || self.quit {
            self.render();
            return;
        }
        if self.phase == Phase::Terminal && self.session.is_none() {
            let Some(bundle) = self.bundle.clone() else {
                return;
            };
            match Session::spawn(bundle) {
                Ok(session) => {
                    self.session = Some(Box::new(session));
                    self.launch_requested = false;
                    self.close_after_launch = false;
                    self.ui.launch_status.set_text("");
                    self.report = None;
                    self.fault = None;
                    self.pending = None;
                    self.stopped_by_user = false;
                    self.close_after_join = false;
                    self.active_scope = match &command {
                        Command::Upgrade => Scope::Upgrade,
                        Command::Uninstall => Scope::Uninstall,
                        Command::ManageProviders => Scope::Providers,
                        _ => Scope::ModelCheck,
                    };
                    self.phase = Phase::Inspecting;
                    self.action_after_review = Some(AfterReview::Command(command));
                    self.review_id = None;
                    self.review_refreshable = false;
                    self.started = Some(Instant::now());
                    self.render();
                }
                Err(error) => self.fail(&error),
            }
        } else if matches!(self.phase, Phase::Landing | Phase::Review) && self.review_refreshable {
            self.review_id = None;
            self.active_scope = match command {
                Command::Upgrade => Scope::Upgrade,
                Command::Uninstall => Scope::Uninstall,
                Command::ManageProviders => Scope::Providers,
                _ => Scope::ModelCheck,
            };
            self.choices = Some(Choices::default());
            self.phase = Phase::Working;
            self.started = Some(Instant::now());
            self.render();
            if let Err(error) = self.send(command) {
                self.fail(&error);
            }
        }
    }
    fn model_selection_changed(&self) {
        self.ui.primary.set_sensitive(
            self.phase == Phase::Model
                && self.model_request.is_some()
                && self.ui.model.active_id().is_some(),
        );
    }

    fn model_action(&mut self) {
        let Some(model) = self.ui.model.active_id() else {
            return;
        };
        let Some(request_id) = self.model_request.take() else {
            return;
        };
        let remove = self.ui.remove.is_active();
        self.phase = Phase::Working;
        self.render();
        if let Err(error) = self.send(Command::ModelAction {
            request_id,
            model: model.to_string(),
            remove,
        }) {
            self.fail(&error);
        }
    }
    fn selected_provider(&self) -> Provider {
        if self.ui.provider_choice.active_id().as_deref() == Some("anthropic") {
            Provider::Anthropic
        } else {
            Provider::Openai
        }
    }
    fn provider_selected(&self) {
        self.ui.provider_key.set_text("");
        self.ui.provider_reveal.set_active(false);
        self.ui.provider_model.remove_all();
        if let Some(catalog) = &self.provider_catalog {
            for choice in catalog.choices(self.selected_provider()) {
                self.ui.provider_model.append(
                    Some(&choice.route.model),
                    &format!("{} ({})", choice.label, choice.route.model),
                );
            }
            self.ui.provider_model.set_active(Some(0));
        }
        self.provider_controls();
    }
    fn provider_controls(&self) {
        if self.phase != Phase::Providers {
            return;
        }
        let action = self.ui.provider_action.active_id();
        let save = action.as_deref() == Some("save");
        let validate = action.as_deref() == Some("validate");
        self.ui.primary_label(match action.as_deref() {
            Some("save") => "Review Save / Replace",
            Some("validate") => "Review validation",
            Some("probe") => "Observe local keyring",
            Some("refresh") => "Refresh cached status",
            Some("observe") => "Observe original request",
            _ => "Review provider action",
        });
        self.ui.provider_key.set_visible(save);
        self.ui.provider_key_label.set_visible(save);
        self.ui.provider_reveal.set_visible(save);
        self.ui.provider_model.set_visible(validate);
        self.ui.provider_model_label.set_visible(validate);
        let eligible = match action.as_deref() {
            Some("save") => !self.provider_pending && !self.ui.provider_key.text().is_empty(),
            Some("validate") => {
                !self.provider_pending
                    && self.ui.provider_model.active_id().is_some()
                    && self.provider_catalog.as_ref().is_some_and(|c| {
                        c.provider(self.selected_provider())
                            .credential_generation
                            .is_some()
                    })
            }
            Some("probe") => !self.provider_pending,
            Some("refresh") => true,
            Some("observe") => self.provider_pending,
            _ => false,
        };
        self.ui
            .primary
            .set_sensitive(self.provider_request.is_some() && eligible);
    }
    fn provider_action(&mut self, done: bool) {
        if self.phase != Phase::Providers {
            return;
        }
        let Some(request_id) = self.provider_request else {
            return;
        };
        let provider = self.selected_provider();
        let action = if done {
            ProviderAction::Done
        } else {
            match self.ui.provider_action.active_id().as_deref() {
                Some("save") => {
                    let secret = Secret::new(self.ui.provider_key.text().to_string());
                    self.ui.provider_key.set_text("");
                    self.ui.provider_reveal.set_active(false);
                    match secret {
                        Ok(secret) => ProviderAction::Save { provider, secret },
                        Err(error) => {
                            self.ui.provider_status.set_text(&error);
                            self.provider_controls();
                            return;
                        }
                    }
                }
                Some("validate") => {
                    let Some(model) = self.ui.provider_model.active_id() else {
                        return;
                    };
                    ProviderAction::Validate {
                        provider,
                        model: model.to_string(),
                    }
                }
                Some("probe") => ProviderAction::Probe { provider },
                Some("refresh") => ProviderAction::Refresh,
                Some("observe") => ProviderAction::Observe,
                _ => return,
            }
        };
        self.ui.provider_key.set_text("");
        self.ui.provider_reveal.set_active(false);
        self.provider_request = None;
        self.phase = Phase::Working;
        self.render();
        if let Err(error) = self.send(Command::ProviderAction { request_id, action }) {
            self.fail(&error);
        }
    }
    fn refresh(&mut self) {
        if self.phase != Phase::Review || !self.review_refreshable {
            return;
        }
        self.review_id = None;
        self.review_refreshable = false;
        self.ui
            .review
            .set_text("Refreshing the read-only artifact review…");
        self.render();
        if let Err(error) = self.send(Command::RefreshReview) {
            self.fail(&format!("Cannot refresh review: {error}"));
        }
    }

    fn approve(&mut self) {
        if self.phase != Phase::Confirmation {
            return;
        }
        let Some(pending) = self.pending.take() else {
            return;
        };
        self.phase = Phase::Working;
        self.ui
            .stage
            .set_text("Applying the separately confirmed effect…");
        self.render();
        if let Err(error) = self.send(Command::Approve {
            request_id: pending.request_id,
        }) {
            self.fail(&format!("Cannot submit confirmation: {error}"));
        }
    }

    fn decline(&mut self) {
        if self.phase != Phase::Confirmation {
            return;
        }
        let Some(pending) = self.pending.take() else {
            return;
        };
        if self.provider_confirmation {
            self.provider_confirmation = false;
            self.phase = Phase::Working;
        } else {
            self.stopped_by_user = true;
            self.phase = Phase::Stopping;
        }
        self.render();
        if let Err(error) = self.send(Command::Decline {
            request_id: pending.request_id,
        }) {
            self.fail(&format!("Cannot submit Stop here: {error}"));
        }
    }

    fn dismiss_stop_dialog(&mut self) {
        self.dialog_closes_window = false;
        if let Some(dialog) = self.stop_dialog.take() {
            dialog.hide();
            // SAFETY: this is our inactive dialog, on the GTK main thread.
            unsafe { dialog.destroy() };
        }
    }

    fn stop(&mut self, close_after_join: bool) {
        self.ui.provider_key.set_text("");
        self.ui.provider_reveal.set_active(false);
        self.dismiss_stop_dialog();
        self.close_after_join |= close_after_join;
        self.stopped_by_user = true;
        self.review_id = None;
        self.pending = None;
        if self.session.is_none() {
            self.quit |= self.close_after_join;
            return;
        }
        self.phase = Phase::Stopping;
        self.cancel_pending = true;
        self.cancel();
        self.render();
    }

    fn cancel(&mut self) {
        if self.cancel_pending {
            // Only cancellation may be retried on bounded-channel backpressure.
            // Session::send(Cancel) also requests the composed installer/Core flag.
            if self.send(Command::Cancel).is_ok() {
                self.cancel_pending = false;
            }
        }
    }

    fn fail(&mut self, error: &str) {
        self.ui.provider_key.set_text("");
        self.ui.provider_reveal.set_active(false);
        self.dismiss_stop_dialog();
        if self.fault.is_none() {
            self.fault = Some(text(error, TEXT_LIMIT));
            self.log(error);
            self.flush_log();
        }
        self.review_id = None;
        self.pending = None;
        if self.session.is_some() {
            self.phase = Phase::Stopping;
            self.cancel_pending = true;
            self.cancel();
            self.render();
        } else {
            self.finish();
        }
    }

    fn event(&mut self, event: Event) {
        match event {
            Event::Providers {
                request_id,
                catalog,
                message,
                pending_request,
            } => {
                self.fence_progress();
                if self.phase == Phase::Stopping {
                    return;
                }
                if self.phase != Phase::Working {
                    self.fail("Providers arrived outside active setup");
                    return;
                }
                self.provider_catalog = Some(catalog);
                self.provider_request = Some(request_id);
                self.provider_pending = pending_request.is_some();
                self.provider_confirmation = false;
                let pending = pending_request.map(|id| format!("\nOriginal uncertain request: {id}. Observe only; never repeat its apply.")).unwrap_or_default();
                self.ui.provider_status.set_text(&format!(
                    "{message}\n\n{}{pending}",
                    self.provider_catalog.as_ref().expect("catalog").summary()
                ));
                self.phase = Phase::Providers;
                if self.provider_pending {
                    self.ui.provider_action.set_active_id(Some("observe"));
                }
                self.provider_selected();
                self.render();
            }
            Event::ProviderConfirmation {
                request_id,
                title,
                effects,
            } => {
                self.event(Event::Confirmation {
                    request_id,
                    title,
                    effects,
                });
                self.provider_confirmation = self.phase == Phase::Confirmation;
                self.render();
            }
            Event::Review {
                request_id,
                description,
            } => {
                if self.phase == Phase::Stopping {
                    return;
                }
                if !matches!(
                    self.phase,
                    Phase::Landing | Phase::Inspecting | Phase::Review
                ) || !effect_text(&description, EFFECT_TEXT_LIMIT)
                {
                    self.fail("The exact artifact review could not be displayed safely, or arrived outside review. No new installation was approved.");
                    return;
                }
                self.review_id = Some(request_id);
                self.review_refreshable = true;
                self.ui.review.set_text(&description);
                self.log("Read-only artifact review ready. Install has not been approved.");
                self.render();
                if let Some(action) = self.action_after_review.take() {
                    self.finish_inspection(action);
                }
            }
            Event::Models {
                request_id,
                selected,
                installed,
            } => {
                self.fence_progress();
                if self.phase == Phase::Stopping {
                    return;
                }
                if self.phase != Phase::Working {
                    self.fail("Model choice arrived outside active setup");
                    return;
                }
                self.ui.model.remove_all();
                for entry in smith_service::runner::CATALOG {
                    let state = if installed.iter().any(|m| m == entry.name) {
                        "installed"
                    } else {
                        "download required"
                    };
                    let default = if entry.name == smith_service::runner::DEFAULT_MODEL {
                        " · default"
                    } else {
                        ""
                    };
                    self.ui.model.append(
                        Some(entry.name),
                        &format!(
                            "{} · {} · {:.2} GiB · {state}{default}",
                            entry.tier.label(),
                            entry.display_name,
                            entry.size as f64 / 1024_f64.powi(3),
                        ),
                    );
                }
                if let Some(name) = selected.as_deref() {
                    if !self.ui.model.set_active_id(Some(name)) {
                        self.ui.model.set_active_id(None);
                    }
                } else {
                    self.ui
                        .model
                        .set_active_id(Some(smith_service::runner::DEFAULT_MODEL));
                }
                self.ui.remove.set_active(false);
                let saved = selected.as_deref().unwrap_or("none (fresh setup)");
                let supported = selected.as_deref().is_none_or(|name| {
                    smith_service::runner::CATALOG
                        .iter()
                        .any(|entry| entry.name == name)
                });
                let facts = if supported {
                    format!("Saved selection: {saved}. Removal of selected or loaded models is refused; no implicit stop.")
                } else {
                    format!("Unsupported saved selection: {saved}. It is preserved but cannot be used here. Choose a curated model to continue.")
                };
                self.ui.model_facts.set_text(&facts);
                self.model_request = Some(request_id);
                self.phase = Phase::Model;
                self.render();
            }
            Event::Stage { message } => {
                self.fence_progress();
                self.log(&message);
                if matches!(
                    self.phase,
                    Phase::Landing | Phase::Inspecting | Phase::Review
                ) {
                    // Review-stage messages include preview failure and expiry.
                    // They never authorize continued use of an earlier snapshot.
                    self.review_id = None;
                    self.review_refreshable = true;
                    self.ui.review.set_text(&text(&message, TEXT_LIMIT));
                    if self.phase == Phase::Review {
                        self.ui.introduction.set_text(&text(&format!("Install unavailable: {message}\nUse the independently checked Upgrade flow instead; no effect starts automatically."), TEXT_LIMIT));
                    }
                    self.render();
                    if let Some(action) = self.action_after_review.take() {
                        self.finish_inspection(action);
                    }
                } else if self.phase == Phase::Working {
                    if message.starts_with("Checking model") {
                        self.ui.heading.set_text("Check model");
                    } else if message.starts_with("Download") {
                        self.ui.heading.set_text("Download model");
                    }
                    self.ui.stage.set_text(&text(&message, TEXT_LIMIT));
                }
            }
            Event::Progress { downloaded, total } => {
                if self.phase == Phase::Working && total > 0 && downloaded <= total {
                    self.numeric_progress = true;
                    self.ui
                        .progress
                        .set_fraction(downloaded as f64 / total as f64);
                    self.ui.stage.set_text(&format!(
                        "Downloaded {downloaded} / {total} bytes ({:.1}%)",
                        downloaded as f64 * 100.0 / total as f64
                    ));
                }
            }
            Event::Confirmation {
                request_id,
                title,
                effects,
            } => {
                self.provider_confirmation = false;
                self.fence_progress();
                if self.phase == Phase::Stopping {
                    return;
                }
                let valid = self.phase == Phase::Working
                    && effect_text(&title, 256)
                    && !title.contains(['\n', '\t'])
                    && !effects.is_empty()
                    && effects.len() <= 32
                    && effects.iter().all(|s| effect_text(s, EFFECT_TEXT_LIMIT))
                    && effects.iter().map(|s| s.len() + 2).sum::<usize>() <= EFFECT_TEXT_LIMIT;
                if !valid {
                    self.fail("The exact confirmation effects could not be displayed safely, or arrived without an active installation. No new effect was approved.");
                    return;
                }
                self.ui.confirmation_title.set_text(&title);
                self.ui.effects.set_text(&effects.join("\n\n"));
                self.pending = Some(Pending { request_id, title });
                self.phase = Phase::Confirmation;
                self.ui
                    .stage
                    .set_text("Waiting for your separate confirmation.");
                self.render();
                self.ui.effects.grab_focus();
            }
            Event::Finished { report } => {
                self.fence_progress();
                self.active_scope = report.scope;
                self.pending = None;
                self.review_id = None;
                self.report = Some(report);
                if self.phase != Phase::Stopping {
                    self.phase = Phase::Finishing;
                }
                self.render();
            }
        }
    }

    // A tick never waits for a message or joins a running worker. Drain a bounded
    // batch before interpreting worker exit, so queued final evidence is retained.
    fn tick(&mut self, interrupted: bool) -> bool {
        self.poll_launch();
        self.restart_blocked |= interrupted || (self.cancellation)();
        if self.phase == Phase::Terminal && self.management_blocked() {
            self.render();
        }
        if interrupted && !matches!(self.phase, Phase::Stopping | Phase::Terminal) {
            self.stop(false);
        }
        self.cancel();
        let finished_at_poll_start = self.session.as_ref().is_some_and(|s| s.is_finished());
        let mut drained = self.report.is_some();
        if self.report.is_none() && self.session.is_some() {
            for _ in 0..EVENTS_PER_TICK {
                let result = self.session.as_ref().expect("owned worker").try_event();
                match result {
                    Ok(Some(event)) => {
                        self.event(event);
                        if self.report.is_some() {
                            drained = true;
                            break;
                        }
                    }
                    Ok(None) => {
                        drained = true;
                        break;
                    }
                    Err(error) => {
                        drained = true;
                        self.fail(&format!("Installer event channel failed: {error}"));
                        break;
                    }
                }
            }
        }
        // Exit can race an earlier empty poll. Without a report, defer joining
        // until a batch has been polled AFTER observing that the worker exited.
        if drained
            && (self.report.is_some() || finished_at_poll_start)
            && self.session.as_ref().is_some_and(|s| s.is_finished())
        {
            let result = self.session.as_mut().expect("finished owned worker").join();
            self.session = None;
            self.cancel_pending = false;
            if let Err(error) = result {
                self.fault = Some(text(
                    &format!("Installer worker exit failed: {error}"),
                    TEXT_LIMIT,
                ));
            }
            if self.report.is_none() && self.fault.is_none() {
                self.fault = Some("Installer worker exited without a terminal report. Accepted effects are unverified; inspect before repair or retry.".into());
            }
            self.finish();
        }
        self.flush_log();
        if !self.numeric_progress
            && matches!(
                self.phase,
                Phase::Inspecting | Phase::Working | Phase::Stopping | Phase::Finishing
            )
        {
            self.ui.progress.pulse();
        }
        if let Some(started) = self.started {
            let seconds = started.elapsed().as_secs();
            self.ui.elapsed.set_text(&format!(
                "Elapsed: {}m {}s — no estimated completion percentage",
                seconds / 60,
                seconds % 60
            ));
        }
        self.quit
    }

    fn finish(&mut self) {
        self.ui.provider_key.set_text("");
        self.ui.provider_reveal.set_active(false);
        // Terminal/Close is reachable only after the owned worker has been joined.
        debug_assert!(self.session.is_none());
        self.phase = Phase::Terminal;
        self.dismiss_stop_dialog();
        for child in self.ui.results.children() {
            self.ui.results.remove(&child);
        }
        let complete = self.complete();
        let stopped = self.fault.is_none()
            && !complete
            && (self.stopped_by_user || self.report.as_ref().is_some_and(|r| r.cancelled));
        self.ui.heading.set_text(if complete {
            self.report
                .as_ref()
                .expect("verified report")
                .scope
                .completion_heading()
        } else if stopped {
            match self.active_scope {
                Scope::Upgrade => "Upgrade cancelled",
                Scope::Uninstall => "Uninstall cancelled",
                Scope::ModelCheck | Scope::ModelRemoval => "Model operation cancelled",
                Scope::Providers => "Provider operation cancelled",
                Scope::Installation => "Installation cancelled",
            }
        } else {
            match self.active_scope {
                Scope::Upgrade => "Upgrade incomplete",
                Scope::Uninstall => "Uninstall incomplete",
                _ => "Setup incomplete",
            }
        });
        let message = self
            .report
            .as_ref()
            .map(|r| r.message.as_str())
            .unwrap_or("The installer did not produce a verified result.");
        self.ui.introduction.set_text(&text(message, TEXT_LIMIT));
        if let Some(report) = &self.report {
            if let Some(providers) = &report.providers {
                self.ui.results.pack_start(
                    &label(&format!(
                        "Optional Providers: {}\n{}",
                        if providers.success {
                            "Finished (see individual validation status)"
                        } else {
                            "Failed or uncertain; local setup preserved"
                        },
                        text(&providers.message, 12 * 1024)
                    )),
                    false,
                    false,
                    0,
                );
            }
            for step in report
                .steps
                .iter()
                .filter(|step| {
                    !complete || matches!(step.state, StepState::Failed | StepState::Uncertain)
                })
                .take(32)
            {
                let status = match step.state {
                    StepState::Completed => "Completed",
                    StepState::Skipped => "Skipped",
                    StepState::Failed => "Failed",
                    StepState::Uncertain => "Uncertain",
                };
                let row = label(&format!(
                    "{}: {status}\n{}",
                    text(&step.label, 256),
                    text(&step.detail, TEXT_LIMIT)
                ));
                self.ui.results.pack_start(&row, false, false, 0);
            }
            if report.steps.len() > 32 {
                self.ui.results.pack_start(
                    &label("[Additional step results omitted from this bounded display.]"),
                    false,
                    false,
                    0,
                );
            }
            let headline = text(&report.message, TEXT_LIMIT);
            let details: Vec<String> = report
                .details
                .iter()
                .take(32)
                .map(|s| text(s, TEXT_LIMIT))
                .collect();
            // Final partial-outcome evidence takes precedence over a full progress
            // log, so backpressure cannot hide the worker's retained-effect report.
            let omitted = if report.details.len() > 32 {
                "\n[Additional worker details omitted from bounded display.]"
            } else {
                ""
            };
            self.ui.details.set_text(&text(
                &format!(
                    "{headline}\n\nFinal worker details\n{}{omitted}\n\nProgress log\n{}",
                    details.join("\n"),
                    self.log
                ),
                LOG_LIMIT,
            ));
        }
        if let Some(fault) = &self.fault {
            self.ui.results.pack_start(
                &label(&format!("Result/reporting failure: {fault}")),
                false,
                false,
                0,
            );
        }
        self.ui.outcome.set_text(if complete && self.active_scope == Scope::Uninstall {
            "ALL app/Core state, preferences, history, models, releases and backups are kept. Saved API keys remain in the keyring and are not revoked. Disk usage is not purged."
        } else if complete && self.active_scope == Scope::Upgrade {
            "Models were not downloaded or tested. Keep the package for maintenance."
        } else if complete {
            "Keep the package for future maintenance."
        } else if self.choices.is_some() {
            "Completed and uncertain effects remain; stopping is not rollback. No retry or cleanup. Inspect the outcomes above before repair."
        } else {
            "Install was not authorized. No artifact or service effect was requested. Correct the problem before reopening."
        });
        let checked = complete
            && self.report.as_ref().is_some_and(|r| {
                r.steps.iter().any(|s| {
                    s.label == "Model setup"
                        && s.state == StepState::Completed
                        && s.detail.contains("inference verified")
                })
            });
        self.ui.inference.set_text(if checked { "Inference: Verified through Smith service.\nThe Smith app is not opened automatically." } else if complete && matches!(self.active_scope, Scope::Uninstall | Scope::ModelRemoval) { "" } else { "Model inference not verified by this session.\nThe Smith app is not opened automatically." });
        self.ui
            .inference
            .set_visible(!self.ui.inference.text().is_empty());
        self.ui.results.show_all();
        self.quit |= self.close_after_join;
        self.render();
    }

    fn launch_eligible(&self) -> bool {
        self.phase == Phase::Terminal
            && self.session.is_none()
            && self.complete()
            && !self.stopped_by_user
            && !self.management_blocked()
            && self
                .report
                .as_ref()
                .is_some_and(|report| matches!(report.scope, Scope::Installation | Scope::Upgrade))
    }

    fn launch(&mut self) {
        if !self.launch_eligible()
            || self.launch_pending.is_some()
            || self.launch_requested
            || self.quit
        {
            return;
        }
        // Reserve this completion context before returning to GTK. The launcher
        // only starts a background task; validation never runs in this callback.
        self.launch_requested = true;
        match self.launcher.start() {
            Ok(receiver) => {
                self.launch_pending = Some(receiver);
                self.ui
                    .launch_status
                    .set_text("Checking installed Smith before launch…");
            }
            Err(error) => self.launch_finished(Err(error)),
        }
        self.render();
    }

    fn launch_finished(&mut self, result: Result<()>) {
        self.launch_pending = None;
        self.launch_requested = result.is_ok();
        self.ui.launch_status.set_text(&match result {
            Ok(()) => "Launch requested. You can close the installer; Smith stays open independently.".into(),
            Err(error) => text(&format!("Could not launch Smith: {error}\nInstallation result is unchanged. Retry explicitly or close the installer."), TEXT_LIMIT),
        });
        self.quit |= self.close_after_launch;
        self.render();
    }

    fn poll_launch(&mut self) {
        let result = self
            .launch_pending
            .as_ref()
            .map(|receiver| receiver.try_recv());
        match result {
            Some(Ok(result)) => self.launch_finished(result),
            Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                self.launch_finished(Err("Launch worker ended without a dispatch result.".into()));
            }
            _ => {}
        }
    }

    fn close_terminal(&mut self) {
        if self.launch_pending.is_some() {
            self.close_after_launch = true;
            self.ui
                .launch_status
                .set_text("Waiting for the launch request before closing…");
            self.render();
        } else {
            self.quit = true;
        }
    }

    fn complete(&self) -> bool {
        self.choices.is_some()
            && self.fault.is_none()
            && self.report.as_ref().is_some_and(verified_complete)
    }

    fn exit_code(&self) -> i32 {
        // No Install consent is not success evidence; an initial stop needs a
        // terminal report confirming that every step was skipped without error.
        let initial_stop = self.choices.is_none()
            && self.fault.is_none()
            && self.report.as_ref().is_some_and(|report| {
                (self.stopped_by_user || report.cancelled)
                    && !report.complete
                    && !report.steps.is_empty()
                    && report
                        .steps
                        .iter()
                        .all(|step| matches!(step.state, StepState::Skipped))
            });
        if self.complete() || initial_stop {
            0
        } else {
            1
        }
    }

    fn render(&self) {
        let landing = self.phase == Phase::Landing;
        let review = self.phase == Phase::Review;
        let terminal = self.phase == Phase::Terminal;
        if terminal && self.management_blocked() {
            self.ui.outcome.set_text(REOPEN_GUIDANCE);
        }
        let confirming = self.phase == Phase::Confirmation;
        self.ui.stage.set_visible(!confirming);
        self.ui.progress.set_visible(!confirming);
        self.ui.elapsed.set_visible(!confirming);
        self.ui.landing.set_visible(landing);
        self.ui.register.set_visible(false);
        self.ui.start.set_visible(false);
        self.ui.recipe.set_visible(false);
        self.ui.register.set_sensitive(false);
        self.ui.start.set_sensitive(false);
        self.ui.upgrade.set_visible(landing);
        self.ui.uninstall.set_visible(landing);
        self.ui
            .upgrade
            .set_sensitive(landing && !self.management_blocked());
        self.ui
            .uninstall
            .set_sensitive(landing && !self.management_blocked());
        self.ui.manage.set_visible(false);
        self.ui.manage.set_sensitive(false);
        let management = !self.management_blocked() && self.launch_pending.is_none() && !self.quit;
        self.ui.manage_providers.set_visible(review);
        self.ui
            .manage_providers
            .set_sensitive(review && self.review_refreshable && management);
        self.ui.result_providers.set_visible(
            terminal
                && self.bundle.is_some()
                && self.session.is_none()
                && !self.management_blocked(),
        );
        self.ui
            .result_providers
            .set_sensitive(management && self.session.is_none());
        self.ui
            .launch_status
            .set_visible(terminal && !self.ui.launch_status.text().is_empty());
        self.ui.refresh.set_visible(false);
        self.ui.footer_shell.set_visible(!landing);
        self.ui.secondary.set_visible(!landing);
        self.ui.secondary.set_sensitive(matches!(
            self.phase,
            Phase::Review | Phase::Model | Phase::Providers | Phase::Working | Phase::Confirmation
        ));
        self.ui
            .confirmation
            .set_visible(self.phase == Phase::Confirmation);
        if self.phase == Phase::Confirmation {
            for child in self.ui.confirmation.children() {
                child.show_all();
            }
        }
        self.ui.stack.set_visible_child_name(if landing {
            "landing"
        } else if self.phase == Phase::Model {
            "model"
        } else if self.phase == Phase::Providers {
            "providers"
        } else if review {
            "review"
        } else if terminal {
            "terminal"
        } else {
            "working"
        });
        match self.phase {
            Phase::Landing => {
                self.ui.heading.set_text("Set up Smith");
                self.ui
                    .introduction
                    .set_text("Install or update Smith. Uninstall keeps your data.");
                self.ui.primary.set_visible(false);
                self.ui.secondary.set_visible(false);
            }
            Phase::Inspecting => {
                self.ui.heading.set_text("Checking Smith");
                self.ui
                    .introduction
                    .set_text(if self.active_scope == Scope::Uninstall {
                        "Checking the installed state. Nothing changes yet."
                    } else {
                        "Checking the package and installed state. Nothing changes yet."
                    });
                self.ui
                    .stage
                    .set_text(if self.active_scope == Scope::Uninstall {
                        "Preparing uninstall…"
                    } else {
                        "Checking Smith…"
                    });
                self.ui.primary.set_visible(false);
                self.ui.secondary.set_visible(true);
                self.ui.secondary.set_label("Cancel");
                self.ui.secondary.set_sensitive(true);
            }
            Phase::Review => {
                self.ui.primary.set_visible(true);
                self.ui.secondary.set_visible(true);
                self.ui.primary_label("Install");
                self.ui.primary.set_sensitive(self.review_id.is_some());
                self.ui.secondary.set_label("Manage models");
                self.ui
                    .secondary
                    .set_sensitive(!self.management_blocked() && self.review_refreshable);
            }
            Phase::Model => {
                self.ui.primary.set_visible(true);
                self.ui.heading.set_text("Model choice");
                self.ui
                    .introduction
                    .set_text("Choose a model to review. Selection alone changes nothing.");
                self.ui.primary_label("Review model");
                self.ui.primary.set_sensitive(
                    self.model_request.is_some() && self.ui.model.active_id().is_some(),
                );
                self.ui.secondary.set_label("Cancel model operation…");
            }
            Phase::Providers => {
                self.ui.heading.set_text("Providers");
                self.ui.introduction.set_text("Optional additions after local Model setup. Configure OpenAI and Anthropic independently. Nothing is sent by selecting or typing.");
                self.ui.primary.set_visible(true);
                self.ui.primary_label("Review provider action");
                self.ui.secondary.set_label("Done / Skip Providers");
                self.provider_controls();
            }
            Phase::Working => {
                self.ui.primary.set_visible(false);
                self.ui.heading.set_text(match self.active_scope {
                    Scope::Upgrade => "Upgrading Smith",
                    Scope::Uninstall => "Uninstalling Smith",
                    Scope::ModelCheck | Scope::ModelRemoval => "Managing models",
                    Scope::Providers => "Managing providers",
                    Scope::Installation => "Installing Smith",
                });
                self.ui
                    .introduction
                    .set_text("Working on this step. Cancelling does not undo earlier changes.");
                self.ui.primary_label("Installing…");
                self.ui.primary.set_sensitive(false);
                self.ui.secondary.set_label("Cancel operation…");
            }
            Phase::Confirmation => {
                self.ui
                    .introduction
                    .set_text("Review the next action before approving it.");
                self.ui.primary.set_visible(true);
                if let Some(pending) = &self.pending {
                    self.ui.primary_label(&pending.title);
                }
                self.ui.primary.set_sensitive(self.pending.is_some());
                self.ui.secondary.set_label(if self.provider_confirmation {
                    "Not now"
                } else {
                    "Stop here"
                });
            }
            Phase::Stopping | Phase::Finishing => {
                self.ui.primary.set_visible(false);
                let stopping = self.phase == Phase::Stopping;
                self.ui.heading.set_text(if stopping {
                    "Cancelling operation…"
                } else {
                    "Finishing operation…"
                });
                self.ui.stage.set_text(if stopping {
                    "Waiting for the installer to stop safely. Earlier changes remain; writes may take time."
                } else {
                    "Waiting for the installer to finish safely."
                });
                self.ui.primary_label(if stopping {
                    "Stopping…"
                } else {
                    "Finishing…"
                });
                self.ui.primary.set_sensitive(false);
            }
            Phase::Terminal => {
                self.ui.primary.set_visible(true);
                self.ui.primary_label("Close");
                self.ui
                    .primary
                    .set_sensitive(self.session.is_none() && !self.close_after_launch);
                let manage =
                    !self.management_blocked() && self.bundle.is_some() && self.session.is_none();
                if self.launch_eligible() {
                    self.ui.secondary.set_visible(true);
                    self.ui.secondary.set_label("Launch Smith");
                    self.ui.secondary.set_sensitive(
                        !self.launch_requested && self.launch_pending.is_none() && !self.quit,
                    );
                    self.ui.manage.set_visible(manage);
                    self.ui
                        .manage
                        .set_sensitive(manage && self.launch_pending.is_none() && !self.quit);
                } else {
                    self.ui.secondary.set_visible(manage);
                    self.ui.secondary.set_label("Manage models");
                    self.ui
                        .secondary
                        .set_sensitive(manage && self.launch_pending.is_none() && !self.quit);
                }
            }
        }
    }
}

fn verified_complete(report: &Report) -> bool {
    report.complete
        && !report.cancelled
        && report
            .steps
            .iter()
            .any(|s| matches!(s.state, StepState::Completed))
        && report
            .steps
            .iter()
            .all(|s| matches!(s.state, StepState::Completed | StepState::Skipped))
}

// Never truncate or silently sanitize text that authorizes an effect. Unsafe or
// oversized consent text disables consent instead; other diagnostic text is inert.
fn effect_text(value: &str, limit: usize) -> bool {
    !value.trim().is_empty() && value.len() <= limit && value.chars().all(visible_character)
}
fn visible_character(c: char) -> bool {
    (!c.is_control() || matches!(c, '\n' | '\t'))
        && !matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}
fn text(value: &str, limit: usize) -> String {
    let mut result = String::new();
    for c in value.chars() {
        let c = if visible_character(c) { c } else { '\u{fffd}' };
        if result.len() + c.len_utf8() > limit {
            result.push_str("\n[Text omitted from bounded display.]");
            break;
        }
        result.push(c);
    }
    result
}

fn close_request(controller: &Rc<RefCell<Controller>>) {
    let phase = controller.borrow().phase;
    match phase {
        Phase::Terminal => controller.borrow_mut().close_terminal(),
        Phase::Landing | Phase::Inspecting | Phase::Review => controller.borrow_mut().stop(true),
        Phase::Stopping | Phase::Finishing => controller.borrow_mut().close_after_join = true,
        Phase::Model | Phase::Providers | Phase::Working | Phase::Confirmation => {
            ask_stop(controller, true)
        }
    }
}

fn ask_stop(controller: &Rc<RefCell<Controller>>, close_window: bool) {
    let mut state = controller.borrow_mut();
    state.dialog_closes_window |= close_window;
    if let Some(dialog) = &state.stop_dialog {
        dialog.present();
        return;
    }
    let dialog = gtk::MessageDialog::new(
        Some(&state.ui.window),
        gtk::DialogFlags::MODAL,
        gtk::MessageType::Question,
        gtk::ButtonsType::None,
        "Cancel installer operation?",
    );
    dialog.style_context().add_class("smith-stop-dialog");
    dialog.set_secondary_text(Some("Cancellation waits for the owned installer worker to exit. Accepted/uncertain effects remain; there is no rollback. This cancels the installer operation, not the Smith service or service-accepted work. Reopen the installer before another operation."));
    dialog.add_buttons(&[
        ("Cancel operation", gtk::ResponseType::Yes),
        ("Keep working", gtk::ResponseType::Cancel),
    ]);
    dialog.set_default_response(gtk::ResponseType::Cancel);
    state.stop_dialog = Some(dialog.clone());
    drop(state);
    let weak = Rc::downgrade(controller);
    dialog.connect_response(move |dialog, response| {
        if let Some(controller) = weak.upgrade() {
            let mut state = controller.borrow_mut();
            let close_window = state.dialog_closes_window;
            state.dialog_closes_window = false;
            state.stop_dialog = None;
            if response == gtk::ResponseType::Yes {
                state.stop(close_window);
            }
        }
        dialog.hide();
        // SAFETY: response has finished using this owned dialog on the GTK thread.
        unsafe { dialog.destroy() };
    });
    dialog.show_all();
}

fn connect(controller: &Rc<RefCell<Controller>>) {
    let state = controller.borrow();
    let weak = Rc::downgrade(controller);
    state.ui.primary.connect_clicked(move |_| {
        if let Some(controller) = weak.upgrade() {
            let mut state = controller.borrow_mut();
            match state.phase {
                Phase::Review => state.install(),
                Phase::Model => state.model_action(),
                Phase::Providers => state.provider_action(false),
                Phase::Confirmation => state.approve(),
                Phase::Terminal => state.close_terminal(),
                _ => {}
            }
        }
    });
    let weak = Rc::downgrade(controller);
    state.ui.secondary.connect_clicked(move |_| {
        if let Some(controller) = weak.upgrade() {
            let phase = controller.borrow().phase;
            match phase {
                Phase::Confirmation => controller.borrow_mut().decline(),
                Phase::Providers => controller.borrow_mut().provider_action(true),
                Phase::Terminal if controller.borrow().launch_eligible() => {
                    controller.borrow_mut().launch()
                }
                Phase::Review | Phase::Terminal => controller.borrow_mut().manage(),
                Phase::Inspecting => controller.borrow_mut().stop(false),
                Phase::Working | Phase::Model => ask_stop(&controller, false),
                _ => {}
            }
        }
    });
    let weak = Rc::downgrade(controller);
    state.ui.refresh.connect_clicked(move |_| {
        if let Some(controller) = weak.upgrade() {
            controller.borrow_mut().refresh();
        }
    });
    let weak = Rc::downgrade(controller);
    state.ui.manage.connect_clicked(move |_| {
        if let Some(controller) = weak.upgrade() {
            controller.borrow_mut().manage();
        }
    });
    for button in [&state.ui.manage_providers, &state.ui.result_providers] {
        let weak = Rc::downgrade(controller);
        button.connect_clicked(move |_| {
            if let Some(controller) = weak.upgrade() {
                controller
                    .borrow_mut()
                    .independent_action(Command::ManageProviders);
            }
        });
    }
    let weak = Rc::downgrade(controller);
    state.ui.provider_choice.connect_changed(move |_| {
        if let Some(controller) = weak.upgrade() {
            if let Ok(state) = controller.try_borrow() {
                state.provider_selected();
            }
        }
    });
    let weak = Rc::downgrade(controller);
    state.ui.provider_action.connect_changed(move |_| {
        if let Some(controller) = weak.upgrade() {
            if let Ok(state) = controller.try_borrow() {
                state.ui.provider_key.set_text("");
                state.ui.provider_reveal.set_active(false);
                state.provider_controls();
            }
        }
    });
    let weak = Rc::downgrade(controller);
    state.ui.provider_key.connect_changed(move |_| {
        if let Some(controller) = weak.upgrade() {
            if let Ok(state) = controller.try_borrow() {
                state.provider_controls();
            }
        }
    });
    let key = state.ui.provider_key.clone();
    state.ui.provider_reveal.connect_toggled(move |show| {
        key.set_visibility(show.is_active());
    });
    let weak = Rc::downgrade(controller);
    state.ui.upgrade.connect_clicked(move |_| {
        if let Some(controller) = weak.upgrade() {
            controller.borrow_mut().choose_install_or_upgrade();
        }
    });
    let weak = Rc::downgrade(controller);
    state.ui.uninstall.connect_clicked(move |_| {
        if let Some(controller) = weak.upgrade() {
            controller.borrow_mut().choose_uninstall();
        }
    });
    let start = state.ui.start.clone();
    let weak = Rc::downgrade(controller);
    state.ui.register.connect_toggled(move |register| {
        if !register.is_active() {
            start.set_active(false);
        }
        if let Some(controller) = weak.upgrade() {
            let state = controller.borrow();
            start.set_sensitive(state.phase == Phase::Review && register.is_active());
            state.ui.recipe();
        }
    });
    let weak = Rc::downgrade(controller);
    state.ui.start.connect_toggled(move |_| {
        if let Some(controller) = weak.upgrade() {
            controller.borrow().ui.recipe();
        }
    });
    let weak = Rc::downgrade(controller);
    state.ui.model.connect_changed(move |_| {
        if let Some(controller) = weak.upgrade() {
            if let Ok(state) = controller.try_borrow() {
                state.model_selection_changed();
            }
        }
    });
    let weak = Rc::downgrade(controller);
    state.ui.window.connect_delete_event(move |_, _| {
        if let Some(controller) = weak.upgrade() {
            close_request(&controller);
        }
        Propagation::Stop
    });
}

fn run_gui(bundle: Option<PathBuf>) -> Result<i32> {
    display_preflight(
        std::env::var_os("DISPLAY"),
        std::env::var_os("WAYLAND_DISPLAY"),
    )?;
    // Install the composed signal handler before GTK can create any threads.
    consent::install_cancellation()?;
    gtk::init().map_err(|_| "Cannot open the graphical display. Use your desktop session or the separate smith-installer CLI.")?;
    let _fonts = appearance::load_fonts()?;
    let css = gtk::CssProvider::new();
    css.load_from_data(CSS.as_bytes())
        .map_err(|_| "Cannot load installer style")?;
    let screen = gtk::gdk::Screen::default().ok_or("Graphical display has no screen")?;
    gtk::StyleContext::add_provider_for_screen(
        &screen,
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let ui = Widgets::new();
    let bundle = match bundle {
        Some(bundle) => {
            std::path::absolute(bundle).map_err(|_| "Cannot resolve explicit bundle root".into())
        }
        None => smith_installer::cli::default_bundle(),
    };
    let package = bundle.as_ref().ok().cloned();
    ui.register.set_active(true);
    ui.start.set_active(true);
    let session = bundle.and_then(Session::spawn);
    let (session, error) = match session {
        Ok(session) => (Some(Box::new(session) as Box<dyn SessionIo>), None),
        Err(error) => (None, Some(error)),
    };
    let controller = Rc::new(RefCell::new(Controller::new(
        ui,
        session,
        Box::new(NativeLauncher::from_environment()),
    )));
    controller.borrow_mut().bundle = package;
    connect(&controller);
    controller.borrow().ui.window.show_all();
    controller.borrow().render();
    if let Some(error) = error {
        controller.borrow_mut().fail(&error);
    }
    let weak = Rc::downgrade(&controller);
    let timer = glib::timeout_add_local(POLL_INTERVAL, move || {
        let Some(controller) = weak.upgrade() else {
            return ControlFlow::Break;
        };
        if controller.borrow_mut().tick(consent::cancelled()) {
            gtk::main_quit();
        }
        ControlFlow::Continue
    });
    loop {
        gtk::main();
        if controller.borrow().session.is_none() {
            if controller.borrow().launch_pending.is_some() {
                controller.borrow_mut().close_terminal();
                continue;
            }
            break;
        }
        // Even an unexpected main-loop return must not detach an effect worker.
        controller.borrow_mut().stop(true);
    }
    timer.remove();
    let state = controller.borrow();
    state.ui.window.hide();
    // SAFETY: no worker remains, callbacks use weak ownership, and GTK is on main.
    unsafe { state.ui.window.destroy() };
    Ok(state.exit_code())
}

#[cfg(test)]
mod tests {
    use super::*;
    use smith_installer::wizard::StepResult;
    use std::collections::VecDeque;

    fn args(values: &[&str]) -> Result<Launch> {
        parse(values.iter().map(OsString::from))
    }

    #[test]
    fn help_and_version_are_finite_documents_without_operational_discovery() {
        for values in [
            &["--help"][..],
            &["-h"],
            &["--version"],
            &["-V"],
            &["--bundle", "/missing-package", "--help"],
        ] {
            let mut output = Vec::new();
            let code = dispatch(
                args(values).unwrap(),
                |s| {
                    output.extend_from_slice(s.as_bytes());
                    Ok(())
                },
                |_| {
                    panic!(
                        "read-only documentation attempted GTK/bundle/provider/manager discovery"
                    )
                },
            )
            .unwrap();
            assert_eq!(code, 0);
            assert!(!output.is_empty() && output.len() < 4096);
            assert!(String::from_utf8(output)
                .unwrap()
                .contains("smith-installer-ui"));
        }
        assert!(dispatch(
            Launch::Help,
            |_| Err("closed output".into()),
            |_| panic!("GUI discovery")
        )
        .is_err());
    }

    #[test]
    fn licenses_are_embedded_and_display_free() {
        let mut output = String::new();
        assert_eq!(
            dispatch(
                args(&["--licenses"]).unwrap(),
                |value| {
                    output.push_str(value);
                    Ok(())
                },
                |_| panic!("licenses must not initialize GTK or inspect a package"),
            )
            .unwrap(),
            0
        );
        assert!(output.contains("SIL OPEN FONT LICENSE"));
        assert!(output.contains("JetBrains"));
        assert!(output.contains(include_str!("../assets/ICONS-NOTICE.txt")));
        assert!(output.len() < 16 * 1024);
        assert!(args(&["--licenses", "--licenses"]).is_err());
    }

    #[test]
    fn gui_argument_and_display_preflight_are_read_only_and_fail_closed() {
        assert_eq!(args(&[]).unwrap(), Launch::Gui { bundle: None });
        assert_eq!(
            args(&["--bundle=./package"]).unwrap(),
            Launch::Gui {
                bundle: Some(PathBuf::from("./package"))
            }
        );
        for invalid in [
            &["--yes"][..],
            &["--demo"],
            &["--model=any:tag"],
            &["--bundle"],
            &["--bundle="],
            &["--bundle", "--help"],
            &["--help=1"],
            &["--help", "-h"],
            &["--bundle=a", "--bundle=b"],
        ] {
            assert!(args(invalid).is_err(), "accepted {invalid:?}");
        }
        assert!(display_preflight(None, None).is_err());
        assert!(display_preflight(Some(OsString::new()), None).is_err());
        assert!(display_preflight(Some(OsString::from(":99")), None).is_ok());
        assert!(display_preflight(None, Some(OsString::from("wayland-test"))).is_ok());
    }

    #[test]
    fn exact_consent_text_is_not_truncated_or_silently_sanitized() {
        assert!(effect_text(
            "Write <literal> file\nThen reload",
            EFFECT_TEXT_LIMIT
        ));
        for invalid in [
            "",
            " \n",
            "hidden\0effect",
            "wrong\rline",
            "a\u{202e}b",
            "a\u{2066}b",
        ] {
            assert!(!effect_text(invalid, EFFECT_TEXT_LIMIT));
        }
        assert!(!effect_text(
            &"a".repeat(EFFECT_TEXT_LIMIT + 1),
            EFFECT_TEXT_LIMIT
        ));
        assert!(text(&"é".repeat(10), 5).starts_with("éé\n[Text omitted"));
        assert!(!text("a\0b", TEXT_LIMIT).contains('\0'));
    }

    fn report(complete: bool, cancelled: bool, state: StepState) -> Report {
        Report {
            scope: smith_installer::wizard::Scope::Installation,
            providers: None,
            complete,
            cancelled,
            message: "Selected effects reported by the worker.".into(),
            steps: vec![
                StepResult {
                    label: "Smith artifacts".into(),
                    state,
                    detail: "Preserved artifact outcome.".into(),
                },
                StepResult {
                    label: "Register service".into(),
                    state: StepState::Skipped,
                    detail: "Not selected.".into(),
                },
                StepResult {
                    label: "Start now".into(),
                    state: StepState::Skipped,
                    detail: "Not selected; existing runtime untouched.".into(),
                },
            ],
            details: vec!["Final retained worker evidence.".into()],
        }
    }

    #[test]
    fn completion_requires_verified_report_not_worker_exit_or_inference() {
        assert!(verified_complete(&report(
            true,
            false,
            StepState::Completed
        )));
        assert!(!verified_complete(&report(
            false,
            false,
            StepState::Completed
        )));
        assert!(!verified_complete(&report(
            true,
            true,
            StepState::Completed
        )));
        assert!(!verified_complete(&report(
            true,
            false,
            StepState::Uncertain
        )));
        assert!(!verified_complete(&report(true, false, StepState::Failed)));
        assert!(!verified_complete(&report(true, false, StepState::Skipped)));
    }

    #[derive(Default)]
    struct FakeData {
        events: VecDeque<Event>,
        commands: Vec<Command>,
        finished: bool,
        joins: usize,
        polls: usize,
        reject_commands: bool,
        finish_after_empty: Option<Report>,
        cancel_request: Option<Box<dyn Fn()>>,
    }
    struct FakeSession(Rc<RefCell<FakeData>>);
    impl SessionIo for FakeSession {
        fn send(&self, command: Command) -> Result<()> {
            let mut data = self.0.borrow_mut();
            if data.reject_commands {
                return Err("bounded channel full".into());
            }
            if matches!(command, Command::Cancel) {
                if let Some(request) = &data.cancel_request {
                    request();
                }
            }
            data.commands.push(command);
            Ok(())
        }
        fn try_event(&self) -> Result<Option<Event>> {
            let mut data = self.0.borrow_mut();
            data.polls += 1;
            let event = data.events.pop_front();
            if event.is_none() {
                if let Some(report) = data.finish_after_empty.take() {
                    data.events.push_back(Event::Finished { report });
                    data.finished = true;
                }
            }
            Ok(event)
        }
        fn is_finished(&self) -> bool {
            self.0.borrow().finished
        }
        fn join(&mut self) -> Result<()> {
            let mut data = self.0.borrow_mut();
            assert!(data.finished, "attempt to join a running worker");
            data.joins += 1;
            Ok(())
        }
    }

    const NARROW_WIDTH: i32 = 480;
    const NARROW_HEIGHT: i32 = 420;

    fn assert_allocated<W: glib::object::IsA<gtk::Widget>>(widget: &W, name: &str) {
        let widget = widget.as_ref();
        let allocation = widget.allocation();
        assert!(widget.is_visible(), "{name} is not visible");
        assert!(
            allocation.width() > 0 && allocation.height() > 0,
            "{name} has no usable allocation: {allocation:?}"
        );
    }

    fn assert_action<W: glib::object::IsA<gtk::Widget>>(widget: &W, name: &str) {
        assert_allocated(widget, name);
        let widget = widget.as_ref();
        assert!(widget.is_sensitive(), "{name} is not sensitive");
        assert!(
            widget.can_focus(),
            "{name} is not keyboard/hit-test reachable"
        );
    }

    fn assert_narrow_frame(controller: &Controller) {
        let allocation = controller.ui.window.allocation();
        assert_eq!(allocation.width(), NARROW_WIDTH);
        assert_eq!(allocation.height(), NARROW_HEIGHT);
        assert_allocated(&controller.ui.heading, "heading");
        assert_allocated(&controller.ui.introduction, "introduction");
    }

    struct NoLaunch;
    impl DesktopLauncher for NoLaunch {
        fn start(&self) -> Result<LaunchResult> {
            panic!("test unexpectedly dispatched a desktop launch")
        }
    }

    fn fixture() -> (Rc<RefCell<Controller>>, Rc<RefCell<FakeData>>) {
        let data = Rc::new(RefCell::new(FakeData::default()));
        let controller = Rc::new(RefCell::new(Controller::new(
            Widgets::new(),
            Some(Box::new(FakeSession(data.clone()))),
            Box::new(NoLaunch),
        )));
        connect(&controller);
        controller
            .borrow()
            .ui
            .window
            .resize(NARROW_WIDTH, NARROW_HEIGHT);
        controller.borrow().ui.window.show_all();
        controller.borrow().render();
        pump_gtk();
        (controller, data)
    }
    fn pump_gtk() {
        let context = glib::MainContext::default();
        for _ in 0..64 {
            if !context.pending() {
                break;
            }
            context.iteration(false);
        }
    }
    fn review_event(id: u64) -> Event {
        Event::Review {
            request_id: id,
            description:
                "Exact <literal> artifact effects\nNo registration or start authorization.".into(),
        }
    }
    fn destroy(controller: Rc<RefCell<Controller>>) {
        assert!(
            controller.borrow().session.is_none(),
            "test left an owned worker"
        );
        // SAFETY: private test window with no worker, on the sole GTK test thread.
        unsafe { controller.borrow().ui.window.destroy() };
    }

    fn settle_layout() {
        // GTK resize/style work is scheduled on the frame clock, not necessarily
        // pending when a nonblocking main-context drain first returns.
        for _ in 0..12 {
            pump_gtk();
            std::thread::sleep(Duration::from_millis(5));
        }
        pump_gtk();
    }

    fn descendants(widget: &gtk::Widget) -> Vec<gtk::Widget> {
        let mut result = vec![widget.clone()];
        if let Some(container) = widget.downcast_ref::<gtk::Container>() {
            for child in container.children() {
                result.extend(descendants(&child));
            }
        }
        result
    }

    fn body_scroll(ui: &Widgets) -> gtk::ScrolledWindow {
        ui.stack
            .ancestor(gtk::ScrolledWindow::static_type())
            .expect("body remains scrollable")
            .downcast()
            .unwrap()
    }

    fn assert_in_window(widget: &gtk::Widget, window: &gtk::Window) {
        let (x, y) = widget.translate_coordinates(window, 0, 0).unwrap();
        let bounds = widget.allocation();
        assert!(
            x >= 0
                && y >= 0
                && x + bounds.width() <= window.allocated_width()
                && y + bounds.height() <= window.allocated_height(),
            "{} clipped: {x},{y} {bounds:?} in {:?}",
            widget.type_().name(),
            window.allocation()
        );
    }

    fn assert_visual_frame(ui: &Widgets, width: i32, height: i32) {
        assert_eq!(
            (ui.window.allocated_width(), ui.window.allocated_height()),
            (width, height),
            "text scaling must not enlarge the requested window"
        );
        let scroll = body_scroll(ui);
        assert_in_window(scroll.upcast_ref(), &ui.window);
        assert!(scroll.allocated_height() >= 80, "footer consumed the body");
        assert_eq!(scroll.hscrollbar_policy(), gtk::PolicyType::Never);
        assert_eq!(ui.stack.transition_type(), gtk::StackTransitionType::None);
        assert!(ui.details.parent().is_none());
        assert!(ui.window.default_widget().is_none(), "no default approval");
        for button in [&ui.primary, &ui.secondary] {
            if button.is_visible() {
                assert_in_window(button.upcast_ref(), &ui.window);
                assert!(button.allocated_height() >= 48, "footer hit target <48px");
                if button.is_sensitive() {
                    assert_action(button, "footer action");
                }
            }
        }
        if ui.footer_shell.is_visible() {
            assert_in_window(ui.footer_shell.upcast_ref(), &ui.window);
        }
        if ui.model.is_mapped() {
            assert!(
                ui.model.allocated_height() >= 48,
                "model chooser hit target <48px"
            );
            assert!(
                ui.remove.allocated_height() >= 48,
                "model removal hit target <48px"
            );
        }
        let viewport = scroll.child().unwrap();
        let adjustment = scroll.vadjustment();
        let body = viewport
            .clone()
            .downcast::<gtk::Container>()
            .unwrap()
            .children()[0]
            .clone();
        for widget in descendants(&body).into_iter().filter(|w| w.is_mapped()) {
            let (x, y) = widget.translate_coordinates(&body, 0, 0).unwrap();
            assert!(
                x >= 0 && x + widget.allocated_width() <= body.allocated_width(),
                "horizontal clipping in {}: {x} + {} > {}",
                widget.type_().name(),
                widget.allocated_width(),
                body.allocated_width()
            );
            assert!(
                y >= 0 && f64::from(y + widget.allocated_height()) <= adjustment.upper() + 1.0,
                "body content cannot be reached by scrolling: {}",
                widget.type_().name()
            );
            if let Some(label) = widget.downcast_ref::<gtk::Label>() {
                assert_eq!(
                    label.ellipsize(),
                    gtk::pango::EllipsizeMode::None,
                    "body text must remain complete: {}",
                    label.text()
                );
                let (_, text_height) = label
                    .layout()
                    .expect("allocated label has a layout")
                    .pixel_size();
                assert!(
                    text_height <= label.allocated_height(),
                    "wrapped text clipped vertically: {} ({text_height} > {})",
                    label.text(),
                    label.allocated_height()
                );
            }
        }
        if width >= 1024 {
            assert!(
                ui.heading.allocated_width() <= 640,
                "unbounded reading column"
            );
        }
    }

    fn capture_view(ui: &Widgets, name: &str, text_scale: i32) {
        // This opt-in writer exists only in the test binary. Never consult a
        // production display, package, manager, model or session for a fixture.
        let Some(directory) = std::env::var_os("SMITH_UI_CAPTURE_DIR") else {
            return;
        };
        use sha2::{Digest, Sha256};
        let directory = PathBuf::from(directory);
        assert!(
            directory.is_absolute(),
            "capture directory must be explicit and absolute"
        );
        std::fs::create_dir_all(&directory).unwrap();
        let (width, height) = (ui.window.allocated_width(), ui.window.allocated_height());
        let scale = ui.window.scale_factor();
        let surface = gtk::cairo::ImageSurface::create(
            gtk::cairo::Format::ARgb32,
            width * scale,
            height * scale,
        )
        .unwrap();
        surface.set_device_scale(f64::from(scale), f64::from(scale));
        let context = gtk::cairo::Context::new(&surface).unwrap();
        ui.window.draw(&context);
        surface.flush();
        let pixbuf =
            gtk::gdk::pixbuf_get_from_surface(&surface, 0, 0, width * scale, height * scale)
                .expect("Widget.draw produced a readable Cairo surface");
        let path = directory.join(format!("{name}.png"));
        assert!(!path.exists(), "capture names must be unique");
        pixbuf.savev(&path, "png", &[]).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let scroll = body_scroll(ui).vadjustment();
        let settings = gtk::Settings::default().unwrap();
        let metadata = serde_json::json!({
            "synthetic": true,
            "renderer": "GTK3 Widget.draw / Cairo ImageSurface / GdkPixbuf PNG",
            "logical_size": [width, height],
            "png_size": [pixbuf.width(), pixbuf.height()],
            "gdk_scale_factor": scale,
            "text_scale": text_scale,
            "gtk_font_name": settings.property::<String>("gtk-font-name"),
            "gtk_xft_dpi": settings.property::<i32>("gtk-xft-dpi"),
            "font_family": appearance::FONT_FAMILY,
            "heading_pango_font": ui.heading.pango_context().font_description().unwrap().to_string(),
            "scroll": {"value": scroll.value(), "upper": scroll.upper(), "page_size": scroll.page_size()},
            "png_bytes": bytes.len(),
            "sha256": format!("{:x}", Sha256::digest(&bytes)),
            "decorations": "client area only; no window manager in Xvfb",
        });
        std::fs::write(
            directory.join(format!("{name}.json")),
            serde_json::to_vec_pretty(&metadata).unwrap(),
        )
        .unwrap();
    }

    fn visual_checkpoint(
        controller: &Rc<RefCell<Controller>>,
        data: &Rc<RefCell<FakeData>>,
        name: &str,
        geometry: (i32, i32, i32),
        all_scroll_positions: bool,
    ) {
        let commands = data.borrow().commands.clone();
        let (width, height, text_scale) = geometry;
        settle_layout();
        let ui = &controller.borrow().ui;
        eprintln!("visual checkpoint {name} {width}x{height} text{text_scale}; window {:?}; footer {:?}/{:?}; scroll {:?}/{:?}; primary {:?}/{:?}", ui.window.allocation(), ui.footer_shell.allocation(), ui.footer_shell.preferred_height(), body_scroll(ui).allocation(), body_scroll(ui).preferred_height(), ui.primary.allocation(), ui.primary.preferred_height());
        if (ui.window.allocated_width(), ui.window.allocated_height()) != (width, height) {
            capture_view(
                ui,
                &format!("FAILED-{name}-{width}x{height}-text{text_scale}"),
                text_scale,
            );
        }
        assert_visual_frame(ui, width, height);
        let adjustment = body_scroll(ui).vadjustment();
        let end = (adjustment.upper() - adjustment.page_size()).max(0.0);
        let mut value = 0.0;
        let mut page = 0;
        loop {
            adjustment.set_value(value);
            settle_layout();
            assert!(
                (adjustment.value() - value).abs() < 1.0,
                "body scroll is unreachable"
            );
            assert_visual_frame(ui, width, height);
            capture_view(
                ui,
                &format!("{name}-{width}x{height}-text{text_scale}-scroll{page:02}"),
                text_scale,
            );
            if !all_scroll_positions || value >= end {
                break;
            }
            value = (value + adjustment.page_size() * 0.85).min(end);
            page += 1;
            assert!(page < 64, "unbounded synthetic capture");
        }
        adjustment.set_value(end);
        settle_layout();
        assert_visual_frame(ui, width, height);
        adjustment.set_value(0.0);
        assert_eq!(
            data.borrow().commands,
            commands,
            "resize/draw/scroll authorized an effect"
        );
    }

    fn finish_visual_fixture(
        controller: &Rc<RefCell<Controller>>,
        data: &Rc<RefCell<FakeData>>,
        result: Report,
    ) {
        data.borrow_mut()
            .events
            .push_back(Event::Finished { report: result });
        controller.borrow_mut().tick(false);
        assert_eq!(data.borrow().joins, 0, "a report is not worker exit");
        assert!(
            !controller.borrow().ui.primary.is_visible(),
            "no premature Close"
        );
        data.borrow_mut().finished = true;
        controller.borrow_mut().tick(false);
        assert_eq!(data.borrow().joins, 1);
    }

    fn visual_matrix() {
        let settings = gtk::Settings::default().unwrap();
        let original_font = settings.property::<String>("gtk-font-name");
        let original_dpi = settings.property::<i32>("gtk-xft-dpi");
        settings.set_property("gtk-xft-dpi", 96 * 1024);
        settings.set_property("gtk-enable-animations", false);
        settings.set_property("gtk-cursor-blink", false);
        for geometry in [
            (720, 560, 1),
            (720, 560, 2),
            (480, 420, 1),
            (480, 420, 2),
            (1024, 420, 1),
            (1440, 900, 1),
            (2560, 1080, 1),
        ] {
            let (width, height, text_scale) = geometry;
            settings.set_property("gtk-font-name", format!("Sans {}", 10 * text_scale));
            let (controller, data) = fixture();
            controller.borrow().ui.window.resize(width, height);
            controller.borrow().ui.upgrade.grab_focus();
            let check = |name, full| visual_checkpoint(&controller, &data, name, geometry, full);
            check("landing", true);
            {
                let state = controller.borrow();
                let labels: Vec<_> = descendants(state.ui.window.upcast_ref())
                    .into_iter()
                    .filter_map(|w| w.downcast::<gtk::Label>().ok())
                    .collect();
                assert!(labels.iter().any(|l| l.text() == "> SMITH"));
                assert!(labels
                    .iter()
                    .any(|l| l.text() == concat!("INSTALLER · v", env!("CARGO_PKG_VERSION"))));
                assert_eq!(
                    state
                        .ui
                        .heading
                        .pango_context()
                        .font_description()
                        .unwrap()
                        .family()
                        .as_deref(),
                    Some(appearance::FONT_FAMILY)
                );
                for button in [&state.ui.upgrade, &state.ui.uninstall] {
                    assert!(button.allocated_height() >= 88);
                    assert!(button
                        .image()
                        .unwrap()
                        .downcast::<gtk::Image>()
                        .unwrap()
                        .pixbuf()
                        .is_some());
                }
            }
            assert!(data.borrow().commands.is_empty());
            controller.borrow_mut().choose_install_or_upgrade();
            check("checking", false);
            data.borrow_mut().events.push_back(Event::Review {
                request_id: 600,
                description: "Synthetic package review\nSmith app, menu/icon, user command and paired service.\nPublish only the reviewed files under /fixture/data/smith and /fixture/bin.\nRegister, enable at login, Start and every model action still require separate approvals.".into(),
            });
            controller.borrow_mut().tick(false);
            check("review", true);
            assert!(data.borrow().commands.is_empty());
            controller.borrow_mut().install();
            let effects = vec![
                "Synthetic exact effect: write only /fixture/config/systemd/user/smith-next.service.".to_string(),
                "Packaged service: /fixture/package/payload/service/smith-service; source SHA256: 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef.".into(),
                "Reload the user manager only. Do not Start, enable at login, alter lingering, or change any other unit.".into(),
                "Keep all existing conversations, preferences, downloaded models, releases, backups and preserved objects. This approval does not authorize model download, preference, inference or removal.".into(),
                "Original prepared request 601 expires without renewal. If state or identity changes, stop and start a new explicit run. Literal text remains literal: <not markup> & no hidden effects. END OF EXACT EFFECTS.".into(),
            ];
            controller.borrow_mut().event(Event::Confirmation {
                request_id: 601,
                title: "Register Smith service".into(),
                effects: effects.clone(),
            });
            check("confirmation-exact", true);
            {
                let state = controller.borrow();
                assert_eq!(state.ui.effects.text(), effects.join("\n\n"));
                assert_eq!(
                    state.ui.introduction.text(),
                    "Review the next action before approving it."
                );
                assert!(!state.ui.effects.uses_markup());
                assert!(!state.ui.stage.is_visible());
                assert!(!state.ui.progress.is_visible());
                assert!(!state.ui.elapsed.is_visible());
                assert_eq!(
                    state.ui.window.focused_widget(),
                    Some(state.ui.effects.clone().upcast())
                );
                assert_eq!(state.pending.as_ref().unwrap().request_id, 601);
            }
            assert_eq!(data.borrow().commands.len(), 1);
            controller.borrow_mut().approve();
            controller.borrow_mut().approve();
            assert_eq!(data.borrow().commands.len(), 2);
            assert_eq!(
                data.borrow().commands[1],
                Command::Approve { request_id: 601 }
            );
            controller.borrow_mut().event(Event::Models {
                request_id: 602,
                selected: None,
                installed: vec![],
            });
            check("model", true);
            assert_eq!(
                controller
                    .borrow()
                    .ui
                    .model
                    .model()
                    .unwrap()
                    .iter_n_children(None),
                smith_service::runner::CATALOG.len() as i32
            );
            let chooser = controller.borrow().ui.model.clone();
            chooser.set_active_id(Some(smith_service::runner::CATALOG[0].name));
            pump_gtk();
            assert_eq!(
                data.borrow().commands.len(),
                2,
                "selection is not model preparation or approval"
            );
            chooser.set_active_id(Some(smith_service::runner::DEFAULT_MODEL));
            controller.borrow_mut().model_action();
            assert_eq!(
                controller.borrow().ui.introduction.text(),
                "Working on this step. Cancelling does not undo earlier changes."
            );
            assert!(
                matches!(&data.borrow().commands[2], Command::ModelAction { request_id: 602, model, remove: false } if model == smith_service::runner::DEFAULT_MODEL)
            );
            controller.borrow_mut().event(Event::Progress {
                downloaded: 4_500_876_480,
                total: 9_001_752_960,
            });
            controller.borrow().ui.elapsed.set_text("Elapsed: 42s");
            check("progress-download", false);
            assert_eq!(controller.borrow().ui.progress.fraction(), 0.5);
            assert!(
                !controller.borrow().ui.primary.is_visible(),
                "no disabled pseudo-action during work"
            );
            controller.borrow_mut().event(Event::Stage {
                message: "Hashing the downloaded model; no inference has been verified.".into(),
            });
            check("progress-indeterminate", false);
            assert_eq!(controller.borrow().ui.progress.fraction(), 0.0);
            let mut result = report(true, false, StepState::Completed);
            result.message = "Smith artifacts and selected service operations completed. The exact model check was verified by the service.".into();
            result.steps.push(StepResult {
                label: "Model setup".into(),
                state: StepState::Completed,
                detail: "Exact model inference verified".into(),
            });
            finish_visual_fixture(&controller, &data, result);
            check("success", true);
            assert!(controller.borrow().ui.results.children().is_empty());
            assert!(controller
                .borrow()
                .ui
                .inference
                .text()
                .contains("Inference: Verified"));
            let commands = data.borrow().commands.clone();
            let close = controller.borrow().ui.primary.clone();
            close.emit_clicked();
            assert!(controller.borrow().quit);
            assert_eq!(
                data.borrow().commands,
                commands,
                "Close must not send consent"
            );
            destroy(controller);

            let (controller, data) = fixture();
            controller.borrow().ui.window.resize(width, height);
            data.borrow_mut().events.push_back(review_event(610));
            controller.borrow_mut().tick(false);
            controller.borrow_mut().choose_install_or_upgrade();
            controller.borrow_mut().install();
            let mut partial = report(false, false, StepState::Completed);
            partial.message = "Service Start could not be verified. Earlier artifact and registration effects remain; setup is incomplete.".into();
            partial.steps = vec![
                StepResult { label: "Smith artifacts".into(), state: StepState::Completed, detail: "The paired files were published. Existing releases and backup files are preserved.".into() },
                StepResult { label: "Register service".into(), state: StepState::Completed, detail: "The exact reviewed user unit was written and reload was accepted.".into() },
                StepResult { label: "Start now".into(), state: StepState::Uncertain, detail: "The manager accepted Start but the private service bridge could not be verified before the deadline. The service may still be running; cancelling observation does not stop accepted work.".into() },
                StepResult { label: "Model setup".into(), state: StepState::Failed, detail: "Service readiness is unknown. No model was downloaded, selected, removed or tested. Inspect the service state before a new explicit run; there is no automatic retry, rollback or cleanup. END OF PARTIAL OUTCOMES.".into() },
            ];
            finish_visual_fixture(&controller, &data, partial);
            visual_checkpoint(&controller, &data, "partial-failure", geometry, true);
            assert_eq!(controller.borrow().ui.results.children().len(), 4);
            assert!(controller
                .borrow()
                .ui
                .outcome
                .text()
                .contains("not rollback"));
            assert_eq!(controller.borrow().exit_code(), 1);
            destroy(controller);
        }
        settings.set_property("gtk-font-name", "Sans 10");
        visual_decisions_and_results();
        settings.set_property("gtk-font-name", original_font);
        settings.set_property("gtk-xft-dpi", original_dpi);
    }

    fn visual_decisions_and_results() {
        let geometry = (720, 560, 1);
        let (controller, data) = fixture();
        controller.borrow().ui.window.resize(geometry.0, geometry.1);
        data.borrow_mut().events.push_back(review_event(700));
        controller.borrow_mut().tick(false);
        controller.borrow_mut().choose_install_or_upgrade();
        controller.borrow_mut().install();
        for (index, (name, title, effect)) in [
            ("enable", "Enable Smith at login", "Synthetic exact effect: enable only smith-next.service for the current user default.target. Do not Start now or change lingering."),
            ("start", "Start Smith service", "Synthetic exact effect: Start the reviewed paired service now. Enablement is unchanged; no model inference is authorized."),
            ("download", "Download model", "Synthetic exact effect: download Qwen3-14B Q4_K_M, 9001752960 bytes, from the exact reviewed HTTPS catalog revision. Verify its size and SHA256 before publication. No resume, preference change or inference."),
            ("preference", "Save model preference", "Synthetic exact effect: select the checked Qwen3-14B model at the displayed expected configuration revision. Stale configuration must refuse; no inference."),
            ("inference", "Test model", "Synthetic exact effect: run the fixed greeting through the verified Smith service with the selected model. No saved conversation is used or appended. This is inference, not acknowledgement of the download."),
            ("removal", "Remove model permanently", "Synthetic destructive effect: remove only the reviewed non-selected, unloaded model and its marker. Preserve all conversations, preferences, other models and partial/orphan data on failure. No implicit unload or Stop."),
            ("uninstall", "Unregister Smith service", "Synthetic destructive effect: disable and verify removal of owned login enablement, then unregister the exact owned smith-next.service. Preserve ALL app/Core data, models, releases, backups and preserved objects. Lingering and unrelated units are unchanged."),
        ].into_iter().enumerate() {
            let request_id = 701 + index as u64;
            controller.borrow_mut().event(Event::Confirmation {
                request_id, title: title.into(), effects: vec![effect.into()],
            });
            visual_checkpoint(&controller, &data, &format!("approval-{name}"), geometry, true);
            assert_eq!(controller.borrow().ui.effects.text(), effect);
            let window = controller.borrow().ui.window.clone();
            let before = data.borrow().commands.clone();
            // Each new request moves focus to the selectable exact effects, not
            // the next approval. Repeated Enter/default activation cannot carry
            // authority from the previous request into this one.
            for _ in 0..20 {
                window.activate_default();
                window.activate_focus();
            }
            assert_eq!(data.borrow().commands, before, "held activation approved a new request");
            let button = controller.borrow().ui.primary.clone();
            button.grab_focus();
            assert_eq!(window.focused_widget(), Some(button.clone().upcast()));
            button.emit_clicked();
            controller.borrow_mut().approve();
            assert_eq!(data.borrow().commands.len(), before.len() + 1);
            assert_eq!(data.borrow().commands.last(), Some(&Command::Approve { request_id }));
        }
        controller.borrow_mut().stop(false);
        visual_checkpoint(&controller, &data, "cancelling", geometry, true);
        finish_visual_fixture(
            &controller,
            &data,
            report(false, true, StepState::Completed),
        );
        visual_checkpoint(&controller, &data, "cancelled-preserved", geometry, true);
        destroy(controller);

        for (name, scope, command, message) in [
            ("upgrade", Scope::Upgrade, Command::Upgrade, "Paired artifacts upgraded and the service verified ready. Models were not checked."),
            ("uninstall", Scope::Uninstall, Command::Uninstall, "Smith active integration was removed. Your data and preserved files remain."),
            ("model-check", Scope::ModelCheck, Command::ManageModels, "The selected model check completed; no Smith artifacts were published."),
            ("model-removal", Scope::ModelRemoval, Command::ManageModels, "The exact reviewed model was removed. Other models and all conversations remain."),
        ] {
            let (controller, data) = fixture();
            controller.borrow_mut().bundle = Some(PathBuf::from("/fixture/package"));
            controller.borrow().ui.window.resize(geometry.0, geometry.1);
            data.borrow_mut().events.push_back(review_event(720));
            controller.borrow_mut().tick(false);
            controller.borrow_mut().independent_action(command);
            let mut result = report(true, false, StepState::Completed);
            result.scope = scope;
            result.message = message.into();
            finish_visual_fixture(&controller, &data, result);
            visual_checkpoint(&controller, &data, &format!("success-{name}"), geometry, true);
            assert_eq!(controller.borrow().ui.heading.text(), scope.completion_heading());
            assert!(controller.borrow().ui.secondary.is_visible(), "management remains reachable");
            let before = data.borrow().commands.clone();
            let close = controller.borrow().ui.primary.clone();
            close.emit_clicked();
            assert_eq!(data.borrow().commands, before);
            assert_eq!(data.borrow().joins, 1);
            destroy(controller);
        }
    }

    #[derive(Default)]
    struct LaunchFake {
        calls: usize,
        replies: Vec<std::sync::mpsc::SyncSender<Result<()>>>,
        start_error: bool,
    }
    struct FakeLauncher(Rc<RefCell<LaunchFake>>);
    impl DesktopLauncher for FakeLauncher {
        fn start(&self) -> Result<LaunchResult> {
            let mut fake = self.0.borrow_mut();
            fake.calls += 1;
            if fake.start_error {
                return Err("Cannot create launch thread".into());
            }
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            fake.replies.push(sender);
            Ok(receiver)
        }
    }

    fn launch_cases() {
        for scope in [
            Scope::Installation,
            Scope::Upgrade,
            Scope::Uninstall,
            Scope::ModelCheck,
            Scope::ModelRemoval,
        ] {
            let (controller, data) = fixture();
            let fake = Rc::new(RefCell::new(LaunchFake::default()));
            {
                let mut state = controller.borrow_mut();
                state.launcher = Box::new(FakeLauncher(fake.clone()));
                state.bundle = Some(PathBuf::from("/fixture/package"));
                state.choices = Some(Choices::smith_install());
                state.active_scope = scope;
                state.phase = Phase::Working;
            }
            let mut result = report(true, false, StepState::Completed);
            result.scope = scope;
            data.borrow_mut()
                .events
                .push_back(Event::Finished { report: result });
            controller.borrow_mut().tick(false);
            controller.borrow_mut().launch();
            assert!(!controller.borrow().launch_eligible(), "worker not joined");
            assert_eq!(fake.borrow().calls, 0);
            data.borrow_mut().finished = true;
            controller.borrow_mut().tick(false);
            let eligible = matches!(scope, Scope::Installation | Scope::Upgrade);
            assert_eq!(controller.borrow().launch_eligible(), eligible);
            assert_eq!(fake.borrow().calls, 0, "completion never launches");
            if !eligible {
                controller.borrow_mut().launch();
                assert_eq!(fake.borrow().calls, 0);
                close_request(&controller);
                destroy(controller);
                continue;
            }
            settle_layout();
            assert_action(&controller.borrow().ui.secondary, "Launch Smith");
            assert_action(&controller.borrow().ui.primary, "Close");
            assert!(controller.borrow().ui.manage.is_visible());
            assert!(
                !controller.borrow().ui.secondary.has_default(),
                "launch is not default activation"
            );
            for phase in [
                Phase::Landing,
                Phase::Inspecting,
                Phase::Review,
                Phase::Model,
                Phase::Working,
                Phase::Confirmation,
                Phase::Stopping,
                Phase::Finishing,
            ] {
                controller.borrow_mut().phase = phase;
                controller.borrow_mut().launch();
                assert_eq!(fake.borrow().calls, 0);
            }
            controller.borrow_mut().phase = Phase::Terminal;
            for case in 0..7 {
                {
                    let mut state = controller.borrow_mut();
                    match case {
                        0 => state.report.as_mut().unwrap().complete = false,
                        1 => state.report.as_mut().unwrap().cancelled = true,
                        2 => state.report.as_mut().unwrap().steps[0].state = StepState::Uncertain,
                        3 => state.fault = Some("join/report failure".into()),
                        4 => state.stopped_by_user = true,
                        5 => state.restart_blocked = true,
                        6 => state.choices = None,
                        _ => unreachable!(),
                    }
                    state.render();
                    assert!(!state.launch_eligible());
                    assert_ne!(state.ui.secondary.label().as_deref(), Some("Launch Smith"));
                    state.launch();
                    state.report = Some(Report {
                        scope,
                        ..report(true, false, StepState::Completed)
                    });
                    state.fault = None;
                    state.stopped_by_user = false;
                    state.restart_blocked = false;
                    state.choices = Some(Choices::smith_install());
                }
                assert_eq!(fake.borrow().calls, 0);
            }
            controller.borrow().render();
            let launch = controller.borrow().ui.secondary.clone();
            launch.emit_clicked();
            launch.emit_clicked();
            controller.borrow_mut().launch();
            assert_eq!(fake.borrow().calls, 1);
            assert!(controller.borrow().launch_pending.is_some());
            assert!(!controller.borrow().ui.manage.is_sensitive());
            controller.borrow_mut().manage();
            assert!(
                controller.borrow().session.is_none(),
                "management blocked during dispatch"
            );
            fake.borrow_mut()
                .replies
                .pop()
                .unwrap()
                .send(Err("unsafe\u{1b}\u{202e} target".repeat(1000)))
                .unwrap();
            controller.borrow_mut().tick(false);
            assert_eq!(controller.borrow().exit_code(), 0);
            assert!(controller.borrow().complete());
            assert!(!controller
                .borrow()
                .ui
                .launch_status
                .text()
                .contains('\u{1b}'));
            assert!(!controller
                .borrow()
                .ui
                .launch_status
                .text()
                .contains('\u{202e}'));
            assert!(controller.borrow().ui.launch_status.text().len() <= TEXT_LIMIT + 100);
            assert!(controller.borrow().ui.secondary.is_sensitive());
            launch.emit_clicked();
            assert_eq!(fake.borrow().calls, 2);
            fake.borrow_mut()
                .replies
                .pop()
                .unwrap()
                .send(Ok(()))
                .unwrap();
            controller.borrow_mut().tick(false);
            assert!(controller
                .borrow()
                .ui
                .launch_status
                .text()
                .contains("Launch requested"));
            launch.emit_clicked();
            assert_eq!(fake.borrow().calls, 2);
            close_request(&controller);
            assert!(controller.borrow().quit);
            assert_eq!(controller.borrow().exit_code(), 0);
            destroy(controller);
        }
        for success in [false, true] {
            let (controller, data) = fixture();
            let fake = Rc::new(RefCell::new(LaunchFake::default()));
            {
                let mut state = controller.borrow_mut();
                state.launcher = Box::new(FakeLauncher(fake.clone()));
                state.choices = Some(Choices::smith_install());
            }
            data.borrow_mut().events.push_back(Event::Finished {
                report: report(true, false, StepState::Completed),
            });
            data.borrow_mut().finished = true;
            controller.borrow_mut().tick(false);
            fake.borrow_mut().start_error = true;
            controller.borrow_mut().launch();
            assert!(controller.borrow().launch_pending.is_none());
            assert_eq!(controller.borrow().exit_code(), 0);
            fake.borrow_mut().start_error = false;
            controller.borrow_mut().launch();
            let close = controller.borrow().ui.primary.clone();
            close.emit_clicked();
            close_request(&controller);
            assert!(!controller.borrow().quit);
            assert!(controller.borrow().close_after_launch);
            if success {
                fake.borrow_mut()
                    .replies
                    .pop()
                    .unwrap()
                    .send(Ok(()))
                    .unwrap();
            } else {
                // A dead worker must release pending Close, not strand GTK.
                fake.borrow_mut().replies.clear();
            }
            assert!(controller.borrow_mut().tick(false));
            assert_eq!(controller.borrow().exit_code(), 0);
            destroy(controller);
        }
    }

    fn provider_catalog() -> ProviderCatalog {
        use serde_json::json;
        let providers: Vec<_> = Provider::ALL
            .iter()
            .map(|p| {
                json!({"provider":p.id(),"revision":0,"credential_generation":null,
            "credential_state":"not_configured","credential_error":null,"validation":null})
            })
            .collect();
        let models: Vec<_> = smith_service::cloud::MODELS.iter().map(|model| json!({"group":"cloud","label":model.label,
            "route":{"provider":model.vendor.name(),"model":model.id,"credential_generation":null},"available":false,
            "unavailable_reason":{"code":"PROVIDER_NOT_CONFIGURED","message":"Synthetic missing credential"},"validated_model":false})).collect();
        ProviderCatalog::parse(json!({"config_revision":0,"inventory_observed":false,"providers":providers,"models":models})).unwrap()
    }
    fn providers_event(request_id: u64) -> Event {
        Event::Providers {
            request_id,
            catalog: provider_catalog(),
            message: "Optional Providers; local setup is unchanged.".into(),
            pending_request: None,
        }
    }
    fn provider_cases() {
        let (controller, data) = fixture();
        assert!(!controller.borrow().ui.manage_providers.is_visible());
        assert!(!controller.borrow().ui.result_providers.is_visible());
        data.borrow_mut().events.push_back(review_event(600));
        controller.borrow_mut().tick(false);
        controller.borrow_mut().choose_install_or_upgrade();
        let manage = controller.borrow().ui.manage_providers.clone();
        assert!(manage.is_visible() && manage.is_sensitive());
        manage.emit_clicked();
        assert_eq!(data.borrow().commands, [Command::ManageProviders]);
        data.borrow_mut().events.push_back(providers_event(601));
        controller.borrow_mut().tick(false);
        assert_eq!(controller.borrow().phase, Phase::Providers);
        let action = controller.borrow().ui.provider_action.clone();
        let key = controller.borrow().ui.provider_key.clone();
        let reveal = controller.borrow().ui.provider_reveal.clone();
        let provider = controller.borrow().ui.provider_choice.clone();
        action.set_active_id(Some("validate"));
        assert!(!controller.borrow().ui.primary.is_sensitive());
        assert!(controller.borrow().ui.provider_model.is_visible());
        action.set_active_id(Some("save"));
        key.set_text("synthetic-gtk-key-NOT-A-CREDENTIAL");
        assert!(!EntryExt::is_visible(&key));
        reveal.set_active(true);
        assert!(EntryExt::is_visible(&key));
        assert!(controller.borrow().ui.primary.is_sensitive());
        assert_eq!(
            data.borrow().commands.len(),
            1,
            "Selection/typing must have no worker effect"
        );
        controller.borrow_mut().provider_action(false);
        assert!(key.text().is_empty() && !EntryExt::is_visible(&key) && !reveal.is_active());
        assert!(!format!("{:?}", data.borrow().commands).contains("synthetic-gtk-key"));
        assert!(matches!(
            &data.borrow().commands[1],
            Command::ProviderAction {
                request_id: 601,
                action: ProviderAction::Save {
                    provider: Provider::Openai,
                    ..
                }
            }
        ));
        data.borrow_mut()
            .events
            .push_back(Event::ProviderConfirmation {
                request_id: 602,
                title: "Save OpenAI API key".into(),
                effects: vec![
                    "Save only to keyring; no API validation. Previous items retained.".into(),
                ],
            });
        controller.borrow_mut().tick(false);
        assert_eq!(
            controller.borrow().ui.secondary.label().as_deref(),
            Some("Not now")
        );
        assert!(controller
            .borrow()
            .ui
            .effects
            .text()
            .contains("no API validation"));
        controller.borrow_mut().decline();
        assert!(!controller.borrow().stopped_by_user);
        data.borrow_mut().events.push_back(providers_event(603));
        controller.borrow_mut().tick(false);
        key.set_text("unsubmitted-fixture-key");
        provider.set_active_id(Some("anthropic"));
        assert!(key.text().is_empty());
        assert_eq!(data.borrow().commands.len(), 3);
        key.set_text("invalid whitespace");
        controller.borrow_mut().provider_action(false);
        assert!(key.text().is_empty());
        assert_eq!(data.borrow().commands.len(), 3);
        assert!(controller
            .borrow()
            .ui
            .provider_status
            .text()
            .contains("without whitespace"));
        key.set_text(&"x".repeat(5000));
        assert_eq!(key.text().len(), 4097);
        controller.borrow_mut().provider_action(false);
        assert!(key.text().is_empty());
        assert_eq!(
            data.borrow().commands.len(),
            3,
            "Oversized input must not be saved after truncation"
        );
        controller.borrow_mut().provider_action(true);
        assert!(matches!(
            &data.borrow().commands[3],
            Command::ProviderAction {
                request_id: 603,
                action: ProviderAction::Done
            }
        ));
        let mut done = report(true, false, StepState::Completed);
        done.scope = Scope::Providers;
        done.providers = Some(smith_installer::providers::Outcome {
            success: true,
            message: "Not configured; skipping is not validation.".into(),
        });
        data.borrow_mut()
            .events
            .push_back(Event::Finished { report: done });
        data.borrow_mut().finished = true;
        controller.borrow_mut().tick(false);
        assert_eq!(controller.borrow().phase, Phase::Terminal);
        assert!(!controller.borrow().launch_eligible());
        destroy(controller);

        // Unknown work offers only observation as an effect; Close clears any
        // abandoned input and waits for the existing joined worker.
        let (controller, data) = fixture();
        data.borrow_mut().events.push_back(review_event(610));
        controller.borrow_mut().tick(false);
        controller
            .borrow_mut()
            .independent_action(Command::ManageProviders);
        let mut event = providers_event(611);
        if let Event::Providers {
            pending_request, ..
        } = &mut event
        {
            *pending_request = Some("a".repeat(32));
        }
        data.borrow_mut().events.push_back(event);
        controller.borrow_mut().tick(false);
        assert_eq!(
            controller
                .borrow()
                .ui
                .provider_action
                .active_id()
                .as_deref(),
            Some("observe")
        );
        assert!(controller
            .borrow()
            .ui
            .provider_status
            .text()
            .contains("Original uncertain request"));
        let key = controller.borrow().ui.provider_key.clone();
        key.set_text("abandoned-fixture-key");
        controller.borrow_mut().stop(true);
        assert!(key.text().is_empty());
        assert!(!controller.borrow().quit);
        data.borrow_mut().events.push_back(Event::Finished {
            report: report(false, true, StepState::Skipped),
        });
        data.borrow_mut().finished = true;
        controller.borrow_mut().tick(false);
        destroy(controller);

        // Optional provider failure must not erase verified local install/Launch.
        let (controller, data) = fixture();
        controller.borrow_mut().bundle = Some(PathBuf::from("/fixture/package"));
        data.borrow_mut().events.push_back(review_event(620));
        controller.borrow_mut().tick(false);
        controller.borrow_mut().choose_install_or_upgrade();
        controller.borrow_mut().install();
        data.borrow_mut().events.push_back(providers_event(621));
        controller.borrow_mut().tick(false);
        controller.borrow_mut().provider_action(true);
        let mut done = report(true, false, StepState::Completed);
        done.providers = Some(smith_installer::providers::Outcome {
            success: false,
            message: "Optional provider validation failed; saved key and local setup remain."
                .into(),
        });
        data.borrow_mut()
            .events
            .push_back(Event::Finished { report: done });
        data.borrow_mut().finished = true;
        controller.borrow_mut().tick(false);
        assert!(controller.borrow().launch_eligible());
        assert!(controller.borrow().ui.result_providers.is_visible());
        assert!(controller.borrow().ui.results.children().iter().any(|w| w
            .clone()
            .downcast::<gtk::Label>()
            .is_ok_and(|l| l.text().contains("Optional provider validation failed"))));
        assert_eq!(controller.borrow().exit_code(), 0);
        destroy(controller);
    }

    // Keep all GTK scenarios in one test: GTK must stay on the initialization
    // thread. Run under isolated Xvfb; no production session/host effect is used.
    #[test]
    fn gtk_widget_smoke_and_session_transitions() {
        gtk::init().expect("GTK smoke requires an isolated Xvfb display");
        let _fonts = appearance::load_fonts().unwrap();
        appearance::tests::assert_gtk_images();
        let css = gtk::CssProvider::new();
        css.load_from_data(CSS.as_bytes()).expect("valid GTK3 CSS");
        gtk::StyleContext::add_provider_for_screen(
            &gtk::gdk::Screen::default().unwrap(),
            &css,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        for (command, scope, heading) in [
            (Command::Upgrade, Scope::Upgrade, "Upgrade complete"),
            (Command::Uninstall, Scope::Uninstall, "Uninstall complete"),
        ] {
            let (controller, data) = fixture();
            controller.borrow_mut().bundle = Some(PathBuf::from("/fixture/package"));
            assert_eq!(
                controller.borrow().ui.upgrade.label().as_deref(),
                Some("INSTALL / UPGRADE")
            );
            assert_eq!(
                controller.borrow().ui.uninstall.label().as_deref(),
                Some("UNINSTALL")
            );
            assert!(controller.borrow().ui.upgrade.is_sensitive());
            assert!(controller.borrow().ui.uninstall.is_sensitive());
            data.borrow_mut().events.push_back(Event::Stage {
                message: "Preview unavailable: installed service mismatch".into(),
            });
            controller.borrow_mut().tick(false);
            assert_eq!(controller.borrow().phase, Phase::Landing);
            assert!(controller.borrow().ui.upgrade.is_visible());
            assert!(controller.borrow().ui.uninstall.is_visible());
            controller.borrow_mut().independent_action(command.clone());
            assert_eq!(
                controller.borrow().ui.introduction.text(),
                "Working on this step. Cancelling does not undo earlier changes."
            );
            assert_eq!(data.borrow().commands, [command]);
            assert_eq!(controller.borrow().active_scope, scope);
            assert_eq!(
                controller.borrow().ui.secondary.label().as_deref(),
                Some("Cancel operation…")
            );
            let mut result = report(true, false, StepState::Completed);
            result.scope = scope;
            data.borrow_mut()
                .events
                .push_back(Event::Finished { report: result });
            data.borrow_mut().finished = true;
            controller.borrow_mut().tick(false);
            assert_eq!(controller.borrow().ui.heading.text(), heading);
            assert!(data.borrow().joins > 0);
            assert!(!controller.borrow().ui.upgrade.is_visible());
            assert!(!controller.borrow().ui.uninstall.is_visible());
            if scope == Scope::Uninstall {
                assert!(controller
                    .borrow()
                    .ui
                    .outcome
                    .text()
                    .contains("ALL app/Core state"));
                assert!(controller.borrow().ui.outcome.text().contains("not purged"));
            }
            controller.borrow_mut().restart_blocked = true;
            controller.borrow_mut().render();
            assert!(!controller.borrow().ui.upgrade.is_visible());
            assert!(!controller.borrow().ui.uninstall.is_visible());
            destroy(controller);
        }
        let (controller, data) = fixture();
        assert_eq!(controller.borrow().ui.window.default_size(), (720, 560));
        assert!(controller.borrow().ui.window.is_resizable());
        assert_eq!(controller.borrow().phase, Phase::Landing);
        assert!(controller.borrow().ui.register.is_active());
        assert!(controller.borrow().ui.start.is_active());
        assert!(!controller.borrow().ui.register.is_visible());
        assert!(!controller.borrow().ui.start.is_visible());
        assert!(!controller.borrow().ui.recipe.is_visible());
        assert!(!controller.borrow().ui.refresh.is_visible());
        assert!(!controller.borrow().ui.manage.is_visible());
        assert!(controller.borrow().ui.details.parent().is_none());
        assert!(!controller.borrow().ui.footer_shell.is_visible());
        assert!(!controller.borrow().ui.primary.is_visible());
        assert!(!controller.borrow().ui.secondary.is_visible());
        assert!(controller.borrow().ui.upgrade.is_visible());
        assert!(controller.borrow().ui.uninstall.is_visible());
        pump_gtk();
        assert_narrow_frame(&controller.borrow());
        assert_action(&controller.borrow().ui.upgrade, "INSTALL / UPGRADE");
        assert_action(&controller.borrow().ui.uninstall, "UNINSTALL");
        assert!(!controller.borrow().ui.footer_shell.is_visible());
        assert!(!controller.borrow().ui.primary.is_visible());
        assert!(!controller.borrow().ui.secondary.is_visible());
        assert!(!controller.borrow().ui.manage.is_visible());
        assert!(!controller.borrow().ui.refresh.is_visible());
        controller.borrow_mut().choose_install_or_upgrade();
        assert_eq!(controller.borrow().phase, Phase::Inspecting);
        pump_gtk();
        assert_narrow_frame(&controller.borrow());
        assert_allocated(&controller.borrow().ui.footer_shell, "inspection footer");
        assert_eq!(controller.borrow().ui.heading.text(), "Checking Smith");
        assert!(controller
            .borrow()
            .ui
            .stage
            .text()
            .contains("Checking Smith"));
        assert!(!controller.borrow().ui.primary.is_visible());
        assert_action(&controller.borrow().ui.secondary, "inspection Cancel");
        assert_eq!(
            controller.borrow().ui.secondary.label().as_deref(),
            Some("Cancel")
        );
        assert!(!controller.borrow().ui.upgrade.is_visible());
        assert!(!controller.borrow().ui.uninstall.is_visible());
        assert!(!controller.borrow().ui.manage.is_visible());
        assert!(!controller.borrow().ui.refresh.is_visible());
        controller.borrow_mut().choose_install_or_upgrade();
        controller.borrow_mut().choose_uninstall();
        assert!(matches!(
            controller.borrow().action_after_review,
            Some(AfterReview::InstallOrUpgrade)
        ));
        assert!(data.borrow().commands.is_empty());
        data.borrow_mut().events.push_back(review_event(10));
        controller.borrow_mut().tick(false);
        assert!(
            data.borrow().commands.is_empty(),
            "review authorized an effect"
        );
        assert_eq!(controller.borrow().phase, Phase::Review);
        assert!(controller.borrow().ui.review.is_selectable());
        assert!(controller.borrow().ui.review.is_line_wrap());
        assert!(controller.borrow().ui.review.text().contains("<literal>"));
        assert!(controller.borrow().ui.primary.is_visible());
        assert_eq!(
            controller.borrow().ui.secondary.label().as_deref(),
            Some("Manage models")
        );
        assert!(controller.borrow().ui.details.parent().is_none());
        controller.borrow_mut().install();
        controller.borrow_mut().install();
        assert_eq!(
            data.borrow().commands.len(),
            1,
            "duplicate Install escaped UI guard"
        );
        assert!(matches!(
            data.borrow().commands[0],
            Command::Install {
                request_id: 10,
                choices: Choices {
                    register_service: true,
                    enable_service: true,
                    start_service: true,
                }
            }
        ));
        assert!(!controller.borrow().ui.register.is_sensitive());
        assert!(!controller.borrow().ui.start.is_sensitive());
        data.borrow_mut().events.push_back(Event::Confirmation {
            request_id: 12,
            title: "Register Smith service".into(),
            effects: vec![
                "Write the exact reviewed user unit.".into(),
                "Reload only the user manager; do not start or enable.".into(),
            ],
        });
        controller.borrow_mut().tick(false);
        pump_gtk();
        assert_narrow_frame(&controller.borrow());
        assert_allocated(&controller.borrow().ui.footer_shell, "confirmation footer");
        assert_action(
            &controller.borrow().ui.primary,
            "exact confirmation primary",
        );
        assert_action(
            &controller.borrow().ui.secondary,
            "exact confirmation Stop here",
        );
        assert!(!controller.borrow().ui.upgrade.is_visible());
        assert!(!controller.borrow().ui.uninstall.is_visible());
        assert!(!controller.borrow().ui.manage.is_visible());
        assert!(!controller.borrow().ui.refresh.is_visible());
        assert_eq!(
            controller.borrow().ui.primary.label().as_deref(),
            Some("Register Smith service")
        );
        assert_eq!(
            controller.borrow().ui.secondary.label().as_deref(),
            Some("Stop here")
        );
        assert!(controller.borrow().ui.confirmation.is_visible());
        assert!(controller.borrow().ui.effects.is_visible());
        assert!(controller.borrow().ui.effects.is_selectable());
        assert!(controller
            .borrow()
            .ui
            .effects
            .text()
            .contains("do not start or enable"));
        controller.borrow_mut().approve();
        controller.borrow_mut().approve();
        assert_eq!(data.borrow().commands.len(), 2);
        assert!(matches!(
            data.borrow().commands[1],
            Command::Approve { request_id: 12 }
        ));

        close_request(&controller);
        let dialog = controller.borrow().stop_dialog.clone().unwrap();
        assert_eq!(
            dialog.text().as_deref(),
            Some("Cancel installer operation?")
        );
        assert_eq!(
            dialog.default_widget(),
            dialog.widget_for_response(gtk::ResponseType::Cancel)
        );
        dialog.response(gtk::ResponseType::Cancel);
        assert_eq!(controller.borrow().phase, Phase::Working);
        assert_eq!(
            data.borrow().commands.len(),
            2,
            "Keep installing requested cancellation"
        );
        close_request(&controller);
        let dialog = controller.borrow().stop_dialog.clone().unwrap();
        dialog.response(gtk::ResponseType::Yes);
        assert_eq!(controller.borrow().phase, Phase::Stopping);
        assert!(matches!(data.borrow().commands[2], Command::Cancel));
        assert!(!controller.borrow_mut().tick(false));
        assert_eq!(data.borrow().joins, 0);
        assert!(!controller.borrow().ui.primary.is_sensitive());
        data.borrow_mut().events.push_back(Event::Finished {
            report: report(false, true, StepState::Completed),
        });
        controller.borrow_mut().tick(false);
        assert_eq!(
            data.borrow().joins,
            0,
            "Finished event alone joined a live worker"
        );
        assert_eq!(controller.borrow().phase, Phase::Stopping);
        data.borrow_mut().finished = true;
        assert!(
            controller.borrow_mut().tick(false),
            "normal close did not wait then exit"
        );
        assert_eq!(data.borrow().joins, 1);
        assert_eq!(
            controller.borrow().ui.heading.text(),
            "Installation cancelled"
        );
        assert!(controller
            .borrow()
            .ui
            .outcome
            .text()
            .contains("not rollback"));
        destroy(controller);

        for (complete, cancelled, state, heading) in [
            (true, false, StepState::Completed, "Installation complete"),
            (false, true, StepState::Completed, "Installation cancelled"),
            (false, false, StepState::Uncertain, "Setup incomplete"),
        ] {
            let (controller, data) = fixture();
            data.borrow_mut().events.push_back(review_event(20));
            controller.borrow_mut().tick(false);
            controller.borrow_mut().choose_install_or_upgrade();
            controller.borrow_mut().install();
            for _ in 0..20 {
                controller.borrow_mut().log(&"p".repeat(TEXT_LIMIT));
            }
            assert!(controller.borrow().log_full);
            data.borrow_mut().events.push_back(Event::Finished {
                report: report(complete, cancelled, state),
            });
            data.borrow_mut().finished = true;
            controller.borrow_mut().tick(false);
            pump_gtk();
            assert!(controller
                .borrow()
                .ui
                .details
                .text()
                .contains("Final retained worker evidence."));
            assert!(controller
                .borrow()
                .ui
                .details
                .text()
                .starts_with("Selected effects reported by the worker."));
            assert_eq!(data.borrow().joins, 1);
            assert_eq!(controller.borrow().phase, Phase::Terminal);
            assert_eq!(controller.borrow().ui.heading.text(), heading);
            assert_eq!(
                controller.borrow().ui.stack.visible_child_name().as_deref(),
                Some("terminal")
            );
            assert_eq!(
                controller.borrow().ui.primary.label().as_deref(),
                Some("Close")
            );
            assert!(controller.borrow().ui.primary.is_sensitive());
            if state == StepState::Uncertain {
                assert_narrow_frame(&controller.borrow());
                assert_allocated(&controller.borrow().ui.footer_shell, "terminal footer");
                assert_action(&controller.borrow().ui.primary, "uncertain terminal Close");
                assert!(!controller.borrow().ui.secondary.is_visible());
                assert!(!controller.borrow().ui.upgrade.is_visible());
                assert!(!controller.borrow().ui.uninstall.is_visible());
                assert!(!controller.borrow().ui.manage.is_visible());
                assert!(!controller.borrow().ui.refresh.is_visible());
                assert_allocated(
                    &controller.borrow().ui.results,
                    "uncertain terminal results",
                );
                assert_allocated(
                    &controller.borrow().ui.outcome,
                    "uncertain preservation guidance",
                );
            }
            assert!(controller
                .borrow()
                .ui
                .inference
                .text()
                .contains("inference not verified"));
            assert!(controller
                .borrow()
                .ui
                .inference
                .text()
                .contains("not opened automatically"));
            let rows = controller.borrow().ui.results.children();
            if complete {
                assert!(
                    rows.is_empty(),
                    "successful low-level steps stayed expanded"
                );
            } else {
                assert_eq!(rows.len(), 3);
                assert!(rows[1]
                    .clone()
                    .downcast::<gtk::Label>()
                    .unwrap()
                    .text()
                    .contains("Skipped"));
            }
            assert_eq!(
                controller.borrow().exit_code(),
                if complete { 0 } else { 1 }
            );
            destroy(controller);
        }

        for (complete, state) in [
            (false, StepState::Failed),
            (false, StepState::Uncertain),
            (false, StepState::Completed),
            (false, StepState::Skipped),
            (true, StepState::Completed),
            (true, StepState::Uncertain),
        ] {
            let (controller, data) = fixture();
            data.borrow_mut().events.push_back(Event::Finished {
                report: report(complete, false, state),
            });
            data.borrow_mut().finished = true;
            controller.borrow_mut().tick(false);
            assert!(controller.borrow().choices.is_none());
            assert!(controller.borrow().fault.is_none());
            assert_eq!(controller.borrow().phase, Phase::Terminal);
            assert_eq!(controller.borrow().ui.heading.text(), "Setup incomplete");
            assert_eq!(
                controller.borrow().exit_code(),
                1,
                "pre-Install report masked by absent choices: complete={complete}, state={state:?}"
            );
            assert!(
                data.borrow().commands.is_empty(),
                "terminal report authorized an effect"
            );
            assert_eq!(data.borrow().joins, 1);
            destroy(controller);
        }

        for (cancelled, stopped_by_user) in [(true, false), (false, true)] {
            let (controller, data) = fixture();
            controller.borrow_mut().stopped_by_user = stopped_by_user;
            data.borrow_mut().events.push_back(Event::Finished {
                report: report(false, cancelled, StepState::Skipped),
            });
            data.borrow_mut().finished = true;
            controller.borrow_mut().tick(false);
            assert!(controller.borrow().choices.is_none());
            assert_eq!(
                controller.borrow().ui.heading.text(),
                "Installation cancelled"
            );
            assert_eq!(
                controller.borrow().exit_code(),
                0,
                "intentional no-effect initial cancel/decline failed"
            );
            assert!(data.borrow().commands.is_empty());
            assert_eq!(data.borrow().joins, 1);
            destroy(controller);
        }

        let (controller, data) = fixture();
        assert!(controller.borrow().report.is_none());
        assert!(controller.borrow().choices.is_none());
        assert!(controller.borrow().fault.is_none());
        assert_eq!(
            controller.borrow().exit_code(),
            1,
            "missing report masked by absent choices"
        );
        controller.borrow_mut().stop(false);
        data.borrow_mut().finished = true;
        controller.borrow_mut().tick(false);
        assert!(controller.borrow().report.is_none());
        assert!(controller.borrow().fault.is_some());
        assert_eq!(controller.borrow().ui.heading.text(), "Setup incomplete");
        assert_eq!(
            controller.borrow().exit_code(),
            1,
            "initial cancel masked a missing terminal report"
        );
        assert_eq!(data.borrow().joins, 1);
        destroy(controller);

        // Numeric observations remain compact and do not grow or repeatedly rewrite
        // copyable Details. Every critical boundary clears the prior numeric display.
        let (controller, data) = fixture();
        data.borrow_mut().events.push_back(review_event(19));
        controller.borrow_mut().tick(false);
        controller.borrow_mut().choose_install_or_upgrade();
        controller.borrow_mut().install();
        let details_before = controller.borrow().ui.details.text();
        for downloaded in 1..=10_000 {
            controller.borrow_mut().event(Event::Progress {
                downloaded,
                total: 10_000,
            });
        }
        assert_eq!(controller.borrow().log.lines().count(), 1);
        assert_eq!(controller.borrow().ui.details.text(), details_before);
        assert_eq!(controller.borrow().ui.progress.fraction(), 1.0);
        assert!(controller
            .borrow()
            .ui
            .stage
            .text()
            .contains("10000 / 10000 bytes (100.0%)"));

        controller.borrow_mut().event(Event::Stage {
            message: "Hashing downloaded model".into(),
        });
        assert_eq!(controller.borrow().ui.progress.fraction(), 0.0);
        assert_eq!(
            controller.borrow().ui.stage.text(),
            "Hashing downloaded model"
        );
        controller.borrow_mut().event(Event::Progress {
            downloaded: 9_000,
            total: 10_000,
        });
        controller.borrow_mut().event(Event::Confirmation {
            request_id: 191,
            title: "Save model preference".into(),
            effects: vec!["Save the exact checked model revision.".into()],
        });
        assert_eq!(controller.borrow().ui.progress.fraction(), 0.0);

        controller.borrow_mut().phase = Phase::Working;
        controller.borrow_mut().event(Event::Progress {
            downloaded: 9_001,
            total: 10_000,
        });
        controller.borrow_mut().event(Event::Models {
            request_id: 192,
            selected: None,
            installed: vec![],
        });
        assert_eq!(controller.borrow().ui.progress.fraction(), 0.0);

        controller.borrow_mut().phase = Phase::Working;
        controller.borrow_mut().event(Event::Progress {
            downloaded: 9_002,
            total: 10_000,
        });
        controller.borrow_mut().event(Event::Finished {
            report: report(false, true, StepState::Skipped),
        });
        assert_eq!(controller.borrow().ui.progress.fraction(), 0.0);
        data.borrow_mut().finished = true;
        controller.borrow_mut().tick(false);
        destroy(controller);

        let (controller, data) = fixture();
        for _ in 0..EVENTS_PER_TICK + 5 {
            data.borrow_mut().events.push_back(Event::Stage {
                message: "Read-only progress".into(),
            });
        }
        controller.borrow_mut().tick(false);
        assert_eq!(
            data.borrow().polls,
            EVENTS_PER_TICK,
            "poll batch is unbounded"
        );
        close_request(&controller);
        assert_eq!(controller.borrow().phase, Phase::Stopping);
        assert!(data
            .borrow()
            .commands
            .iter()
            .all(|c| matches!(c, Command::Cancel)));
        data.borrow_mut().events.push_back(Event::Finished {
            report: report(false, true, StepState::Skipped),
        });
        data.borrow_mut().finished = true;
        controller.borrow_mut().tick(false);
        assert_eq!(data.borrow().joins, 1);
        assert_eq!(controller.borrow().exit_code(), 0);
        destroy(controller);

        let (controller, data) = fixture();
        controller.borrow_mut().choose_install_or_upgrade();
        assert_eq!(controller.borrow().phase, Phase::Inspecting);
        data.borrow_mut().events.push_back(Event::Stage {
            message: "Preview unavailable; explicitly Refresh review. No effects applied.".into(),
        });
        controller.borrow_mut().tick(false);
        assert!(controller
            .borrow()
            .ui
            .review
            .text()
            .contains("Preview unavailable"));
        assert!(!controller.borrow().ui.refresh.is_visible());
        assert!(!controller.borrow().ui.manage.is_visible());
        assert_eq!(data.borrow().commands, [Command::Upgrade]);
        assert_eq!(controller.borrow().active_scope, Scope::Upgrade);
        assert_eq!(controller.borrow().phase, Phase::Working);
        close_request(&controller);
        data.borrow_mut().events.push_back(Event::Finished {
            report: report(false, true, StepState::Skipped),
        });
        data.borrow_mut().finished = true;
        controller.borrow_mut().tick(false);
        destroy(controller);

        let (controller, data) = fixture();
        data.borrow_mut().events.push_back(Event::Review {
            request_id: 30,
            description: "x".repeat(EFFECT_TEXT_LIMIT + 1),
        });
        controller.borrow_mut().tick(false);
        assert_eq!(controller.borrow().phase, Phase::Stopping);
        assert!(!controller.borrow().ui.primary.is_sensitive());
        data.borrow_mut().finished = true;
        controller.borrow_mut().tick(false);
        assert_eq!(controller.borrow().ui.heading.text(), "Setup incomplete");
        destroy(controller);

        let (controller, data) = fixture();
        data.borrow_mut().events.push_back(review_event(40));
        controller.borrow_mut().tick(false);
        controller.borrow_mut().choose_install_or_upgrade();
        controller.borrow_mut().install();
        data.borrow_mut().events.push_back(Event::Confirmation {
            request_id: 41,
            title: "Reload Smith service".into(),
            effects: vec!["Reload the user manager only.".into()],
        });
        controller.borrow_mut().tick(false);
        controller.borrow_mut().decline();
        controller.borrow_mut().approve();
        assert!(matches!(
            data.borrow().commands[1],
            Command::Decline { request_id: 41 }
        ));
        assert_eq!(
            data.borrow().commands.len(),
            2,
            "declined consent remained usable"
        );
        data.borrow_mut().events.push_back(Event::Finished {
            report: report(false, false, StepState::Completed),
        });
        data.borrow_mut().finished = true;
        controller.borrow_mut().tick(false);
        assert_eq!(
            controller.borrow().ui.heading.text(),
            "Installation cancelled"
        );
        assert!(controller.borrow().ui.results.children()[0]
            .clone()
            .downcast::<gtk::Label>()
            .unwrap()
            .text()
            .contains("Completed"));
        destroy(controller);

        let (controller, data) = fixture();
        data.borrow_mut().events.push_back(review_event(45));
        controller.borrow_mut().tick(false);
        controller.borrow_mut().choose_install_or_upgrade();
        controller.borrow_mut().install();
        data.borrow_mut().finish_after_empty = Some(report(true, false, StepState::Completed));
        controller.borrow_mut().tick(false);
        assert_eq!(
            data.borrow().joins,
            0,
            "worker exit raced an empty poll and lost its report"
        );
        controller.borrow_mut().tick(false);
        assert_eq!(data.borrow().joins, 1);
        assert_eq!(
            controller.borrow().ui.heading.text(),
            "Installation complete"
        );
        destroy(controller);

        let (controller, data) = fixture();
        data.borrow_mut().events.push_back(review_event(50));
        controller.borrow_mut().tick(false);
        controller.borrow_mut().choose_install_or_upgrade();
        controller.borrow_mut().install();
        ask_stop(&controller, false);
        assert!(controller.borrow().stop_dialog.is_some());
        controller.borrow_mut().tick(true);
        assert!(controller.borrow().stop_dialog.is_none());
        assert_eq!(controller.borrow().phase, Phase::Stopping);
        assert!(matches!(data.borrow().commands[1], Command::Cancel));
        data.borrow_mut().events.push_back(Event::Finished {
            report: report(false, true, StepState::Completed),
        });
        data.borrow_mut().finished = true;
        assert!(
            !controller.borrow_mut().tick(false),
            "signal abandoned final outcomes"
        );
        assert_eq!(data.borrow().joins, 1);
        destroy(controller);

        let (controller, data) = fixture();
        data.borrow_mut().reject_commands = true;
        close_request(&controller);
        assert!(controller.borrow().cancel_pending);
        assert!(!controller.borrow_mut().tick(false));
        assert_eq!(data.borrow().joins, 0);
        data.borrow_mut().reject_commands = false;
        controller.borrow_mut().tick(false);
        assert!(!controller.borrow().cancel_pending);
        assert!(matches!(data.borrow().commands[0], Command::Cancel));
        data.borrow_mut().events.push_back(Event::Finished {
            report: report(false, true, StepState::Skipped),
        });
        data.borrow_mut().finished = true;
        assert!(controller.borrow_mut().tick(false));
        assert_eq!(data.borrow().joins, 1);
        destroy(controller);

        // Model-only completions must never inherit the installation heading.
        for scope in [
            smith_installer::wizard::Scope::ModelCheck,
            smith_installer::wizard::Scope::ModelRemoval,
        ] {
            let (controller, data) = fixture();
            controller.borrow_mut().choices = Some(Choices::default());
            let mut completed = report(true, false, StepState::Skipped);
            completed.scope = scope;
            completed.steps.push(StepResult {
                label: "Model setup".into(),
                state: StepState::Completed,
                detail: "Exact model operation verified".into(),
            });
            data.borrow_mut()
                .events
                .push_back(Event::Finished { report: completed });
            data.borrow_mut().finished = true;
            controller.borrow_mut().tick(false);
            assert_eq!(
                controller.borrow().ui.heading.text(),
                scope.completion_heading()
            );
            assert_ne!(
                controller.borrow().ui.heading.text(),
                "Installation complete"
            );
            destroy(controller);
        }

        // A repeat action queues its chosen operation until either preview result,
        // never requiring a successful artifact review and never constructing native effects here.
        for command in [Command::ManageModels, Command::Upgrade, Command::Uninstall] {
            for preview_ok in [true, false] {
                let (controller, data) = fixture();
                controller.borrow_mut().phase = Phase::Inspecting;
                controller.borrow_mut().action_after_review =
                    Some(AfterReview::Command(command.clone()));
                data.borrow_mut().events.push_back(if preview_ok {
                    review_event(340)
                } else {
                    Event::Stage {
                        message: "Preview unavailable: unknown old artifacts".into(),
                    }
                });
                controller.borrow_mut().tick(false);
                assert_eq!(
                    data.borrow().commands.as_slice(),
                    std::slice::from_ref(&command)
                );
                assert!(controller.borrow().action_after_review.is_none());
                assert_eq!(controller.borrow().phase, Phase::Working);
                controller.borrow_mut().stop(false);
                data.borrow_mut().events.push_back(Event::Finished {
                    report: report(false, true, StepState::Skipped),
                });
                data.borrow_mut().finished = true;
                controller.borrow_mut().tick(false);
                destroy(controller);
            }
        }

        // Both the Cancel request and the UI observer share one sticky process flag.
        // No fixture no-op cancellation can accidentally make this restart appear safe.
        let (controller, data) = fixture();
        let sticky = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let requested = sticky.clone();
        data.borrow_mut().cancel_request = Some(Box::new(move || {
            requested.store(true, std::sync::atomic::Ordering::Release)
        }));
        let observed = sticky.clone();
        controller.borrow_mut().cancellation =
            Box::new(move || observed.load(std::sync::atomic::Ordering::Acquire));
        controller.borrow_mut().bundle = Some(PathBuf::from("/fixture/package"));
        data.borrow_mut().events.push_back(review_event(350));
        controller.borrow_mut().tick(false);
        controller.borrow_mut().choose_install_or_upgrade();
        assert_eq!(
            controller.borrow().ui.secondary.label().as_deref(),
            Some("Manage models")
        );
        assert!(controller.borrow().ui.secondary.is_visible());
        controller.borrow_mut().stop(false);
        data.borrow_mut().events.push_back(Event::Finished {
            report: report(false, true, StepState::Skipped),
        });
        data.borrow_mut().finished = true;
        controller.borrow_mut().tick(false);
        assert!(sticky.load(std::sync::atomic::Ordering::Acquire));
        assert_eq!(data.borrow().joins, 1);
        assert!(controller.borrow().session.is_none());
        assert!(!controller.borrow().ui.manage.is_visible());
        assert!(!controller.borrow().ui.manage.is_sensitive());
        assert!(controller
            .borrow()
            .ui
            .outcome
            .text()
            .contains("Close and reopen"));
        let commands = data.borrow().commands.len();
        controller.borrow_mut().manage();
        assert!(
            controller.borrow().session.is_none(),
            "Sticky cancellation respawned a session"
        );
        assert_eq!(data.borrow().commands.len(), commands);
        assert!(sticky.load(std::sync::atomic::Ordering::Acquire));
        destroy(controller);

        // The focused model page preserves existing selections and emits only an explicit exact-name action.
        for selected in [
            None,
            Some(smith_service::runner::CATALOG[1].name.to_string()),
        ] {
            let (controller, data) = fixture();
            data.borrow_mut().events.push_back(review_event(400));
            controller.borrow_mut().tick(false);
            controller.borrow_mut().manage();
            assert!(matches!(data.borrow().commands[0], Command::ManageModels));
            data.borrow_mut().events.push_back(Event::Models {
                request_id: 401,
                selected: selected.clone(),
                installed: vec![],
            });
            controller.borrow_mut().tick(false);
            assert_eq!(controller.borrow().phase, Phase::Model);
            pump_gtk();
            assert_narrow_frame(&controller.borrow());
            assert_allocated(&controller.borrow().ui.footer_shell, "model footer");
            assert_allocated(&controller.borrow().ui.model, "model chooser");
            assert_allocated(&controller.borrow().ui.model_facts, "model status");
            assert_allocated(&controller.borrow().ui.remove, "model removal control");
            assert_action(&controller.borrow().ui.primary, "model Continue");
            assert_action(&controller.borrow().ui.secondary, "model Cancel");
            assert!(!controller.borrow().ui.upgrade.is_visible());
            assert!(!controller.borrow().ui.uninstall.is_visible());
            assert!(!controller.borrow().ui.manage.is_visible());
            assert!(!controller.borrow().ui.refresh.is_visible());
            assert_eq!(
                controller.borrow().ui.stack.visible_child_name().as_deref(),
                Some("model")
            );
            let expected = selected
                .as_deref()
                .unwrap_or(smith_service::runner::DEFAULT_MODEL);
            assert_eq!(
                controller.borrow().ui.model.active_id().as_deref(),
                Some(expected)
            );
            assert_eq!(
                data.borrow().commands.len(),
                1,
                "Opening model step approved an effect"
            );
            controller.borrow_mut().model_action();
            assert!(
                matches!(&data.borrow().commands[1], Command::ModelAction { request_id:401, model, remove:false } if model == expected)
            );
            let mut failed = report(false, false, StepState::Completed);
            failed.steps.push(StepResult {
                label: "Model setup".into(),
                state: StepState::Failed,
                detail: "Service check failed; setup incomplete".into(),
            });
            data.borrow_mut()
                .events
                .push_back(Event::Finished { report: failed });
            data.borrow_mut().finished = true;
            controller.borrow_mut().tick(false);
            pump_gtk();
            assert_narrow_frame(&controller.borrow());
            assert_allocated(&controller.borrow().ui.footer_shell, "terminal footer");
            assert_eq!(controller.borrow().ui.heading.text(), "Setup incomplete");
            assert_action(&controller.borrow().ui.primary, "failed terminal Close");
            assert!(!controller.borrow().ui.secondary.is_visible());
            assert!(!controller.borrow().ui.upgrade.is_visible());
            assert!(!controller.borrow().ui.uninstall.is_visible());
            assert!(!controller.borrow().ui.manage.is_visible());
            assert!(!controller.borrow().ui.refresh.is_visible());
            assert_allocated(&controller.borrow().ui.results, "failed terminal results");
            assert_allocated(
                &controller.borrow().ui.outcome,
                "failure preservation guidance",
            );
            assert!(controller
                .borrow()
                .ui
                .inference
                .text()
                .contains("inference not verified"));
            destroy(controller);
        }

        // Uninstall selected during the hidden preview has the same visible,
        // single-intent inspection state and sends nothing before review completes.
        let (controller, data) = fixture();
        controller.borrow_mut().choose_uninstall();
        assert_eq!(controller.borrow().phase, Phase::Inspecting);
        assert_eq!(controller.borrow().ui.heading.text(), "Checking Smith");
        assert!(controller
            .borrow()
            .ui
            .stage
            .text()
            .contains("Preparing uninstall"));
        assert_eq!(
            controller.borrow().ui.secondary.label().as_deref(),
            Some("Cancel")
        );
        controller.borrow_mut().choose_install_or_upgrade();
        assert!(matches!(
            controller.borrow().action_after_review,
            Some(AfterReview::Command(Command::Uninstall))
        ));
        assert!(data.borrow().commands.is_empty());
        data.borrow_mut().events.push_back(review_event(490));
        controller.borrow_mut().tick(false);
        assert_eq!(data.borrow().commands, [Command::Uninstall]);
        controller.borrow_mut().stop(false);
        data.borrow_mut().events.push_back(Event::Finished {
            report: report(false, true, StepState::Skipped),
        });
        data.borrow_mut().finished = true;
        controller.borrow_mut().tick(false);
        destroy(controller);

        // A saved name outside the compiled catalog is status only: it is never
        // inserted into the chooser or submitted back to the worker.
        let (controller, data) = fixture();
        data.borrow_mut().events.push_back(review_event(500));
        controller.borrow_mut().tick(false);
        controller.borrow_mut().manage();
        data.borrow_mut().events.push_back(Event::Models {
            request_id: 501,
            selected: Some("unsupported/local-model".into()),
            installed: vec!["unsupported/local-model".into()],
        });
        controller.borrow_mut().tick(false);
        assert_eq!(controller.borrow().phase, Phase::Model);
        assert!(controller.borrow().ui.model.active_id().is_none());
        assert!(!controller.borrow().ui.primary.is_sensitive());
        assert!(controller
            .borrow()
            .ui
            .model_facts
            .text()
            .contains("Unsupported saved selection"));
        controller.borrow_mut().model_action();
        assert_eq!(data.borrow().commands, [Command::ManageModels]);
        let model = controller.borrow().ui.model.clone();
        assert!(model.set_active_id(Some(smith_service::runner::CATALOG[0].name)));
        pump_gtk();
        controller.borrow().model_selection_changed();
        assert!(controller.borrow().ui.primary.is_sensitive());
        controller.borrow_mut().model_action();
        assert!(matches!(
            &data.borrow().commands[1],
            Command::ModelAction { model, .. }
                if model == smith_service::runner::CATALOG[0].name
        ));
        controller.borrow_mut().stop(false);
        data.borrow_mut().events.push_back(Event::Finished {
            report: report(false, true, StepState::Skipped),
        });
        data.borrow_mut().finished = true;
        controller.borrow_mut().tick(false);
        destroy(controller);

        provider_cases();
        launch_cases();
        visual_matrix();
    }
}
