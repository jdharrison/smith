//! One native recipient executable with an explicitly separate artifact-only mode.
use crate::{artifact, consent, model::ProgressOutput, setup, Result};
use std::collections::HashSet;
use std::ffi::OsString;

use std::path::{Path, PathBuf};

pub const HELP: &str = "Smith native installer — Ubuntu 24.04 LTS x86_64, unprivileged user\n\n\
Usage: smith-installer [--bundle ROOT] [--dry-run] [--model EXACT]\n\
                       [--download-model] [--verify-inference]\n\
       smith-installer --artifacts-only [--client BUNDLE] [--service BINARY]\n\
                       [--dry-run] [--yes]\n\n\
  --bundle ROOT       Recipient package root (default: actual executable directory)\n\
  --dry-run           Read-only artifact plan; no locks/writes, manager or provider\n\
  --model EXACT       Exact eligible local name:tag; no default model\n\
  --download-model    Offer separate curated GGUF download consent only if missing\n\
  --models            List the pinned local model catalog (no discovery/download)\n\
  --pull-model        Compatibility alias for --download-model\n\
  --verify-inference  Offer separate fixed minimal inference, not a saved chat turn\n\
  --artifacts-only    Developer artifact publication only; never register/start\n\
  --client BUNDLE     Complete paired Flutter bundle, only with --artifacts-only\n\
  --service BINARY    Native service artifact, only with --artifacts-only\n\
  --yes              Artifact plan consent only; requires --artifacts-only\n\
  -h, --help          Show help without operational discovery\n\
  -V, --version       Show version without operational discovery\n\n\
Recipient confirmations are separate for artifacts, registration/reload, Start,\n\
exact-model download, saved preference and optional inference. No recipient --yes.\n\
CPU/Vulkan llama.cpp runtimes are bundled with CPU fallback; no Ollama, Python or Docker.\n\
No OS/GPU provisioning, upgrades, lingering changes or GUI launch.\n\
Use a prepared Ubuntu session and existing safe XDG roots.\n\
Later decline/failure preserves prior effects; no automatic retry or rollback.\n";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Recipient {
        bundle: Option<PathBuf>,
        model: Option<String>,
        pull_model: bool,
        verify_inference: bool,
    },
    Artifacts {
        client: Option<PathBuf>,
        service: Option<PathBuf>,
        assume_yes: bool,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub mode: Mode,
    pub dry_run: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Help,
    Version,
    Models,
    Run(Options),
}

/// Parse only: no current_exe, cwd, roots, files, manager, or provider discovery.
/// Values may use --flag=value as with the previous recipient CLI.
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command> {
    let mut args = args.into_iter();
    let mut seen = HashSet::new();
    let (mut bundle, mut client, mut service, mut model) = (None, None, None, None);
    let (mut dry_run, mut artifact_mode, mut assume_yes, mut pull_model, mut verify_inference) =
        (false, false, false, false, false);
    let (mut help, mut version, mut list_models) = (false, false, false);
    while let Some(argument) = args.next() {
        let text = argument.to_str().ok_or("Option names must be UTF-8")?;
        let (name, inline) = match text.split_once('=') {
            Some((name, value)) => (name, Some(OsString::from(value))),
            None => (text, None),
        };
        let name = match name {
            "--download-model" => "--pull-model",
            "-h" => "--help",
            "-V" => "--version",
            other => other,
        };
        if !seen.insert(name.to_owned()) {
            return Err(format!("Repeated option: {name}"));
        }
        let takes_value = matches!(name, "--bundle" | "--client" | "--service" | "--model");
        let value = if takes_value {
            let value = inline
                .or_else(|| args.next())
                .ok_or_else(|| format!("{name} requires a value"))?;
            if value.is_empty() || value.to_str().is_some_and(|s| s.starts_with("--")) {
                return Err(format!("{name} requires a nonempty value"));
            }
            Some(value)
        } else {
            if inline.is_some() {
                return Err(format!("{name} does not take a value"));
            }
            None
        };
        match name {
            "--help" => help = true,
            "--version" => version = true,
            "--models" => list_models = true,
            "--dry-run" => dry_run = true,
            "--artifacts-only" => artifact_mode = true,
            "--yes" => assume_yes = true,
            "--pull-model" => pull_model = true,
            "--verify-inference" => verify_inference = true,
            "--bundle" => bundle = value.map(PathBuf::from),
            "--client" => client = value.map(PathBuf::from),
            "--service" => service = value.map(PathBuf::from),
            "--model" => {
                model = Some(
                    value
                        .ok_or("Missing model")?
                        .into_string()
                        .map_err(|_| "Model must be UTF-8")?,
                )
            }
            _ => return Err(format!("Unknown argument: {name}")),
        }
    }
    if help {
        return Ok(Command::Help);
    }
    if version {
        return Ok(Command::Version);
    }
    if list_models {
        if seen.len() != 1 {
            return Err("--models cannot be combined with operational options".into());
        }
        return Ok(Command::Models);
    }
    let mode = if artifact_mode {
        if bundle.is_some() || model.is_some() || pull_model || verify_inference {
            return Err("--artifacts-only rejects recipient --bundle/--model/--pull-model/--verify-inference".into());
        }
        if client.is_none() && service.is_none() {
            return Err("--artifacts-only requires --client BUNDLE and/or --service BINARY".into());
        }
        Mode::Artifacts {
            client,
            service,
            assume_yes,
        }
    } else {
        if client.is_some() || service.is_some() || assume_yes {
            return Err("--client/--service/--yes require explicit --artifacts-only; no recipient consent bypass".into());
        }
        if (pull_model || verify_inference) && model.is_none() {
            return Err("--pull-model/--verify-inference require --model EXACT".into());
        }
        Mode::Recipient {
            bundle,
            model,
            pull_model,
            verify_inference,
        }
    };
    Ok(Command::Run(Options { mode, dry_run }))
}

/// Resolve the actual running ELF, not argv[0], cwd, PATH or a symlink alias.
pub fn default_bundle() -> Result<PathBuf> {
    let executable =
        std::env::current_exe().map_err(|_| "Cannot locate actual running installer")?;
    executable_parent(&executable)
}
pub fn executable_parent(executable: &Path) -> Result<PathBuf> {
    let actual = executable
        .canonicalize()
        .map_err(|_| "Cannot canonicalize actual installer executable")?;
    actual
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "Actual installer executable has no parent".into())
}

pub fn execute(
    options: Options,
    confirm: &mut dyn FnMut(&str) -> Result<String>,
    output: &mut dyn ProgressOutput,
) -> Result<artifact::Outcome> {
    match options.mode {
        Mode::Recipient {
            bundle,
            model,
            pull_model,
            verify_inference,
        } => {
            let bundle = match bundle {
                Some(root) => absolute(root)?,
                None => default_bundle()?,
            };
            setup::run(
                &setup::Options {
                    bundle,
                    model,
                    pull_model,
                    verify_inference,
                    dry_run: options.dry_run,
                },
                confirm,
                output,
            )
        }
        Mode::Artifacts {
            client,
            service,
            assume_yes,
        } => {
            setup::prerequisites()?;
            let client = client.map(absolute).transpose()?;
            let service = service.map(absolute).transpose()?;
            artifact::Installer::new(client, service)?.run(options.dry_run, assume_yes, confirm)
        }
    }
}
fn absolute(path: PathBuf) -> Result<PathBuf> {
    // Explicit relative paths are supported; publisher validation still rejects
    // non-normalized/unsafe paths. The default never consults cwd.
    std::path::absolute(path).map_err(|_| "Cannot resolve explicit input path".into())
}

/// Exit 0: inert help/version, dry-run, success or initial artifact decline;
/// 2: argument errors; 1: semantic/input/cancellation/later-decline/partial failure.
pub fn main(args: impl IntoIterator<Item = OsString>) -> i32 {
    let command = match parse(args) {
        Ok(command) => command,
        Err(error) => {
            let _ = consent::diagnostic(&format!(
                "smith-installer: {}\nUse --help for usage.",
                consent::display(&error)
            ));
            return 2;
        }
    };
    match command {
        Command::Help => exit_status(consent::stdout_text(HELP)),
        Command::Version => exit_status(consent::stdout_text(&format!(
            "smith-installer {}\n",
            env!("CARGO_PKG_VERSION")
        ))),
        Command::Models => {
            let catalog = smith_service::runner::CATALOG
                .iter()
                .map(|entry| {
                    format!(
                        "{} · {} · {} · {:.2} GiB — {} — {}, SHA256 {}",
                        entry.tier.label(),
                        entry.display_name,
                        entry.parameter_label,
                        entry.size as f64 / 1024_f64.powi(3),
                        entry.name,
                        entry.license,
                        entry.sha256
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            exit_status(consent::stdout_text(&format!(
                "Pinned local GGUF catalog (weights not bundled):\n{catalog}\n"
            )))
        }
        Command::Run(options) => {
            if let Err(error) = consent::install_cancellation() {
                return exit_status::<()>(Err(error));
            }
            // Each console write scopes/restores the native descriptor flags.
            // No stdout lock or FdProgress survives across publisher stdio calls.
            exit_status(execute(
                options,
                &mut consent::prompt,
                &mut consent::StdoutConsole,
            ))
        }
    }
}

/// Never let final diagnostics re-enter an unbounded stderr write/flush after a
/// failed/cancelled model stage. An unwritable report cannot turn failure into
/// success or imply that previously accepted effects were rolled back.
pub fn exit_status<T>(result: Result<T>) -> i32 {
    match result {
        Ok(_) => 0,
        Err(error) => {
            let _ = consent::diagnostic(&format!("smith-installer: {}", consent::display(&error)));
            1
        }
    }
}
