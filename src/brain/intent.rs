//! Structured intents emitted by the sealed brain for host-side workers.
//!
//! Chat stays tool-free. The host parses a `:::smith … :::` block and may open
//! workers (search, files, code/sandbox, pipeline).

use serde::{Deserialize, Serialize};

pub const INTENT_BEGIN: &str = ":::smith";
pub const INTENT_END: &str = ":::";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum IntentAction {
    #[default]
    None,
    Search,
    Files,
    Code,
    Pipeline,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum IntentPhase {
    #[default]
    None,
    Ask,
    Run,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum PipelineKind {
    #[default]
    None,
    Plan,
    Develop,
    Review,
    Release,
    Ship,
    Status,
    #[serde(other)]
    Unknown,
}

/// Host-routable request from a brain turn.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SmithIntent {
    #[serde(default)]
    pub v: u32,
    #[serde(default)]
    pub action: IntentAction,
    #[serde(default)]
    pub phase: IntentPhase,
    /// User-facing clarifying question (phase=ask).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ask: Option<String>,
    /// Search query / filename / worker brief / pipeline prompt (declassified).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    /// Host files: open/preview the first image hit.
    #[serde(default)]
    pub preview: bool,
    /// Code/sandbox: host port to publish (default 8080).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Pipeline stage or ship/status.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<PipelineKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// Plan id (`plan-xxxx` or short id).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    /// Develop task (falls back to query when empty).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    /// Request PR after develop/release when supported.
    #[serde(default)]
    pub pr: bool,
}

impl SmithIntent {
    pub fn is_search_ask(&self) -> bool {
        self.action == IntentAction::Search && self.phase == IntentPhase::Ask
    }

    pub fn is_search_run(&self) -> bool {
        self.action == IntentAction::Search
            && self.phase == IntentPhase::Run
            && self
                .query
                .as_deref()
                .map(|q| !q.trim().is_empty())
                .unwrap_or(false)
    }

    pub fn is_files_ask(&self) -> bool {
        self.action == IntentAction::Files && self.phase == IntentPhase::Ask
    }

    pub fn is_files_run(&self) -> bool {
        self.action == IntentAction::Files
            && self.phase == IntentPhase::Run
            && self
                .query
                .as_deref()
                .map(|q| !q.trim().is_empty())
                .unwrap_or(false)
    }

    pub fn is_code_ask(&self) -> bool {
        self.action == IntentAction::Code && self.phase == IntentPhase::Ask
    }

    pub fn is_code_run(&self) -> bool {
        self.action == IntentAction::Code
            && self.phase == IntentPhase::Run
            && self
                .query
                .as_deref()
                .map(|q| !q.trim().is_empty())
                .unwrap_or(false)
    }

    pub fn is_pipeline_ask(&self) -> bool {
        self.action == IntentAction::Pipeline && self.phase == IntentPhase::Ask
    }

    pub fn is_pipeline_run(&self) -> bool {
        self.action == IntentAction::Pipeline && self.phase == IntentPhase::Run
    }

    pub fn is_pipeline_release_ask(&self) -> bool {
        self.is_pipeline_ask() && matches!(self.pipeline_kind(), PipelineKind::Release)
    }

    pub fn pipeline_kind(&self) -> PipelineKind {
        self.kind.clone().unwrap_or(PipelineKind::None)
    }

    pub fn code_port(&self) -> u16 {
        self.port.unwrap_or(8080).max(1)
    }

    pub fn query_trimmed(&self) -> Option<String> {
        self.query
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    }

    pub fn task_trimmed(&self) -> Option<String> {
        self.task
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .or_else(|| self.query_trimmed())
    }

    pub fn project_trimmed(&self) -> Option<String> {
        self.project
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    }

    pub fn branch_trimmed(&self) -> Option<String> {
        self.branch
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    }

    pub fn plan_trimmed(&self) -> Option<String> {
        self.plan
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    }
}

/// Split visible prose from an optional trailing/embedded `:::smith` JSON block.
pub fn split_intent(text: &str) -> (String, Option<SmithIntent>) {
    if let Some(start) = text.find(INTENT_BEGIN) {
        let after_tag = &text[start + INTENT_BEGIN.len()..];
        let json_and_rest = after_tag.trim_start();
        if let Some(end_rel) = json_and_rest.find(INTENT_END) {
            let json_raw = json_and_rest[..end_rel].trim();
            let before = text[..start].trim().to_string();
            let after = json_and_rest[end_rel + INTENT_END.len()..].trim();
            let visible = [before.as_str(), after]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n");
            let intent = parse_intent_json(json_raw);
            return (visible, intent);
        }
    }
    (text.trim().to_string(), None)
}

fn parse_intent_json(raw: &str) -> Option<SmithIntent> {
    if let Ok(intent) = serde_json::from_str::<SmithIntent>(raw) {
        return Some(intent);
    }
    // Models often emit Windows paths with single backslashes, which are invalid JSON.
    let repaired = repair_json_backslashes(raw);
    serde_json::from_str::<SmithIntent>(&repaired).ok()
}

/// Turn invalid `\X` escapes into `\\X` so Windows paths parse.
fn repair_json_backslashes(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + 8);
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            let next = chars[i + 1];
            if matches!(
                next,
                '"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't' | 'u'
            ) {
                out.push('\\');
                out.push(next);
                i += 2;
                continue;
            }
            out.push('\\');
            out.push('\\');
            i += 1;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// User message looks like confirmation of a pending ask.
pub fn looks_like_affirmation(text: &str) -> bool {
    let t = text.trim().to_lowercase();
    matches!(
        t.as_str(),
        "y" | "yes"
            | "yeah"
            | "yep"
            | "yup"
            | "correct"
            | "confirm"
            | "confirmed"
            | "that's right"
            | "thats right"
            | "ok"
            | "okay"
            | "k"
            | "sure"
            | "do it"
            | "proceed"
            | "go"
            | "go ahead"
            | "affirmative"
            | "ship it"
            | "sign off"
            | "signoff"
    ) || t.starts_with("yes ")
        || t.starts_with("yeah ")
        || t.starts_with("ok ")
        || t.starts_with("okay ")
}

/// User declines a pending ask (clear without running).
pub fn looks_like_negation(text: &str) -> bool {
    let t = text.trim().to_lowercase();
    matches!(
        t.as_str(),
        "n" | "no"
            | "nope"
            | "nah"
            | "cancel"
            | "stop"
            | "don't"
            | "dont"
            | "abort"
            | "nevermind"
            | "never mind"
    ) || t.starts_with("no ")
        || t.starts_with("don't ")
        || t.starts_with("dont ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_intent_ask() {
        let raw = r#"Confirming — Corona, California?

:::smith
{"v":1,"action":"search","phase":"ask","ask":"Did you mean Corona, California?","query":"weather today Corona California"}
:::
"#;
        let (visible, intent) = split_intent(raw);
        assert!(visible.contains("Corona"));
        assert!(!visible.contains(":::smith"));
        let intent = intent.expect("intent");
        assert!(intent.is_search_ask());
        assert_eq!(
            intent.query.as_deref(),
            Some("weather today Corona California")
        );
    }

    #[test]
    fn split_intent_run() {
        let raw = r#"I'll look that up.

:::smith
{"v":1,"action":"search","phase":"run","query":"weather Corona California today"}
:::
"#;
        let (visible, intent) = split_intent(raw);
        assert_eq!(visible, "I'll look that up.");
        assert!(intent.unwrap().is_search_run());
    }

    #[test]
    fn split_intent_files() {
        let raw = r#"Looking on this machine.

:::smith
{"v":1,"action":"files","phase":"run","query":"X.jpg","preview":true}
:::
"#;
        let (visible, intent) = split_intent(raw);
        assert!(visible.contains("Looking"));
        let intent = intent.expect("intent");
        assert!(intent.is_files_run());
        assert!(intent.preview);
        assert_eq!(intent.query.as_deref(), Some("X.jpg"));
    }

    #[test]
    fn split_intent_none() {
        let (visible, intent) = split_intent("Hello, I'm smith.");
        assert_eq!(visible, "Hello, I'm smith.");
        assert!(intent.is_none());
    }

    #[test]
    fn split_intent_code() {
        let raw = r#"Building that now.

:::smith
{"v":1,"action":"code","phase":"run","query":"hello world on 8080","port":8080}
:::
"#;
        let (visible, intent) = split_intent(raw);
        assert!(visible.contains("Building"));
        let intent = intent.expect("intent");
        assert!(intent.is_code_run());
        assert_eq!(intent.code_port(), 8080);
    }

    #[test]
    fn split_intent_pipeline_ship() {
        let raw = r#"Starting that.

:::smith
{"v":1,"action":"pipeline","phase":"run","kind":"ship","project":"myapp","branch":"feature/x","query":"Add login"}
:::
"#;
        let (_, intent) = split_intent(raw);
        let intent = intent.expect("intent");
        assert!(intent.is_pipeline_run());
        assert_eq!(intent.pipeline_kind(), PipelineKind::Ship);
        assert_eq!(intent.project_trimmed().as_deref(), Some("myapp"));
    }

    #[test]
    fn split_intent_files_windows_path() {
        let raw = r#"Looking.

:::smith
{"v":1,"action":"files","phase":"run","query":"C:\Users\Jake\Pictures\portrait","preview":true}
:::
"#;
        let (visible, intent) = split_intent(raw);
        assert!(visible.contains("Looking"));
        let intent = intent.expect("repaired intent");
        assert!(intent.is_files_run());
        assert!(intent
            .query
            .as_deref()
            .unwrap_or("")
            .contains("Pictures"));
    }

    #[test]
    fn affirmation_yes() {
        assert!(looks_like_affirmation("Yes"));
        assert!(looks_like_affirmation("  yeah  "));
        assert!(!looks_like_affirmation("no, New York"));
        assert!(looks_like_negation("no"));
        assert!(looks_like_negation("cancel"));
    }
}
