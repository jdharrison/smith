//! Optional GTK3 recipient shell. The worker owns all publication/Core state;
//! this binary only displays reviews and sends explicit, request-bound decisions.
use gtk::glib::{self, ControlFlow, Propagation};
use gtk::prelude::*;
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
       smith-installer-ui --help | --version\n\n\
  --bundle ROOT  Complete recipient package (default: actual executable directory)\n\
  -h, --help     Print this finite document; no display or operational discovery\n\
  -V, --version  Print version; no display or operational discovery\n\n\
Requires compatible GTK3 libraries and a graphical session for installation.\n\
Includes the Smith app, menu/icon, user command and standalone service binary.\n\
Install approves only the exact displayed artifact review. Register Smith service\n\
and Start now are requested by default, with separate inline confirmations.\n\
Model choice/download and a separately confirmed service-backed check follow.\n\
Manage models bypasses artifact publication; exact-model removal is service-mediated.\n\
CPU/Vulkan model runtimes and CPU fallback are bundled; no Ollama dependency. No OS/GPU provisioning, implicit upgrades or lingering changes.\n\
No automatic app launch or rollback; stopping preserves accepted/uncertain effects.\n\
The headless smith-installer CLI is separate and unchanged.\n";
const CSS: &str = "
window.smith-installer, window.smith-stop-dialog {
    background-color: #101713;
    color: #edf4ee;
}
.smith-installer scrolledwindow, .smith-installer viewport, .smith-installer stack {
    background-color: #101713;
}
.smith-installer expander { color: #edf4ee; }
.smith-installer label, .smith-stop-dialog label { color: #edf4ee; }
.smith-installer .title { font-size: 22px; font-weight: bold; }
.smith-installer .section { font-weight: bold; color: #b9e8c8; }
.smith-installer button, .smith-stop-dialog button {
    background-image: none;
    background-color: #243a2d;
    color: #edf4ee;
    border-color: #527a60;
    padding: 8px 14px;
}
.smith-installer button.suggested-action { background-color: #285e3b; }
.smith-installer button.landing-action { font-size: 18px; font-weight: bold; }
.smith-installer button:hover, .smith-stop-dialog button:hover { background-color: #365d43; }
.smith-installer button:disabled { background-color: #202923; color: #a8b2aa; }
.smith-installer button:disabled label { color: #a8b2aa; }
.smith-installer button:focus, .smith-stop-dialog button:focus { outline-color: #b9e8c8; }
.smith-installer check { background-color: #18261d; border-color: #80ae90; }
.smith-installer check:checked { background-color: #347349; color: #ffffff; }
.smith-installer checkbutton:disabled label { color: #a8b2aa; }
.smith-installer progressbar trough { background-color: #202e25; }
.smith-installer progressbar progress { background-color: #73ba8a; }
";

#[derive(Debug, PartialEq, Eq)]
enum Launch {
    Help,
    Version,
    Gui { bundle: Option<PathBuf> },
}

// Parsing is deliberately independent of executable/display/root discovery.
fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Launch> {
    let mut args = args.into_iter();
    let (mut bundle, mut help, mut version) = (None, false, false);
    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("--help" | "-h") if !help => help = true,
            Some("--version" | "-V") if !version => version = true,
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
        window.set_title("Smith Installer");
        window.set_default_size(720, 560);
        window.set_resizable(true);
        window.style_context().add_class("smith-installer");

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let scroll = gtk::ScrolledWindow::new(None::<&gtk::Adjustment>, None::<&gtk::Adjustment>);
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        let body = gtk::Box::new(gtk::Orientation::Vertical, 16);
        body.set_border_width(24);
        let heading = label("Set up Smith");
        heading.style_context().add_class("title");
        let introduction = label("Install or update the Smith app and service, or remove its active integration while keeping your data.");
        body.pack_start(&heading, false, false, 0);
        body.pack_start(&introduction, false, false, 0);

        let stack = gtk::Stack::new();
        stack.set_homogeneous(false);
        let landing = gtk::Box::new(gtk::Orientation::Vertical, 16);
        let upgrade = gtk::Button::with_label("INSTALL / UPGRADE");
        let uninstall = gtk::Button::with_label("UNINSTALL");
        for button in [&upgrade, &uninstall] {
            button.set_size_request(-1, 88);
            button.style_context().add_class("landing-action");
            landing.pack_start(button, false, false, 0);
        }
        stack.add_named(&landing, "landing");
        let choices = gtk::Box::new(gtk::Orientation::Vertical, 12);
        choices.pack_start(&section("Install Smith"), false, false, 0);
        choices.pack_start(&label("Install the reviewed Smith app and service with the normal defaults: register it, enable it at login, then start it now. Each exact effect still requires its own confirmation; lingering is unchanged."), false, false, 0);
        let register = gtk::CheckButton::with_label("Register Smith service");
        register.set_active(true);
        let start = gtk::CheckButton::with_label("Start now");
        start.set_active(true);
        let recipe = label("");
        let review = label("Reading the package and preparing a read-only review…");
        let artifact_details = gtk::Expander::new(Some("Reviewed artifact details"));
        artifact_details.add(&review);
        choices.pack_start(&artifact_details, false, false, 0);
        stack.add_named(&choices, "review");
        let model_page = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let model = gtk::ComboBoxText::new();
        let remove = gtk::CheckButton::with_label("Remove this installed model (destructive)");
        let model_facts = label("");
        model_page.pack_start(&section("Choose a model"), false, false, 0);
        model_page.pack_start(&model, false, false, 0);
        model_page.pack_start(&model_facts, false, false, 0);
        model_page.pack_start(&remove, false, false, 0);
        model_page.pack_start(&label("Downloads, checked preference changes, service inference and destructive removal each require separate exact-model confirmation. Existing selection is preserved. CPU performance is unmeasured."), false, false, 0);
        stack.add_named(&model_page, "model");

        let working = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let stage = label("Preparing installation…");
        let elapsed = label("");
        let progress = gtk::ProgressBar::new();
        working.pack_start(&stage, false, false, 0);
        working.pack_start(&progress, false, false, 0);
        working.pack_start(&elapsed, false, false, 0);
        let confirmation = gtk::Box::new(gtk::Orientation::Vertical, 12);
        confirmation.set_no_show_all(true);
        let confirmation_title = section("");
        let effects = label("");
        confirmation.pack_start(&confirmation_title, false, false, 0);
        confirmation.pack_start(&label("Confirm only these exact effects, or Stop here and preserve earlier outcomes. This preparation expires; it will not be renewed automatically."), false, false, 0);
        confirmation.pack_start(&effects, false, false, 0);
        working.pack_start(&confirmation, false, false, 0);
        stack.add_named(&working, "working");

        let terminal = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let results = gtk::Box::new(gtk::Orientation::Vertical, 10);
        let outcome = label("");
        let inference = label("");
        terminal.pack_start(&results, false, false, 0);
        terminal.pack_start(&outcome, false, false, 0);
        terminal.pack_start(&inference, false, false, 0);
        stack.add_named(&terminal, "terminal");
        body.pack_start(&stack, false, false, 0);

        // Retain bounded diagnostics for report ordering and tests, but do not
        // attach them to the normal installer page. A future explicit debug
        // surface can expose them without creating below-the-fold UI here.
        let details = label("");
        scroll.add(&body);
        root.pack_start(&scroll, true, true, 0);
        let footer_shell = gtk::Box::new(gtk::Orientation::Vertical, 0);
        footer_shell.pack_start(
            &gtk::Separator::new(gtk::Orientation::Horizontal),
            false,
            false,
            0,
        );
        let footer = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        footer.set_border_width(16);
        let refresh = gtk::Button::with_label("Refresh review");
        let manage = gtk::Button::with_label("Manage models");
        let secondary = gtk::Button::with_label("Cancel");
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
            label.set_line_wrap_mode(gtk::pango::WrapMode::WordChar);
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
    cancellation: Box<dyn Fn() -> bool>,
    restart_blocked: bool,
}

impl Controller {
    fn new(ui: Widgets, session: Option<Box<dyn SessionIo>>) -> Self {
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
            cancellation: Box::new(consent::cancelled),
            restart_blocked: false,
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
            self.ui.introduction.set_text("Install the reviewed Smith app and service. Registration, login Enable and Start use the fixed normal recipe and retain separate exact confirmations.");
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
        if self.management_blocked() {
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
                    self.report = None;
                    self.fault = None;
                    self.pending = None;
                    self.stopped_by_user = false;
                    self.close_after_join = false;
                    self.active_scope = match &command {
                        Command::Upgrade => Scope::Upgrade,
                        Command::Uninstall => Scope::Uninstall,
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
        self.stopped_by_user = true;
        self.phase = Phase::Stopping;
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
            "Uninstall complete. ALL app/Core state, preferences, history, model data, releases and backups are retained. Disk usage is not purged."
        } else if complete && self.active_scope == Scope::Upgrade {
            "Upgrade complete; no model download or readiness check. Choose Manage models separately to continue. Keep the complete package."
        } else if complete {
            "Selected effects are verified; see the exact model check/removal result below. Keep the complete recipient package for future management."
        } else if self.choices.is_some() {
            "Completed and uncertain effects are preserved; stopping is not rollback. No automatic retry or cleanup was performed. Keep the package and inspect the reported outcomes before attempting repair."
        } else {
            "Install was not authorized. No artifact or service effect was requested by this window. Keep the package and correct the reported problem before reopening the installer."
        });
        let checked = complete
            && self.report.as_ref().is_some_and(|r| {
                r.steps.iter().any(|s| {
                    s.label == "Model setup"
                        && s.state == StepState::Completed
                        && s.detail.contains("inference verified")
                })
            });
        self.ui.inference.set_text(if checked { "Inference: Verified through Smith service.\nThe Smith app is not opened automatically." } else { "Model inference not verified by this session.\nThe Smith app is not opened automatically." });
        self.ui.results.show_all();
        self.quit |= self.close_after_join;
        self.render();
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
        self.ui.refresh.set_visible(false);
        self.ui.footer_shell.set_visible(!landing);
        self.ui.secondary.set_visible(!landing);
        self.ui.secondary.set_sensitive(matches!(
            self.phase,
            Phase::Review | Phase::Model | Phase::Working | Phase::Confirmation
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
                self.ui.introduction.set_text("Install or update the Smith app and service, or remove its active integration while keeping your data.");
                self.ui.primary.set_visible(false);
                self.ui.secondary.set_visible(false);
            }
            Phase::Inspecting => {
                self.ui.heading.set_text("Checking Smith");
                self.ui.introduction.set_text(if self.active_scope == Scope::Uninstall {
                    "Preparing the independently checked uninstall flow. No effect starts while Smith is being inspected."
                } else {
                    "Reviewing the package and existing Smith installation. No effect starts while Smith is being inspected."
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
                self.ui.introduction.set_text("Choose/install a model, then check it through the Smith service. Nothing is changed merely by opening this step.");
                self.ui.primary_label("Continue with exact model");
                self.ui.primary.set_sensitive(
                    self.model_request.is_some() && self.ui.model.active_id().is_some(),
                );
                self.ui.secondary.set_label("Cancel model operation…");
            }
            Phase::Working => {
                self.ui.primary.set_visible(true);
                self.ui.heading.set_text(match self.active_scope {
                    Scope::Upgrade => "Upgrading Smith",
                    Scope::Uninstall => "Uninstalling Smith",
                    Scope::ModelCheck | Scope::ModelRemoval => "Managing models",
                    Scope::Installation => "Installing Smith",
                });
                self.ui.introduction.set_text("Only explicitly requested and separately confirmed effects may proceed. Completed and uncertain effects are preserved on failure/cancellation; no rollback or retry.");
                self.ui.primary_label("Installing…");
                self.ui.primary.set_sensitive(false);
                self.ui.secondary.set_label("Cancel operation…");
            }
            Phase::Confirmation => {
                self.ui.primary.set_visible(true);
                if let Some(pending) = &self.pending {
                    self.ui.primary_label(&format!("Confirm {}", pending.title));
                }
                self.ui.primary.set_sensitive(self.pending.is_some());
                self.ui.secondary.set_label("Stop here");
            }
            Phase::Stopping | Phase::Finishing => {
                self.ui.primary.set_visible(true);
                let stopping = self.phase == Phase::Stopping;
                self.ui.heading.set_text(if stopping {
                    "Cancelling operation…"
                } else {
                    "Finishing operation…"
                });
                self.ui.stage.set_text(if stopping {
                    "Waiting for the installer worker to stop. Accepted effects remain; filesystem writes may take time. This window stays responsive and will not abandon the worker."
                } else {
                    "Waiting for the installer worker to exit before showing its final result."
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
                self.ui.primary.set_sensitive(self.session.is_none());
                self.ui.secondary.set_visible(
                    !self.management_blocked() && self.bundle.is_some() && self.session.is_none(),
                );
                self.ui.secondary.set_label("Manage models");
                self.ui
                    .secondary
                    .set_sensitive(self.ui.secondary.is_visible());
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
        Phase::Terminal => controller.borrow_mut().quit = true,
        Phase::Landing | Phase::Inspecting | Phase::Review => controller.borrow_mut().stop(true),
        Phase::Stopping | Phase::Finishing => controller.borrow_mut().close_after_join = true,
        Phase::Model | Phase::Working | Phase::Confirmation => ask_stop(controller, true),
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
                Phase::Confirmation => state.approve(),
                Phase::Terminal => state.quit = true,
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
    let controller = Rc::new(RefCell::new(Controller::new(ui, session)));
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

    fn fixture() -> (Rc<RefCell<Controller>>, Rc<RefCell<FakeData>>) {
        let data = Rc::new(RefCell::new(FakeData::default()));
        let controller = Rc::new(RefCell::new(Controller::new(
            Widgets::new(),
            Some(Box::new(FakeSession(data.clone()))),
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

    // Keep all GTK scenarios in one test: GTK must stay on the initialization
    // thread. Run under isolated Xvfb; no production session/host effect is used.
    #[test]
    fn gtk_widget_smoke_and_session_transitions() {
        gtk::init().expect("GTK smoke requires an isolated Xvfb display");
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
            Some("Confirm Register Smith service")
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
    }
}
