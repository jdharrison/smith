//! Structured intents emitted by the sealed brain for host-side workers.
//!
//! Chat stays sealed. The host parses a `:::smith … :::` block and may open a
//! short-lived egress window (search today; sandbox/code later).

use serde::{Deserialize, Serialize};

pub const INTENT_BEGIN: &str = ":::smith";
pub const INTENT_END: &str = ":::";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum IntentAction {
    #[default]
    None,
    Search,
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

/// Host-routable request from a brain turn (template for search → later workers).
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
    /// Search query / worker brief (declassified).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
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

    pub fn query_trimmed(&self) -> Option<String> {
        self.query
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
            let intent = serde_json::from_str::<SmithIntent>(json_raw).ok();
            return (visible, intent);
        }
    }
    (text.trim().to_string(), None)
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
    ) || t.starts_with("yes ")
        || t.starts_with("yeah ")
        || t.starts_with("ok ")
        || t.starts_with("okay ")
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
    fn split_intent_none() {
        let (visible, intent) = split_intent("Hello, I'm smith.");
        assert_eq!(visible, "Hello, I'm smith.");
        assert!(intent.is_none());
    }

    #[test]
    fn affirmation_yes() {
        assert!(looks_like_affirmation("Yes"));
        assert!(looks_like_affirmation("  yeah  "));
        assert!(!looks_like_affirmation("no, New York"));
    }
}
