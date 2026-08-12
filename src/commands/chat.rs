//! Sealed brain chat REPL and think (iterate/kill) loop.

use crate::brain::{
    append_journal, ensure_memory_layout, load_analysis, load_profile, recent_episodic,
    save_analysis, write_episodic, AnalysisState,
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

fn build_chat_preamble() -> Result<String, String> {
    let profile = load_profile()?;
    let recent = recent_episodic(3)?;
    let mut preamble = String::from(
        "You are the sealed local brain for smith. You run entirely privately. \
         Never suggest calling cloud APIs or sending data off-host. \
         Memory is available under /memory (profile.md, episodic/, working/).\n\n",
    );
    preamble.push_str("## Profile\n");
    preamble.push_str(profile.trim());
    preamble.push_str("\n\n");
    if !recent.is_empty() {
        preamble.push_str("## Recent episodic notes\n");
        for note in recent {
            preamble.push_str(note.trim());
            preamble.push_str("\n\n");
        }
    }
    Ok(preamble)
}

fn ensure_ready() -> Result<(std::path::PathBuf, String), String> {
    let mem = ensure_memory_layout()?;
    let model = resolve_local_model();
    docker::ensure_brain_ready(&mem, Some(&model))?;
    Ok((mem, model))
}

/// `smith chat` — interactive REPL or one-shot `-m`.
pub async fn handle_chat(message: Option<String>) {
    let (_mem, model) = match ensure_ready() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    if let Some(msg) = message {
        match run_turn(&msg, &model) {
            Ok(answer) => println!("{}", answer.trim()),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    println!("Sealed brain chat (local only). Type /exit or Ctrl-D to quit.");
    println!();

    let stdin = io::stdin();
    let mut session_notes: Vec<String> = Vec::new();

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

        match run_turn(trimmed, &model) {
            Ok(answer) => {
                let ans = answer.trim();
                println!("brain> {}", ans);
                println!();
                session_notes.push(format!("User: {}\nBrain: {}\n", trimmed, ans));
            }
            Err(e) => eprintln!("Error: {}", e),
        }
    }

    if !session_notes.is_empty() {
        let summary = session_notes.join("\n---\n");
        match write_episodic(&summary) {
            Ok(path) => println!("Session saved to {}", path.display()),
            Err(e) => eprintln!("Warning: failed to save episodic note: {}", e),
        }
    }
}

fn run_turn(user_message: &str, model: &str) -> Result<String, String> {
    let preamble = build_chat_preamble()?;
    let prompt = format!(
        "{}\n## Current user message\n{}\n\nRespond helpfully and concisely.",
        preamble, user_message
    );
    docker::run_prompt_in_brain(&prompt, Some(model), false)
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
        match docker::run_prompt_in_brain(&prompt, Some(&model), false) {
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
        r#"You are the sealed local brain running a private solve loop. No cloud egress.

Your durable notebook is at `/memory/working/analysis.json` (also shown below).
After this single step, UPDATE that file using a shell command (e.g. cat > /memory/working/analysis.json <<'EOF' ... EOF).

Rules:
- Work on exactly ONE open item this step.
- Update fields: open_items, done_criteria, status (pending|in_progress|done|blocked), last_step, evidence.
- Set status to "done" and clear open_items only when the goal is truly satisfied.
- Set status to "blocked" if you cannot proceed without external/network access or human input.
- Keep evidence entries short.
- Stay private; do not suggest cloud APIs.

Current step number: {step}

Current analysis.json:
{body}

Perform the step, write the updated JSON to /memory/working/analysis.json, then briefly summarize what you did."#,
        step = step,
        body = body
    )
}
