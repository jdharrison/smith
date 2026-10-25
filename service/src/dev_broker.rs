//! Per-attempt, append-only model/read authority. No I/O or provider routing lives here.
use crate::dev_local::{
    LocalError, LocalResult, MAX_READS, MAX_READ_BYTES, MAX_ROUNDS, SYSTEM_PROMPT,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};

struct Read {
    arguments: Value,
    authorized: bool,
    output: Option<String>,
}
pub(crate) struct Broker {
    messages: Vec<Value>,
    tools: Option<Value>,
    pending: BTreeMap<String, Read>,
    issued: HashSet<String>,
    fingerprints: HashSet<String>,
    rounds: u32,
    reads: u32,
    bytes: usize,
    repositories: usize,
    awaiting: bool,
    final_text: Option<String>,
}
fn comparison(mut message: Value) -> LocalResult<Value> {
    if message["role"] == "assistant" {
        if message["content"].is_null() || message["content"] == "" {
            message["content"] = Value::Null;
        }
        if let Some(calls) = message["tool_calls"].as_array_mut() {
            for call in calls {
                let raw = call["function"]["arguments"]
                    .as_str()
                    .ok_or(LocalError::ModelProtocol)?;
                call["function"]["arguments"] = crate::cloud::unique_json(raw.as_bytes())
                    .map_err(|_| LocalError::ModelProtocol)?;
            }
        }
    }
    Ok(message)
}
impl Broker {
    pub(crate) fn new(prompt: String, repositories: usize) -> LocalResult<Self> {
        if prompt.trim().is_empty() || !(1..=16).contains(&repositories) {
            return Err(LocalError::ModelProtocol);
        }
        Ok(Self {
            messages: vec![
                json!({"role":"system","content":SYSTEM_PROMPT}),
                json!({"role":"user","content":prompt}),
            ],
            tools: None,
            pending: BTreeMap::new(),
            issued: HashSet::new(),
            fingerprints: HashSet::new(),
            rounds: 0,
            reads: 0,
            bytes: 0,
            repositories,
            awaiting: false,
            final_text: None,
        })
    }
    pub(crate) fn request(&mut self, request: &Value) -> LocalResult<Value> {
        if self.awaiting || self.final_text.is_some() || self.rounds >= MAX_ROUNDS {
            return Err(LocalError::ModelLimit);
        }
        let request = crate::inspection_diagnostics::app(
            crate::inspection_diagnostics::Phase::BrokerValidateRequest,
            || crate::dev_completion::validate_request(request),
        )
        .map_err(LocalError::from)?;
        let actual = request["messages"]
            .as_array()
            .ok_or(LocalError::ModelProtocol)?;
        let prefix = self.messages.len();
        if actual.len() != prefix + self.pending.len() || actual.len() < prefix {
            return Err(LocalError::ModelProtocol);
        }
        for (actual, expected) in actual[..prefix].iter().zip(&self.messages) {
            if comparison(actual.clone())? != comparison(expected.clone())? {
                return Err(LocalError::ModelProtocol);
            }
        }
        let mut results = HashSet::new();
        for message in &actual[prefix..] {
            let id = message["tool_call_id"]
                .as_str()
                .ok_or(LocalError::ModelProtocol)?;
            let read = self.pending.get(id).ok_or(LocalError::ModelProtocol)?;
            if !read.authorized
                || !results.insert(id)
                || message["role"] != "tool"
                || message["content"].as_str() != read.output.as_deref()
                || read.output.is_none()
            {
                return Err(LocalError::ModelProtocol);
            }
        }
        if self
            .pending
            .values()
            .any(|r| !r.authorized || r.output.is_none())
        {
            return Err(LocalError::ModelProtocol);
        }
        let tools = request["tools"].clone();
        if self
            .tools
            .as_ref()
            .is_some_and(|expected| expected != &tools)
        {
            return Err(LocalError::ModelProtocol);
        }
        self.tools = Some(tools);
        let fingerprint = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(actual).map_err(|_| LocalError::ModelProtocol)?)
        );
        if !self.fingerprints.insert(fingerprint) {
            return Err(LocalError::ModelProtocol);
        }
        self.messages = actual.clone();
        self.pending.clear();
        self.rounds += 1;
        self.awaiting = true;
        Ok(request)
    }
    pub(crate) fn response(&mut self, response: &Value) -> LocalResult<Value> {
        if !self.awaiting {
            return Err(LocalError::ModelProtocol);
        }
        let response = crate::inspection_diagnostics::app(
            crate::inspection_diagnostics::Phase::BrokerValidateResponse,
            || crate::dev_completion::validate_response("selected", response),
        )
        .map_err(LocalError::from)?;
        let choice = &response["choices"][0];
        let message = &choice["message"];
        match choice["finish_reason"].as_str() {
            Some("stop") => {
                // Reservations alone are not evidence: a successful read must have
                // been recorded and consumed in the validated model transcript.
                crate::inspection_diagnostics::local(
                    crate::inspection_diagnostics::Phase::BrokerReadEvidence,
                    || {
                        if self.bytes == 0 || !self.pending.is_empty() {
                            return Err(LocalError::ModelProtocol);
                        }
                        Ok(())
                    },
                )?;
                self.final_text = Some(
                    message["content"]
                        .as_str()
                        .ok_or(LocalError::ModelProtocol)?
                        .to_owned(),
                );
            }
            Some("tool_calls") => {
                for call in message["tool_calls"]
                    .as_array()
                    .ok_or(LocalError::ModelProtocol)?
                {
                    let id = call["id"].as_str().ok_or(LocalError::ModelProtocol)?;
                    if !self.issued.insert(id.into()) || self.issued.len() > MAX_READS as usize {
                        return Err(LocalError::ModelLimit);
                    }
                    let raw = call["function"]["arguments"]
                        .as_str()
                        .ok_or(LocalError::ModelProtocol)?;
                    let arguments = crate::cloud::unique_json(raw.as_bytes())
                        .map_err(|_| LocalError::ModelProtocol)?;
                    crate::dev_completion::read_arguments(&arguments).map_err(LocalError::from)?;
                    let path = arguments["filePath"]
                        .as_str()
                        .ok_or(LocalError::ReadDenied)?;
                    if !(0..self.repositories).any(|i| {
                        path == format!("/workspace/repos/r{i}")
                            || path.starts_with(&format!("/workspace/repos/r{i}/"))
                    }) {
                        return Err(LocalError::ReadDenied);
                    }
                    self.pending.insert(
                        id.into(),
                        Read {
                            arguments,
                            authorized: false,
                            output: None,
                        },
                    );
                }
                self.messages.push(message.clone());
            }
            _ => return Err(LocalError::ModelProtocol),
        }
        self.awaiting = false;
        Ok(response)
    }
    pub(crate) fn authorize_read(&mut self, id: &str, arguments: &Value) -> LocalResult<()> {
        if self.awaiting || self.final_text.is_some() || self.reads >= MAX_READS {
            return Err(LocalError::ReadDenied);
        }
        let read = self.pending.get_mut(id).ok_or(LocalError::ReadDenied)?;
        if read.authorized || read.output.is_some() || &read.arguments != arguments {
            return Err(LocalError::ReadDenied);
        }
        read.authorized = true;
        self.reads += 1; // Reservations are never refunded, including after a failed read.
        Ok(())
    }
    pub(crate) fn record_read(&mut self, id: &str, text: String) -> LocalResult<()> {
        let read = self.pending.get_mut(id).ok_or(LocalError::ReadDenied)?;
        if !read.authorized
            || read.output.is_some()
            || text.is_empty()
            || text.len() > MAX_READ_BYTES
            || self.bytes.saturating_add(text.len()) > MAX_READ_BYTES * MAX_READS as usize
            || text
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        {
            return Err(LocalError::ReadDenied);
        }
        self.bytes += text.len();
        read.output = Some(text);
        Ok(())
    }
    pub(crate) fn verify_reply(&self, text: &str) -> LocalResult<()> {
        if self.awaiting
            || self.bytes == 0
            || !self.pending.is_empty()
            || self.final_text.as_deref() != Some(text)
        {
            Err(LocalError::ModelProtocol)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(messages: Vec<Value>) -> Value {
        json!({"model":"selected","stream":true,"messages":messages,"tools":[{"type":"function","function":{"name":"read","parameters":{"type":"object","properties":{"filePath":{"type":"string"},"offset":{"type":"integer","minimum":1,"maximum":20000},"limit":{"type":"integer","minimum":1,"maximum":32}},"required":["filePath"],"additionalProperties":false}}}]})
    }
    fn response(path: &str) -> Value {
        json!({"model":"selected","choices":[{"index":0,"finish_reason":"tool_calls","message":{"role":"assistant","content":null,"tool_calls":[{"id":"call_one","type":"function","function":{"name":"read","arguments":json!({"filePath":path,"limit":4}).to_string()}}]}}]})
    }
    fn start() -> Broker {
        Broker::new("Exact private task".into(), 1).unwrap()
    }
    #[test]
    fn original_task_and_system_cannot_be_replaced_or_retried() {
        let mut b = start();
        let mut altered = request(b.messages.clone());
        altered["messages"][1]["content"] = json!("other");
        assert!(b.request(&altered).is_err());
        let r = request(b.messages.clone());
        b.request(&r).unwrap();
        assert!(b.request(&r).is_err());
    }
    #[test]
    fn read_authorization_evidence_and_transcript_are_single_use() {
        let mut b = start();
        b.request(&request(b.messages.clone())).unwrap();
        let answer = b
            .response(&response("/workspace/repos/r0/Cargo.toml"))
            .unwrap();
        assert_eq!(answer["choices"][0]["finish_reason"], "tool_calls");
        let args = json!({"filePath":"/workspace/repos/r0/Cargo.toml","limit":4});
        assert!(b.record_read("call_one", "unapproved".into()).is_err());
        b.authorize_read("call_one", &args).unwrap();
        assert!(b.authorize_read("call_one", &args).is_err());
        b.record_read("call_one", "1: [package]".into()).unwrap();
        assert!(b.record_read("call_one", "again".into()).is_err());
        let mut history = b.messages.clone();
        history.push(json!({"role":"tool","tool_call_id":"call_one","content":"substitution"}));
        assert!(b.request(&request(history.clone())).is_err());
        history.last_mut().unwrap()["content"] = json!("1: [package]");
        b.request(&request(history)).unwrap();
        let final_response = json!({"model":"selected","choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":"Rust from Cargo.toml:1"}}]});
        b.response(&final_response).unwrap();
        b.verify_reply("Rust from Cargo.toml:1").unwrap();
        assert!(b.verify_reply("fabricated reply").is_err());
        assert!(b.authorize_read("call_one", &args).is_err());
    }
    #[test]
    fn final_response_without_read_evidence_is_rejected_before_publication() {
        let mut b = start();
        b.request(&request(b.messages.clone())).unwrap();
        let final_response = json!({"model":"selected","choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":"Rust from Cargo.toml:1"}}]});
        assert!(b.response(&final_response).is_err());
        assert!(b.final_text.is_none());
        assert!(b.verify_reply("Rust from Cargo.toml:1").is_err());
        assert!(b.request(&request(b.messages.clone())).is_err());
    }

    #[test]
    fn reserved_or_rejected_reads_do_not_establish_inspection_evidence() {
        let mut b = start();
        b.request(&request(b.messages.clone())).unwrap();
        b.response(&response("/workspace/repos/r0/Cargo.toml"))
            .unwrap();
        let args = json!({"filePath":"/workspace/repos/r0/Cargo.toml","limit":4});
        b.authorize_read("call_one", &args).unwrap();
        assert_eq!(b.reads, 1);
        assert!(b.record_read("call_one", String::new()).is_err());
        assert_eq!(b.bytes, 0);
        let mut history = b.messages.clone();
        history.push(json!({"role":"tool","tool_call_id":"call_one","content":"fabricated"}));
        assert!(b.request(&request(history)).is_err());
        assert!(b.verify_reply("fabricated").is_err());
    }

    #[test]
    fn recorded_read_must_be_consumed_before_final_reply() {
        let mut b = start();
        b.request(&request(b.messages.clone())).unwrap();
        b.response(&response("/workspace/repos/r0/Cargo.toml"))
            .unwrap();
        let args = json!({"filePath":"/workspace/repos/r0/Cargo.toml","limit":4});
        b.authorize_read("call_one", &args).unwrap();
        b.record_read("call_one", "1: [package]".into()).unwrap();
        assert!(b.bytes > 0);
        assert!(b.verify_reply("Rust from Cargo.toml:1").is_err());
        assert!(b.request(&request(b.messages.clone())).is_err());
    }

    #[test]
    fn foreign_call_paths_and_mutated_history_are_refused_before_execution() {
        let mut b = start();
        b.request(&request(b.messages.clone())).unwrap();
        assert!(b.response(&response("/workspace/repos/r1/secret")).is_err());
        let mut b = start();
        b.request(&request(b.messages.clone())).unwrap();
        b.response(&response("/workspace/repos/r0")).unwrap();
        assert!(b
            .authorize_read(
                "call_one",
                &json!({"filePath":"/workspace/repos/r0/else","limit":4})
            )
            .is_err());
        assert!(b
            .authorize_read(
                "foreign",
                &json!({"filePath":"/workspace/repos/r0","limit":4})
            )
            .is_err());
    }
    #[test]
    fn equivalent_argument_json_is_not_a_transcript_substitution() {
        let a = json!({"role":"assistant","content":null,"tool_calls":[{"function":{"arguments":"{\"limit\":4, \"filePath\":\"/workspace/repos/r0\"}"}}]});
        let b = json!({"role":"assistant","content":"","tool_calls":[{"function":{"arguments":"{\"filePath\":\"/workspace/repos/r0\",\"limit\":4}"}}]});
        assert_eq!(comparison(a).unwrap(), comparison(b).unwrap());
    }
}
