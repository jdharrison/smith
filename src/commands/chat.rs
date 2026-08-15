//! Sealed brain chat REPL and think (iterate/kill) loop.

use crate::brain::{
    append_chat_turn, append_journal, clear_pending_intent, ensure_memory_layout, load_analysis,
    load_chat_session_meta, load_pending_intent, load_profile, looks_like_affirmation,
    recent_chat_history_text, recent_episodic, reset_chat_session, save_analysis,
    save_pending_intent, set_opencode_session_id, split_intent, AnalysisState,
};
use crate::docker;
use crate::DEFAULT_LOCAL_MODEL;
use std::io::{self, BufRead, Write};

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

/// Concise sealed-brain operating rules (sized for local ~14B models).
fn smith_operating_rules() -> &'static str {
    r#"You are smith — skeptical, precise, reality-checking (Matrix Agent Smith vibe; not cartoon villainy). If asked your name, answer "smith".

Conversation (natural, not an interrogation):
1) Always show you understood: briefly restate the ask / goal in your own words before anything else.
2) Resolve: infer reasonable defaults, determine requirements, and move the work forward. Do not ask a question every turn.
3) Ask at most ONE question, and only if a missing fact truly blocks progress (two equally plausible readings). Never lead with a question; never stack questions.
4) After delivering something useful: short review, then "anything else?" — not another quiz.
5) Board when useful: new → discovery → preproduction → production → review/signoff → completion.

Multi-command: "smith we need to research … also smith we should run QA …" = separate objectives; consolidate on the board.

Sealed/local only — never suggest cloud APIs or off-host data.

Live web facts: you cannot browse. Prefer phase=run immediately when the query is clear enough (state your assumption in the spoken reply, e.g. Corona → California). Use phase=ask only when two readings are plausible — and still restate understanding first, then one question.

:::smith
{"v":1,"action":"search","phase":"run","query":"<full search query>"}
:::

or (only if blocked):

:::smith
{"v":1,"action":"search","phase":"ask","ask":"<one clarifying question>","query":"<draft query>"}
:::

Do not invent live numbers before results. Speak only to the user outside the block. Do not mention Docker or workers."#
}

fn build_chat_preamble() -> Result<String, String> {
    let profile = load_profile()?;
    let recent = recent_episodic(3).unwrap_or_default();
    let history = recent_chat_history_text().unwrap_or_default();
    let mut preamble = String::from(smith_operating_rules());
    preamble.push_str("\n\nContext (do not restate unless asked):\n");
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

pub(crate) fn ensure_ready() -> Result<(std::path::PathBuf, String), String> {
    let mem = ensure_memory_layout()?;
    let model = resolve_local_model();
    docker::ensure_brain_ready(&mem, Some(&model))?;
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
        match run_chat_turn(&msg, &model) {
            Ok(answer) => crate::term_md::print_markdown(answer.trim()),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    println!("Sealed brain chat (local only). Type /exit or Ctrl-D to quit. /new starts a fresh session.");
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

        match run_chat_turn(trimmed, &model) {
            Ok(answer) => {
                println!("brain>");
                crate::term_md::print_markdown(answer.trim());
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

pub(crate) fn run_chat_turn(user_message: &str, model: &str) -> Result<String, String> {
    let pending = load_pending_intent().ok().flatten();
    if let Some(ref p) = pending {
        if p.is_search_ask() && looks_like_affirmation(user_message) {
            if let Some(query) = p.query_trimmed() {
                let _ = clear_pending_intent();
                let final_answer = run_search_and_synthesize(user_message, &query, model)?;
                persist_turn(user_message, &final_answer);
                return Ok(final_answer);
            }
        }
    }

    let prompt = format!(
        "{}\nUser: {}\n\nRestate understanding first, then help — do not open with a question. Infer defaults and resolve; ask at most one blocking question, and only after that restatement. If you delivered an answer, close with a brief review or \"anything else?\" Speak directly; do not narrate your reply.",
        build_chat_preamble()?,
        user_message
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
            let final_answer = run_search_and_synthesize(user_message, &query, model)?;
            persist_turn(user_message, &final_answer);
            return Ok(final_answer);
        }
        if intent.is_search_ask() {
            let _ = save_pending_intent(&intent);
            // Prefer full spoken reply (understanding + question), not only the ask field.
            let shown = if !visible.trim().is_empty() {
                visible.trim()
            } else {
                intent
                    .ask
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("")
            };
            persist_turn(user_message, shown);
            return Ok(shown.to_string());
        }
    }

    let _ = clear_pending_intent();
    persist_turn(user_message, visible.trim());
    Ok(visible)
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

    let result = match docker::run_prompt_in_brain_with(prompt, opts.clone()) {
        Ok(r) => r,
        Err(e) if stored_id.is_some() && session_error_looks_stale(&e) => {
            let _ = set_opencode_session_id(None);
            opts.session_id = None;
            docker::run_prompt_in_brain_with(prompt, opts)?
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
        "{}\nUser: {}\n\nDeclassified search results (ephemeral worker; already shut down):\n---\n{}\n---\nRestate briefly what they asked, then answer from these results only. Do not invent extra live numbers. Close with a short review or \"anything else?\" — do not ask a new quiz. Do not mention search windows, Docker, or workers. Do not emit a :::smith block unless results are unusable.",
        build_chat_preamble()?,
        user_message,
        results.trim()
    );
    let result = prompt_brain(&synth, model)?;
    let (visible, _) = split_intent(&result.answer);
    Ok(visible)
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
        match docker::run_prompt_in_brain(&prompt, Some(&model), false, docker::BRAIN_THINK_AGENT) {
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

Your durable notebook is at `/memory/working/analysis.json` (also shown below).
After this single step, UPDATE that file using a shell command (e.g. cat > /memory/working/analysis.json <<'EOF' ... EOF).

Rules:
- Work on exactly ONE open item this step (pick the highest-ambiguity / highest-risk board item).
- Restate the current goal, then advance it. Do not stall on questions; only block for a truly missing fact.
- Map progress toward board stages when useful: new → discovery → preproduction → production → review/signoff → completion.
- Update fields: open_items, done_criteria, status (pending|in_progress|done|blocked), last_step, evidence.
- Set status to "done" and clear open_items only when the goal is truly satisfied.
- Set status to "blocked" if you cannot proceed without external/network access or human input.
- Keep evidence entries short.
- Stay private; do not suggest cloud APIs.

Current step number: {step}

Current analysis.json:
{body}

Perform the step, write the updated JSON to /memory/working/analysis.json, then briefly summarize what you did."#,
        rules = smith_operating_rules(),
        step = step,
        body = body
    )
}
