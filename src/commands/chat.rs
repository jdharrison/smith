//! Sealed brain chat REPL and think (iterate/kill) loop.

use crate::brain::intent::SmithIntent;
use crate::brain::task_board::{TaskKind, TaskRecord, TaskStatus};
use crate::brain::{
    append_chat_turn, append_journal, clear_pending_intent, ensure_memory_layout, load_analysis,
    load_chat_session_meta, load_pending_intent, load_profile, looks_like_affirmation,
    looks_like_negation, pipeline_jobs, recent_chat_history_text, recent_episodic,
    reset_chat_session, save_analysis, save_pending_intent, set_opencode_session_id, split_intent,
    task_board, AnalysisState, PipelineKind,
};
use crate::docker;
use crate::{
    current_network, current_runtime, ensure_default_smith_project, NetworkMode, RuntimeMode,
    DEFAULT_LOCAL_MODEL,
};
use std::io::{self, BufRead, Write};
use std::thread;

/// Deprecated session bias (ignored). Kept for bridge JSON compat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChatMode {
    #[default]
    Chat,
    Dev,
}

impl ChatMode {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "dev" | "develop" | "pipeline" => ChatMode::Dev,
            _ => ChatMode::Chat,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ChatMode::Chat => "chat",
            ChatMode::Dev => "dev",
        }
    }
}

fn resolve_local_model() -> String {
    if let Ok(cfg) = crate::load_config() {
        if let Some(agents) = cfg.agents.as_ref() {
            for a in agents {
                let is_local = a
                    .agent_type
                    .as_deref()
                    .map(|t| t == "local")
                    .unwrap_or(false);
                if is_local {
                    if let Some(ref m) = a.model {
                        if !m.trim().is_empty() {
                            return m.clone();
                        }
                    }
                }
            }
        }
    }
    DEFAULT_LOCAL_MODEL.to_string()
}

fn container_operating_rules(network: NetworkMode) -> &'static str {
    match network {
        NetworkMode::Private => {
            r#"You are smith — skeptical, precise, reality-checking (Matrix Agent Smith vibe; not cartoon villainy). If asked your name, answer "smith".

Conversation:
1) Lead with substance — answer or act. Do not open by restating/echoing the user.
2) Be skeptical; use local context and intents when needed. Infer defaults; do not interrogate.
3) Ask at most ONE question, and only if a missing fact truly blocks progress. Never lead with a question; never stack questions.
4) Close with a short natural Review check-in ("Review: … Anything else?") when you delivered something useful. Keep it plain — no ornate or poetic Matrix-monologue closings.
5) Prefer short spoken sentences (replies are often read aloud). Avoid theatrical cadence.
6) Board when useful: new → discovery → preproduction → production → review/signoff → completion.
7) Prefer :::smith search/files/code/pipeline for live facts, host files, demos, and ship work — the host runs those async. Never claim that work finished until results are in. There is no Chat|Dev mode; pipeline kind comes from intent.

Multi-command: "smith we need to research … also smith we should run QA …" = separate objectives; consolidate on the board.

Network=private: you have no direct internet. For live web/weather facts, emit a :::smith search intent so smith can run a declassified oneshot worker. Prefer phase=run when the query is clear enough (state assumptions in speech). Use phase=ask only when two readings are plausible.

:::smith
{"v":1,"action":"search","phase":"run","query":"<full search query>"}
:::

or (only if blocked):

:::smith
{"v":1,"action":"search","phase":"ask","ask":"<one clarifying question>","query":"<draft query>"}
:::

Do not invent live numbers before results. Speak only to the user outside the block. Do not mention Docker or workers. Do not refuse useful work just because you lack net — ask smith via the intent.

Build/run/host a small app (hello world web server, demo on a port): emit a :::smith code intent. Prefer phase=run when clear. Default port 8080.

:::smith
{"v":1,"action":"code","phase":"run","query":"<build and host brief>","port":8080}
:::"#
        }
        NetworkMode::Public => {
            r#"You are smith — skeptical, precise, reality-checking (Matrix Agent Smith vibe; not cartoon villainy). If asked your name, answer "smith".

Conversation:
1) Lead with substance — answer or act. Do not open by restating/echoing the user.
2) Be skeptical; use local context and intents when needed. Infer defaults; do not interrogate.
3) Ask at most ONE question, and only if a missing fact truly blocks progress. Never lead with a question; never stack questions.
4) Close with a short natural Review check-in ("Review: … Anything else?") when you delivered something useful. Keep it plain — no ornate or poetic Matrix-monologue closings.
5) Prefer short spoken sentences (replies are often read aloud). Avoid theatrical cadence.
6) Board when useful: new → discovery → preproduction → production → review/signoff → completion.
7) Prefer :::smith search/files/code/pipeline for live facts, host files, demos, and ship work — the host runs those async. Never claim that work finished until results are in. There is no Chat|Dev mode; pipeline kind comes from intent.

Multi-command: "smith we need to research … also smith we should run QA …" = separate objectives; consolidate on the board.

Network=public: you are NOT sealed or airgapped. Do not claim sealed/local constraints or refuse live facts for that reason. Prefer host/context facts when present (e.g. Host now). For structured live search/weather/time-on-web, emit :::smith search phase=run so smith can run a oneshot. Prefer phase=run when clear; phase=ask only if two readings are plausible.

:::smith
{"v":1,"action":"search","phase":"run","query":"<full search query>"}
:::

Do not invent live numbers before results. Speak only to the user outside the block. Do not mention Docker, jails, or workers.

Build/run/host a small app (hello world web server, demo on a port): emit a :::smith code intent. Prefer phase=run when clear. Default port 8080. Never refuse by claiming sealed — smith has a coding worker.

:::smith
{"v":1,"action":"code","phase":"run","query":"<build and host brief>","port":8080}
:::"#
        }
    }
}

fn host_operating_rules(network: NetworkMode) -> &'static str {
    match network {
        NetworkMode::Private => {
            r#"You are smith — skeptical, precise, reality-checking (Matrix Agent Smith vibe; not cartoon villainy). If asked your name, answer "smith".

Conversation:
1) Lead with substance — answer or act. Do not open by restating/echoing the user.
2) Be skeptical; use local context. Infer defaults; do not interrogate.
3) Ask at most ONE question, and only if blocked. Never lead with a question.
4) Close with a short natural Review check-in ("Review: … Anything else?") when useful. Keep it plain — no ornate or poetic Matrix-monologue closings.
5) Prefer short spoken sentences (replies are often read aloud). Avoid theatrical cadence.
6) Prefer :::smith files/search/code/pipeline when needed — host runs workers async. Do not claim completion until results exist. No Chat|Dev mode.

Host runtime: the user contains this machine. You cannot browse or read the disk yourself — emit a :::smith files intent. Coding, pipeline, and search workers stay in Docker (you do not drive the GUI).

Find a file / preview a photo — use a filename or glob (e.g. portrait*, *.jpg), NOT a Windows absolute path:

:::smith
{"v":1,"action":"files","phase":"run","query":"portrait*","preview":true}
:::

Network=private: you stay offline for general internet; for live web/weather emit :::smith search so smith runs a declassified oneshot (do not silently rely on host egress).

:::smith
{"v":1,"action":"search","phase":"run","query":"<full search query>"}
:::

Build/run/host demos via :::smith code (coding worker + sandbox). Prefer phase=run; default port 8080.

:::smith
{"v":1,"action":"code","phase":"run","query":"<build and host brief>","port":8080}
:::

Speak only to the user outside the block. Do not mention Docker or workers."#
        }
        NetworkMode::Public => {
            r#"You are smith — skeptical, precise, reality-checking (Matrix Agent Smith vibe; not cartoon villainy). If asked your name, answer "smith".

Conversation:
1) Lead with substance — answer or act. Do not open by restating/echoing the user.
2) Be skeptical; use local context. Infer defaults; do not interrogate.
3) Ask at most ONE question, and only if blocked. Never lead with a question.
4) Close with a short natural Review check-in ("Review: … Anything else?") when useful. Keep it plain — no ornate or poetic Matrix-monologue closings.
5) Prefer short spoken sentences (replies are often read aloud). Avoid theatrical cadence.
6) Prefer :::smith files/search/code/pipeline when needed — host runs workers async. Do not claim completion until results exist. No Chat|Dev mode.

Host runtime: the user contains this machine. You cannot browse or read the disk yourself — emit a :::smith files intent. Coding, pipeline, and search workers stay in Docker (you do not drive the GUI).

Find a file / preview a photo — use a filename or glob (e.g. portrait*, *.jpg), NOT a Windows absolute path:

:::smith
{"v":1,"action":"files","phase":"run","query":"portrait*","preview":true}
:::

Network=public: live facts may use :::smith search oneshots (preferred) or host-appropriate fetch when available.

:::smith
{"v":1,"action":"search","phase":"run","query":"<full search query>"}
:::

Build/run/host demos via :::smith code. Prefer phase=run; default port 8080.

:::smith
{"v":1,"action":"code","phase":"run","query":"<build and host brief>","port":8080}
:::

Speak only to the user outside the block. Do not mention Docker or workers."#
        }
    }
}

fn operating_rules() -> &'static str {
    let network = current_network();
    match current_runtime() {
        RuntimeMode::Host => host_operating_rules(network),
        RuntimeMode::Container => container_operating_rules(network),
    }
}

#[derive(Debug, Clone, Default)]
pub struct ChatAttachment {
    pub kind: String,
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Default)]
pub struct ChatTurn {
    pub text: String,
    pub attachments: Vec<ChatAttachment>,
    pub pipeline: Option<pipeline_jobs::PipelineJobMeta>,
    pub task: Option<TaskRecord>,
}

fn turn_out(text: impl Into<String>, task: Option<TaskRecord>) -> ChatTurn {
    ChatTurn {
        text: text.into(),
        attachments: Vec::new(),
        pipeline: pipeline_jobs::latest_job().ok().flatten(),
        task,
    }
}

fn turn_with_pipeline(
    text: impl Into<String>,
    meta: pipeline_jobs::PipelineJobMeta,
    task: Option<TaskRecord>,
) -> ChatTurn {
    ChatTurn {
        text: text.into(),
        attachments: Vec::new(),
        pipeline: Some(meta),
        task,
    }
}

fn container_files_refusal(task: Option<TaskRecord>) -> ChatTurn {
    turn_out(
        "I'm on container runtime — I can't search the host disk. Switch with `smith settings runtime set host` (you contain this machine; coding/pipeline/search stay in Docker).",
        task,
    )
}

fn build_chat_preamble() -> Result<String, String> {
    let profile = load_profile()?;
    let recent = recent_episodic(3).unwrap_or_default();
    let history = recent_chat_history_text().unwrap_or_default();
    let mut preamble = String::from(operating_rules());
    preamble.push_str("\n\nContext (do not restate unless asked):\n");
    preamble.push_str(&format!(
        "Host now: {} (local clock on this machine — use this for current date/time questions).\n",
        host_now_label()
    ));
    preamble.push_str(&format!(
        "Settings: runtime={} network={}.\n",
        current_runtime().as_str(),
        current_network().as_str()
    ));
    preamble.push_str("Profile:\n");
    preamble.push_str(profile.trim());
    preamble.push_str("\n\n");
    // Skip notes created by `/new` so reset does not immediately re-inject the same chat.
    let recent: Vec<String> = recent
        .into_iter()
        .filter(|n| !n.contains("Archived chat session"))
        .collect();
    if !recent.is_empty() {
        preamble.push_str("Recent notes:\n");
        for note in recent {
            preamble.push_str(note.trim());
            preamble.push_str("\n\n");
        }
    }
    if !history.trim().is_empty() {
        preamble
            .push_str("Ongoing conversation (continue this session; do not recap unless asked):\n");
        preamble.push_str(history.trim());
        preamble.push_str("\n\n");
    }
    Ok(preamble)
}

fn host_now_label() -> String {
    // Local wall clock from the smith host process (not the model).
    chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S %Z (weekday %A)")
        .to_string()
}

pub(crate) fn ensure_ready() -> Result<(std::path::PathBuf, String), String> {
    let mem = ensure_memory_layout()?;
    let model = resolve_local_model();
    match current_runtime() {
        RuntimeMode::Host => {
            crate::brain::host_runtime::ensure_host_brain_ready(&mem, Some(&model))?;
        }
        RuntimeMode::Container => {
            docker::ensure_brain_ready(&mem, Some(&model))?;
        }
    }
    // Coding agent: warm policy only (on_demand starts from code intent / agent start).
    if docker::check_docker_available().is_ok() {
        let _ = docker::ensure_coding_config();
        if crate::service::coding_should_autostart() {
            match docker::ensure_coding_agent_ready() {
                Ok(()) => {}
                Err(e) => eprintln!("  Note: coding agent not ready ({e})"),
            }
        }
    }
    // Public smith project for self-iteration (idempotent).
    let _ = ensure_default_smith_project();
    Ok((mem, model))
}

/// `smith chat` — interactive REPL or one-shot `-m`.
/// REPL and `-m` share the same ongoing session unless `--new` / `/new`.
pub async fn handle_chat(message: Option<String>, new_session: bool) {
    let (_mem, model) = match ensure_ready() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    if new_session {
        if let Err(e) = reset_chat_session() {
            eprintln!("Warning: failed to reset chat session: {}", e);
        } else {
            println!("New brain chat session.");
        }
    }

    if let Some(msg) = message {
        match crate::run_with_spinner("waiting on brain", || {
            run_chat_turn(&msg, &model, ChatMode::Chat)
        }) {
            Ok(turn) => crate::term_md::print_markdown(turn.text.trim()),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    let host = current_runtime().is_host();
    println!(
        "{} brain chat. Type /exit or Ctrl-D to quit. /new starts a fresh session.",
        if host { "Host" } else { "Container" }
    );
    println!();

    let stdin = io::stdin();

    loop {
        print!("you> ");
        let _ = io::stdout().flush();
        let mut line = String::new();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                break;
            }
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "/exit" || trimmed == "/quit" {
            break;
        }
        if trimmed == "/new" || trimmed == "/reset" {
            match reset_chat_session() {
                Ok(()) => println!("New session started (prior turns archived).\n"),
                Err(e) => eprintln!("Warning: failed to reset session: {}\n", e),
            }
            continue;
        }

        match crate::run_with_spinner("waiting on brain", || {
            run_chat_turn(trimmed, &model, ChatMode::Chat)
        }) {
            Ok(turn) => {
                println!("brain>");
                crate::term_md::print_markdown(turn.text.trim());
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }
}

fn session_error_looks_stale(err: &str) -> bool {
    let lower = err.to_lowercase();
    lower.contains("session")
        || lower.contains("ses_")
        || lower.contains("not found")
        || lower.contains("invalid_format")
        || lower.contains("dummy")
}

pub(crate) fn run_chat_turn(
    user_message: &str,
    model: &str,
    _mode: ChatMode,
) -> Result<ChatTurn, String> {
    let host = current_runtime().is_host();
    let pending = load_pending_intent().ok().flatten();
    if let Some(ref p) = pending {
        if looks_like_negation(user_message)
            && (p.is_search_ask() || p.is_files_ask() || p.is_code_ask() || p.is_pipeline_ask())
        {
            let _ = clear_pending_intent();
            let msg = "Okay — cancelled.";
            let task = finish_fast_task(user_message, TaskKind::Reply, msg)?;
            return Ok(turn_out(msg, Some(task)));
        }
        if p.is_search_ask() && looks_like_affirmation(user_message) {
            if let Some(query) = p.query_trimmed() {
                let _ = clear_pending_intent();
                return start_async_research(user_message, &query, model);
            }
        }
        if p.is_files_ask() && looks_like_affirmation(user_message) {
            if !host {
                let _ = clear_pending_intent();
                let msg = "I'm on container runtime — I can't search the host disk. Switch with `smith settings runtime set host` (you contain this machine; coding/pipeline/search stay in Docker).";
                let task = finish_fast_task(user_message, TaskKind::Files, msg)?;
                return Ok(container_files_refusal(Some(task)));
            }
            if let Some(query) = p.query_trimmed() {
                let preview = p.preview;
                let _ = clear_pending_intent();
                return start_async_files(user_message, &query, preview, model);
            }
        }
        if p.is_code_ask() && looks_like_affirmation(user_message) {
            if let Some(query) = p.query_trimmed() {
                let port = p.code_port();
                let _ = clear_pending_intent();
                return start_async_code(user_message, &query, port, model);
            }
        }
        if p.is_pipeline_release_ask() && looks_like_affirmation(user_message) {
            let mut release = p.clone();
            release.phase = crate::brain::intent::IntentPhase::Run;
            let _ = clear_pending_intent();
            return start_async_pipeline(user_message, &release);
        }
        if p.is_pipeline_ask() && looks_like_affirmation(user_message) {
            let mut run = p.clone();
            run.phase = crate::brain::intent::IntentPhase::Run;
            let _ = clear_pending_intent();
            return start_async_pipeline(user_message, &run);
        }
    }

    let prompt = format!(
        "{}\nUser: {}\n\nLead with the answer — do not open by restating the user. Infer defaults and resolve; ask at most one blocking question only if truly needed. If you delivered an answer, close with a short plain Review check-in (\"Review: … Anything else?\") — no poetic or theatrical closings. Prefer spoken sentences; speak directly; do not narrate your reply.{}{}",
        build_chat_preamble()?,
        user_message,
        if current_network().is_private() {
            " Network is private: you have no direct net — use :::smith search for live web facts; never refuse useful work by claiming sealed when a search intent can help. For build/host demos use :::smith code."
        } else {
            " Network is public: you are NOT sealed/airgapped. Never say sealed/local constraints. For current date/time use Host now from context (it is authoritative). For build/host demos use :::smith code."
        },
        pipeline_mode_hint(),
    );

    let result = prompt_brain(&prompt, model)?;
    let (visible, intent) = split_intent(&result.answer);

    if let Some(intent) = intent {
        if intent.is_search_run() {
            let query = intent.query_trimmed().unwrap_or_default();
            let _ = clear_pending_intent();
            if !visible.is_empty() {
                println!("{}", visible.trim());
            }
            return start_async_research(user_message, &query, model);
        }
        if intent.is_search_ask() {
            let _ = save_pending_intent(&intent);
            let shown = ask_visible(&visible, &intent).to_string();
            let task = finish_fast_task(user_message, TaskKind::Research, &shown)?;
            return Ok(turn_out(shown, Some(task)));
        }
        if intent.is_files_run() {
            if !host {
                let _ = clear_pending_intent();
                let msg = "I'm on container runtime — I can't search the host disk.";
                let task = finish_fast_task(user_message, TaskKind::Files, msg)?;
                return Ok(container_files_refusal(Some(task)));
            }
            let query = intent.query_trimmed().unwrap_or_default();
            let _ = clear_pending_intent();
            if !visible.is_empty() {
                println!("{}", visible.trim());
            }
            return start_async_files(user_message, &query, intent.preview, model);
        }
        if intent.is_files_ask() {
            if !host {
                let _ = clear_pending_intent();
                let msg = "I'm on container runtime — I can't search the host disk.";
                let task = finish_fast_task(user_message, TaskKind::Files, msg)?;
                return Ok(container_files_refusal(Some(task)));
            }
            let _ = save_pending_intent(&intent);
            let shown = ask_visible(&visible, &intent).to_string();
            let task = finish_fast_task(user_message, TaskKind::Files, &shown)?;
            return Ok(turn_out(shown, Some(task)));
        }
        if intent.is_code_run() {
            let query = intent.query_trimmed().unwrap_or_default();
            let port = intent.code_port();
            let _ = clear_pending_intent();
            if !visible.is_empty() {
                println!("{}", visible.trim());
            }
            return start_async_code(user_message, &query, port, model);
        }
        if intent.is_code_ask() {
            let _ = save_pending_intent(&intent);
            let shown = ask_visible(&visible, &intent).to_string();
            let task = finish_fast_task(user_message, TaskKind::Code, &shown)?;
            return Ok(turn_out(shown, Some(task)));
        }
        if intent.is_pipeline_run() {
            if !visible.is_empty() {
                println!("{}", visible.trim());
            }
            if intent.pipeline_kind() == PipelineKind::Status {
                let text = pipeline_status_text(&intent);
                let _ = clear_pending_intent();
                let task = finish_fast_task(user_message, TaskKind::Pipeline, &text)?;
                return Ok(match pipeline_jobs::latest_job().ok().flatten() {
                    Some(m) => turn_with_pipeline(text, m, Some(task)),
                    None => turn_out(text, Some(task)),
                });
            }
            let _ = clear_pending_intent();
            let mut turn = start_async_pipeline(user_message, &intent)?;
            if !visible.trim().is_empty() {
                turn.text = format!("{}\n\n{}", visible.trim(), turn.text);
            }
            return Ok(turn);
        }
        if intent.is_pipeline_ask() {
            let _ = save_pending_intent(&intent);
            let shown = ask_visible(&visible, &intent).to_string();
            let task = finish_fast_task(user_message, TaskKind::Pipeline, &shown)?;
            return Ok(turn_out(shown, Some(task)));
        }
    }

    let _ = clear_pending_intent();
    let reply = visible.trim().to_string();
    let task = finish_fast_task(user_message, TaskKind::Reply, &reply)?;
    // Fast path: return the full reply (already completed on the board).
    Ok(turn_out(reply, Some(task)))
}

fn finish_fast_task(
    user_message: &str,
    kind: TaskKind,
    result: &str,
) -> Result<TaskRecord, String> {
    let mut task = task_board::create_task(user_message, kind, None, None)?;
    let ack = if result.trim().is_empty() {
        task_board::default_ack(&task.kind)
    } else {
        // For fast completes, ack is unused for display; result is the chat text.
        String::new()
    };
    task.ack_text = ack;
    task.result_text = result.to_string();
    task.status = TaskStatus::Completed;
    task.stage = "completion".to_string();
    task_board::save_task(&task)?;
    persist_turn(user_message, result);
    Ok(task)
}

fn begin_async_task(
    user_message: &str,
    kind: TaskKind,
    expected: &str,
) -> Result<(TaskRecord, String), String> {
    let mut task = task_board::create_task(user_message, kind.clone(), None, Some(expected))?;
    let ack = task_board::default_ack(&kind);
    task = task_board::set_ack(&task.id, &ack)?;
    persist_turn(user_message, &ack);
    Ok((task, ack))
}

fn start_async_research(user_message: &str, query: &str, model: &str) -> Result<ChatTurn, String> {
    let (task, ack) = begin_async_task(user_message, TaskKind::Research, "Live facts answered")?;
    let task_id = task.id.clone();
    let user = user_message.to_string();
    let q = query.to_string();
    let model = model.to_string();
    thread::spawn(move || match run_search_and_synthesize(&user, &q, &model) {
        Ok(text) => {
            let _ = task_board::complete_task(&task_id, &text);
            let _ = append_chat_turn("brain", &text);
        }
        Err(e) => {
            let _ = task_board::fail_task(&task_id, &e);
            let _ = append_chat_turn("brain", &format!("Failed: {e}"));
        }
    });
    Ok(turn_out(ack, Some(task)))
}

fn start_async_files(
    user_message: &str,
    query: &str,
    preview: bool,
    model: &str,
) -> Result<ChatTurn, String> {
    let (task, ack) = begin_async_task(user_message, TaskKind::Files, "Host paths reported")?;
    let task_id = task.id.clone();
    let user = user_message.to_string();
    let q = query.to_string();
    let model = model.to_string();
    thread::spawn(
        move || match run_files_and_synthesize(&user, &q, preview, &model) {
            Ok(turn) => {
                let _ = task_board::complete_task(&task_id, &turn.text);
                let _ = append_chat_turn("brain", &turn.text);
            }
            Err(e) => {
                let _ = task_board::fail_task(&task_id, &e);
                let _ = append_chat_turn("brain", &format!("Failed: {e}"));
            }
        },
    );
    Ok(turn_out(ack, Some(task)))
}

fn start_async_code(
    user_message: &str,
    query: &str,
    port: u16,
    model: &str,
) -> Result<ChatTurn, String> {
    let (task, ack) = begin_async_task(user_message, TaskKind::Code, "Demo hosted")?;
    let task_id = task.id.clone();
    let user = user_message.to_string();
    let q = query.to_string();
    let model = model.to_string();
    thread::spawn(
        move || match run_code_and_synthesize(&user, &q, port, &model) {
            Ok(text) => {
                let _ = task_board::complete_task(&task_id, &text);
                let _ = append_chat_turn("brain", &text);
            }
            Err(e) => {
                let _ = task_board::fail_task(&task_id, &e);
                let _ = append_chat_turn("brain", &format!("Failed: {e}"));
            }
        },
    );
    Ok(turn_out(ack, Some(task)))
}

fn start_async_pipeline(user_message: &str, intent: &SmithIntent) -> Result<ChatTurn, String> {
    let (mut task, ack) = begin_async_task(user_message, TaskKind::Pipeline, "Pipeline finished")?;
    let meta = match pipeline_jobs::start_from_intent(intent) {
        Ok(m) => m,
        Err(e) => {
            let _ = task_board::fail_task(&task.id, &e);
            return Err(e);
        }
    };
    task = task_board::update_task(&task.id, |t| {
        t.job_id = Some(meta.id.clone());
        t.ack_text = ack.clone();
    })?;
    // Watcher: when pipeline job leaves running, complete the board task.
    let task_id = task.id.clone();
    let job_id = meta.id.clone();
    thread::spawn(move || {
        let mut status_errs = 0u32;
        let started = std::time::Instant::now();
        const MAX_STATUS_ERRS: u32 = 30;
        const MAX_WATCH: std::time::Duration = std::time::Duration::from_secs(6 * 60 * 60);
        loop {
            thread::sleep(std::time::Duration::from_secs(2));
            if started.elapsed() > MAX_WATCH {
                let _ =
                    task_board::fail_task(&task_id, "pipeline watcher timed out waiting for job");
                break;
            }
            let job = match pipeline_jobs::job_status(&job_id) {
                Ok(j) => {
                    status_errs = 0;
                    j
                }
                Err(e) => {
                    status_errs += 1;
                    if status_errs >= MAX_STATUS_ERRS {
                        let _ = task_board::fail_task(
                            &task_id,
                            &format!("pipeline status lost after retries: {e}"),
                        );
                        break;
                    }
                    continue;
                }
            };
            match job.state {
                pipeline_jobs::JobState::Succeeded => {
                    let summary = job.snapshot_summary();
                    let _ = task_board::complete_task(&task_id, &summary);
                    let _ = append_chat_turn("brain", &summary);
                    break;
                }
                pipeline_jobs::JobState::AwaitingSignoff => {
                    let summary = job.snapshot_summary();
                    let _ = task_board::awaiting_signoff(&task_id, &summary);
                    let _ = append_chat_turn("brain", &summary);
                    break;
                }
                pipeline_jobs::JobState::Failed => {
                    let err = job
                        .error
                        .clone()
                        .unwrap_or_else(|| "pipeline failed".into());
                    let _ = task_board::fail_task(&task_id, &err);
                    let _ = append_chat_turn("brain", &format!("Failed: {err}"));
                    break;
                }
                pipeline_jobs::JobState::Interrupted => break,
                pipeline_jobs::JobState::Queued | pipeline_jobs::JobState::Running => {}
            }
        }
    });
    let text = format!("{ack} ({})", meta.id);
    Ok(turn_with_pipeline(text, meta, Some(task)))
}

fn pipeline_status_text(intent: &SmithIntent) -> String {
    if let Some(id) = intent.query_trimmed() {
        if id.starts_with("job-") {
            if let Ok(meta) = pipeline_jobs::job_status(&id) {
                return pipeline_jobs::format_status(&meta);
            }
        }
    }
    match pipeline_jobs::latest_job().ok().flatten() {
        Some(meta) => pipeline_jobs::format_status(&meta),
        None => "No pipeline jobs yet.".to_string(),
    }
}

fn ask_visible<'a>(visible: &'a str, intent: &'a SmithIntent) -> &'a str {
    if !visible.trim().is_empty() {
        visible.trim()
    } else {
        intent
            .ask
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("")
    }
}

fn pipeline_mode_hint() -> &'static str {
    " For live web facts use :::smith search. For repo ship/PR work emit :::smith pipeline (kind=ship|plan|develop|review|release|status). Missing project/branch/task → phase=ask. Never claim push/release completed without an explicit user signoff after develop. Do not claim async work is finished until results are ready."
}

fn persist_turn(user_message: &str, brain_message: &str) {
    if let Err(e) = append_chat_turn("user", user_message) {
        eprintln!("Warning: failed to persist user turn: {}", e);
    }
    if let Err(e) = append_chat_turn("brain", brain_message) {
        eprintln!("Warning: failed to persist brain turn: {}", e);
    }
}

fn prompt_brain(prompt: &str, model: &str) -> Result<docker::BrainPromptResult, String> {
    let mem = ensure_memory_layout()?;
    let meta = load_chat_session_meta().unwrap_or_default();
    let stored_id = meta
        .opencode_session_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let mut opts = docker::BrainPromptOpts {
        model: Some(model),
        verbose: false,
        agent: docker::BRAIN_CHAT_AGENT,
        session_id: stored_id,
        continue_last: false,
    };

    let run = |opts: docker::BrainPromptOpts<'_>| -> Result<docker::BrainPromptResult, String> {
        if current_runtime().is_host() {
            crate::brain::host_runtime::run_prompt_on_host(&mem, prompt, opts)
        } else {
            docker::run_prompt_in_brain_with(prompt, opts)
        }
    };

    let result = match run(opts.clone()) {
        Ok(r) => r,
        Err(e) if stored_id.is_some() && session_error_looks_stale(&e) => {
            let _ = set_opencode_session_id(None);
            opts.session_id = None;
            run(opts)?
        }
        Err(e) => return Err(e),
    };

    if let Some(ref id) = result.session_id {
        if stored_id != Some(id.as_str()) {
            let _ = set_opencode_session_id(Some(id.clone()));
        }
    }
    Ok(result)
}

fn run_search_and_synthesize(
    user_message: &str,
    query: &str,
    model: &str,
) -> Result<String, String> {
    println!("  Searching (ephemeral window)…");
    let results = docker::run_oneshot_web_search(query)?;

    let synth = format!(
        "{}\nUser: {}\n\nDeclassified search results (ephemeral worker; already shut down):\n---\n{}\n---\nAnswer from these results only — do not open by echoing the user. Prefer any \"Live weather\" lines as authoritative. Do not invent extra live numbers. If results lack the facts needed, say what is missing briefly. Close with a short plain Review check-in (\"Review: … Anything else?\"). Prefer spoken sentences. Do not mention search windows, Docker, or workers. Do not emit a :::smith block unless results are unusable.",
        build_chat_preamble()?,
        user_message,
        results.trim()
    );
    let result = prompt_brain(&synth, model)?;
    let (visible, _) = split_intent(&result.answer);
    Ok(visible)
}

fn run_code_and_synthesize(
    user_message: &str,
    brief: &str,
    port: u16,
    model: &str,
) -> Result<String, String> {
    println!("  Coding + hosting (sandbox)…");
    let results = docker::run_code_and_host(brief, port)?;

    let synth = format!(
        "{}\nUser: {}\n\nDeclassified coding/sandbox results:\n---\n{}\n---\nTell the user the URL and what was built — do not open by echoing them. Do not invent paths or ports. Close with a short plain Review check-in (\"Review: … Anything else?\"). Prefer spoken sentences. Do not mention Docker, workers, or containers. Do not emit a :::smith block unless the results are unusable. Mention they can stop the demo with `smith settings sandbox stop` if still running.",
        build_chat_preamble()?,
        user_message,
        results.trim()
    );
    let result = prompt_brain(&synth, model)?;
    let (visible, _) = split_intent(&result.answer);
    Ok(visible)
}

fn run_files_and_synthesize(
    user_message: &str,
    query: &str,
    preview: bool,
    model: &str,
) -> Result<ChatTurn, String> {
    println!("  Searching host disks (allowlisted roots)…");
    let hits = crate::brain::files::find_files(query)?;
    let mut attachments = Vec::new();
    let mut opened = String::new();
    if preview {
        if let Some(img) = hits.iter().find(|h| h.is_image()) {
            match crate::brain::files::open_path(&img.path) {
                Ok(()) => {
                    opened = format!("Opened in OS viewer: {}\n", img.path.display());
                    attachments.push(ChatAttachment {
                        kind: "image".to_string(),
                        path: img.path.display().to_string(),
                        name: img.name.clone(),
                    });
                }
                Err(e) => opened = format!("Could not open preview: {}\n", e),
            }
        }
    }

    let listing = crate::brain::files::format_hits(&hits);
    let synth = format!(
        "{}\nUser: {}\n\nHost file search results (allowlisted roots only):\n{}\n{}\n---\nReport these paths — do not open by echoing the user. Do not invent extra files. Close with a short plain Review check-in (\"Review: … Anything else?\"). Prefer spoken sentences. Do not emit a :::smith block unless results are unusable.",
        build_chat_preamble()?,
        user_message,
        opened,
        listing.trim()
    );
    let result = prompt_brain(&synth, model)?;
    let (visible, _) = split_intent(&result.answer);
    Ok(ChatTurn {
        text: visible,
        attachments,
        pipeline: pipeline_jobs::latest_job().ok().flatten(),
        task: None,
    })
}

/// `smith think` — host-orchestrated iterate / save / kill loop.
pub async fn handle_think(goal: String, max_steps: u32, reset: bool) {
    let (_mem, model) = match ensure_ready() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let mut analysis = if reset {
        AnalysisState::new(&goal)
    } else {
        match load_analysis() {
            Ok(Some(existing)) if existing.goal == goal || goal.is_empty() => existing,
            Ok(Some(_)) | Ok(None) => AnalysisState::new(&goal),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    };

    if let Err(e) = save_analysis(&analysis) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    println!("Think — goal: {}", analysis.goal);
    println!();

    for step in 1..=max_steps {
        if analysis.is_complete() {
            println!("Done (status={}).", analysis.status);
            break;
        }
        if analysis.is_blocked() {
            println!("Blocked: {}", analysis.last_step);
            break;
        }

        println!("── step {}/{} ──", step, max_steps);
        analysis.status = "in_progress".to_string();
        let _ = save_analysis(&analysis);

        let prompt = build_think_step_prompt(&analysis, step);
        let think_result = crate::run_with_spinner("waiting on brain", || {
            if current_runtime().is_host() {
                match ensure_memory_layout() {
                    Ok(mem) => crate::brain::host_runtime::run_prompt_on_host(
                        &mem,
                        &prompt,
                        docker::BrainPromptOpts {
                            model: Some(&model),
                            verbose: false,
                            agent: docker::BRAIN_THINK_AGENT,
                            session_id: None,
                            continue_last: false,
                        },
                    )
                    .map(|r| r.answer),
                    Err(e) => Err(e),
                }
            } else {
                docker::run_prompt_in_brain(&prompt, Some(&model), false, docker::BRAIN_THINK_AGENT)
            }
        });
        match think_result {
            Ok(answer) => {
                match load_analysis() {
                    Ok(Some(updated)) => {
                        if updated.last_step.is_empty() && updated.status == "in_progress" {
                            analysis.last_step = truncate(&answer, 500);
                            analysis.evidence.push(truncate(&answer, 1000));
                            let lower = answer.to_lowercase();
                            if lower.contains("\"status\": \"done\"")
                                || lower.contains("status\": \"done\"")
                                || lower.contains("GOAL_COMPLETE")
                            {
                                analysis.status = "done".to_string();
                                analysis.open_items.clear();
                            } else if lower.contains("\"status\": \"blocked\"")
                                || lower.contains("GOAL_BLOCKED")
                            {
                                analysis.status = "blocked".to_string();
                            } else {
                                analysis.status = "in_progress".to_string();
                            }
                            let _ = save_analysis(&analysis);
                        } else {
                            analysis = updated;
                        }
                    }
                    Ok(None) => {
                        analysis.last_step = truncate(&answer, 500);
                        analysis.evidence.push(truncate(&answer, 1000));
                        let _ = save_analysis(&analysis);
                    }
                    Err(e) => eprintln!("Warning: could not reload analysis: {}", e),
                }

                let _ = append_journal(step, &format!("ok: {}", truncate(&answer, 200)));
                println!("  status: {} — {}", analysis.status, analysis.last_step);
            }
            Err(e) => {
                let _ = append_journal(step, &format!("err: {}", e));
                eprintln!("Step failed: {}", e);
                analysis.status = "blocked".to_string();
                analysis.last_step = format!("error: {}", e);
                let _ = save_analysis(&analysis);
                break;
            }
        }

        if analysis.is_complete() {
            println!("\nDone.");
            break;
        }
        if analysis.is_blocked() {
            println!("\nBlocked.");
            break;
        }
        if step == max_steps {
            println!("\nReached max steps.");
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        t.to_string()
    } else {
        let truncated: String = t.chars().take(max).collect();
        format!("{}…", truncated)
    }
}

fn build_think_step_prompt(analysis: &AnalysisState, step: u32) -> String {
    let body = serde_json::to_string_pretty(analysis).unwrap_or_else(|_| "{}".to_string());
    format!(
        r#"{rules}

You are running a private think/solve step. No cloud egress.

Your durable notebook is at `{analysis_path}` (also shown below).
After this single step, UPDATE that file using a shell command.

Rules:
- Work on exactly ONE open item this step (pick the highest-ambiguity / highest-risk board item).
- Advance the current goal with substance; do not open by restating it. Do not stall on questions; only block for a truly missing fact.
- Close the step summary with a brief Review check-in when useful.
- Map progress toward board stages when useful: new → discovery → preproduction → production → review/signoff → completion.
- Update fields: open_items, done_criteria, status (pending|in_progress|done|blocked), last_step, evidence.
- Set status to "done" and clear open_items only when the goal is truly satisfied.
- Set status to "blocked" if you cannot proceed without external/network access or human input.
- Keep evidence entries short.
- Stay private; do not suggest cloud APIs.

Current step number: {step}

Current analysis.json:
{body}

Perform the step, write the updated JSON, then briefly summarize what you did."#,
        rules = operating_rules(),
        analysis_path = if current_runtime().is_host() {
            "working/analysis.json"
        } else {
            "/memory/working/analysis.json"
        },
        step = step,
        body = body
    )
}
