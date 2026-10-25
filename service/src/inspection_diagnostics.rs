//! Opt-in isolated QA evidence. Never log payloads, error messages or identities.
#[cfg(test)]
use crate::app::AppError;
use crate::app::AppResult;
use crate::dev_local::{LocalError, LocalResult};

#[derive(Clone, Copy)]
pub(crate) enum Phase {
    BrokerRequest,
    BrokerValidateRequest,
    BrokerResponse,
    BrokerValidateResponse,
    BrokerReadEvidence,
    ProviderValidateRequest,
    ProviderFrozenPreflight,
    RunnerValidateRequest,
    Props,
    SemanticProbeRender,
    SemanticProbeVerify,
    ActualTemplate,
    Tokenize,
    Context,
    NativeCompletion,
    ValidateResponse,
}

impl Phase {
    fn label(self) -> &'static str {
        match self {
            Self::BrokerRequest => "broker_request",
            Self::BrokerValidateRequest => "broker_validate_request",
            Self::BrokerResponse => "broker_response",
            Self::BrokerValidateResponse => "broker_validate_response",
            Self::BrokerReadEvidence => "broker_read_evidence",
            Self::ProviderValidateRequest => "provider_validate_request",
            Self::ProviderFrozenPreflight => "provider_frozen_preflight",
            Self::RunnerValidateRequest => "runner_validate_request",
            Self::Props => "props",
            Self::SemanticProbeRender => "semantic_probe_render",
            Self::SemanticProbeVerify => "semantic_probe_verify",
            Self::ActualTemplate => "actual_template",
            Self::Tokenize => "tokenize",
            Self::Context => "context",
            Self::NativeCompletion => "native_completion",
            Self::ValidateResponse => "validate_response",
        }
    }
}

fn safe_code(code: &str) -> &str {
    match code {
        "START"
        | "OK"
        | "PROVIDER_PROTOCOL"
        | "LLM_PROTOCOL"
        | "LLM_UNAVAILABLE"
        | "MODEL_UNAVAILABLE"
        | "LOCAL_MODEL_UNAVAILABLE"
        | "RUNNER_BUSY"
        | "RUNNER_UNSAFE"
        | "RUNNER_CANCELLED"
        | "PROVIDER_TIMEOUT"
        | "PROVIDER_HTTP"
        | "PROVIDER_BODY_LIMIT"
        | "RUNTIME_UNSAFE"
        | "MODEL_STORE_UNSAFE"
        | "UNSUPPORTED"
        | "RUNTIME_MISSING"
        | "CONTEXT_LIMIT"
        | "MODEL_TRUNCATED"
        | "MODEL_RESULT_LIMIT"
        | "GPU_PREPARATION"
        | "INTERRUPTED"
        | "LOCAL_LIMIT"
        | "MODEL_PROTOCOL"
        | "MODEL_LIMIT"
        | "READ_DENIED"
        | "REPOSITORY_REJECTED"
        | "REPOSITORY_UNAVAILABLE"
        | "RUNTIME_UNAVAILABLE"
        | "PUBLICATION"
        | "PROTOCOL"
        | "SHUTDOWN"
        | "DEADLINE"
        | "PREREQUISITE" => code,
        _ => "OTHER",
    }
}

#[cfg(any(feature = "inspection-diagnostics", test))]
const MAX_EVENTS: usize = 256;

#[cfg(any(feature = "inspection-diagnostics", test))]
fn line(counter: &std::sync::atomic::AtomicUsize, phase: Phase, code: &str) -> Option<String> {
    use std::sync::atomic::Ordering;
    let index = counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
            (n <= MAX_EVENTS).then_some(n + 1)
        })
        .ok()?;
    if index == MAX_EVENTS {
        return Some("SMITH_INSPECTION_DIAG phase=limit code=LIMIT".into());
    }
    Some(format!(
        "SMITH_INSPECTION_DIAG phase={} code={}",
        phase.label(),
        safe_code(code)
    ))
}

fn event(phase: Phase, code: &str) {
    #[cfg(feature = "inspection-diagnostics")]
    {
        use std::io::Write;
        static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        if let Some(line) = line(&COUNT, phase, code) {
            // A closed diagnostic sink cannot turn success into failure or skip cleanup.
            let _ = writeln!(std::io::stderr().lock(), "{line}");
        }
    }
    #[cfg(not(feature = "inspection-diagnostics"))]
    let _ = (phase.label(), safe_code(code));
}

pub(crate) fn app<T>(phase: Phase, operation: impl FnOnce() -> AppResult<T>) -> AppResult<T> {
    event(phase, "START");
    let result = operation();
    event(
        phase,
        result.as_ref().err().map_or("OK", |error| &error.code),
    );
    result
}

pub(crate) fn local<T>(phase: Phase, operation: impl FnOnce() -> LocalResult<T>) -> LocalResult<T> {
    event(phase, "START");
    let result = operation();
    let code = match result.as_ref().err() {
        None => "OK",
        Some(LocalError::Prerequisite(_)) => "PREREQUISITE",
        Some(LocalError::RuntimeUnavailable) => "RUNTIME_UNAVAILABLE",
        Some(LocalError::RepositoryRejected) => "REPOSITORY_REJECTED",
        Some(LocalError::RepositoryUnavailable) => "REPOSITORY_UNAVAILABLE",
        Some(LocalError::ReadDenied) => "READ_DENIED",
        Some(LocalError::ModelUnavailable) => "MODEL_UNAVAILABLE",
        Some(LocalError::ModelProtocol) => "MODEL_PROTOCOL",
        Some(LocalError::ModelLimit) => "MODEL_LIMIT",
        Some(LocalError::Interrupted) => "INTERRUPTED",
        Some(LocalError::Deadline) => "DEADLINE",
        Some(LocalError::Publication) => "PUBLICATION",
        Some(LocalError::Protocol) => "PROTOCOL",
        Some(LocalError::Shutdown) => "SHUTDOWN",
    };
    event(phase, code);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn diagnostics_only_render_finite_phase_and_code_not_private_text() {
        let count = AtomicUsize::new(0);
        let raw = "secret-prompt\n/path/to/private-state TOKEN=secret";
        let output = line(&count, Phase::ValidateResponse, raw).unwrap();
        assert_eq!(
            output,
            "SMITH_INSPECTION_DIAG phase=validate_response code=OTHER"
        );
        assert!(!output.contains("secret"));
        assert_eq!(safe_code("PROVIDER_PROTOCOL"), "PROVIDER_PROTOCOL");
    }

    #[test]
    fn every_requested_boundary_has_a_distinct_static_phase() {
        let phases = [
            Phase::BrokerRequest,
            Phase::BrokerValidateRequest,
            Phase::BrokerResponse,
            Phase::BrokerValidateResponse,
            Phase::BrokerReadEvidence,
            Phase::ProviderValidateRequest,
            Phase::ProviderFrozenPreflight,
            Phase::RunnerValidateRequest,
            Phase::Props,
            Phase::SemanticProbeRender,
            Phase::SemanticProbeVerify,
            Phase::ActualTemplate,
            Phase::Tokenize,
            Phase::Context,
            Phase::NativeCompletion,
            Phase::ValidateResponse,
        ];
        let count = AtomicUsize::new(0);
        let mut labels = std::collections::HashSet::new();
        for phase in phases {
            assert!(labels.insert(phase.label()));
            let output = line(&count, phase, "PROVIDER_PROTOCOL").unwrap();
            assert_eq!(
                output,
                format!(
                    "SMITH_INSPECTION_DIAG phase={} code=PROVIDER_PROTOCOL",
                    phase.label()
                )
            );
        }
    }

    #[test]
    fn diagnostics_are_process_bounded_with_one_saturation_marker() {
        let count = AtomicUsize::new(0);
        for _ in 0..MAX_EVENTS {
            assert!(line(&count, Phase::Props, "OK").is_some());
        }
        assert_eq!(
            line(&count, Phase::Props, "OK").unwrap(),
            "SMITH_INSPECTION_DIAG phase=limit code=LIMIT"
        );
        assert!(line(&count, Phase::Props, "OK").is_none());
    }

    #[test]
    fn diagnostics_preserve_success_and_original_error_without_replay() {
        let mut calls = 0;
        let result = app(Phase::ProviderFrozenPreflight, || {
            calls += 1;
            Err::<(), _>(AppError::new("PROVIDER_PROTOCOL", "private message"))
        });
        assert_eq!(calls, 1);
        let error = result.err().unwrap();
        assert_eq!(error.code, "PROVIDER_PROTOCOL");
        assert_eq!(error.message, "private message");
        assert_eq!(app(Phase::Context, || Ok(42)).unwrap(), 42);
        assert!(matches!(
            local::<()>(Phase::BrokerResponse, || Err(LocalError::ModelProtocol)),
            Err(LocalError::ModelProtocol)
        ));
    }

    #[test]
    fn no_read_final_answer_stays_rejected_after_validated_response() {
        let request = serde_json::json!({"model":"selected","stream":true,
                    "messages":[{"role":"system","content":crate::dev_local::SYSTEM_PROMPT},
                                {"role":"user","content":"fixture"}],
                    "tools":[{"type":"function","function":{"name":"read","parameters":{
                        "type":"object","properties":{"filePath":{"type":"string"}},
                        "required":["filePath"],"additionalProperties":false}}}]});
        let mut broker = crate::dev_broker::Broker::new("fixture".into(), 1).unwrap();
        local(Phase::BrokerRequest, || broker.request(&request)).unwrap();
        let response = serde_json::json!({"model":"selected","choices":[{"index":0,
            "finish_reason":"stop","message":{"role":"assistant","content":"fixture"}}]});
        assert!(matches!(
            local(Phase::BrokerResponse, || broker.response(&response)),
            Err(LocalError::ModelProtocol)
        ));
    }

    #[test]
    #[ignore = "explicit private captured fixture only; pure validators, never inference"]
    fn captured_frozen_request_no_inference() {
        use std::io::Read;
        let path = std::env::var_os("SMITH_INSPECTION_FIXTURE").expect("explicit fixture required");
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .expect("private fixture open")
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .expect("private fixture read");
        assert!(bytes.len() <= 1024 * 1024, "fixture bound");
        let value = crate::cloud::unique_json(&bytes).expect("fixture JSON");
        let job: crate::dev_local::LocalJob =
            serde_json::from_value(value["job"].clone()).expect("fixture job");
        let checkouts: Vec<crate::dev_local::Checkout> =
            serde_json::from_value(value["checkouts"].clone()).expect("fixture checkouts");
        let prompt =
            crate::dev_local::prompt_for(&job, &checkouts).expect("frozen checkout binding");
        let mut broker =
            crate::dev_broker::Broker::new(prompt, job.repositories.len()).expect("broker");
        let request = local(Phase::BrokerRequest, || broker.request(&value["request"]));
        assert!(
            request.is_ok(),
            "captured broker request rejected; see finite diagnostics"
        );
        let request = request.unwrap();
        assert!(
            app(Phase::ProviderValidateRequest, || {
                crate::dev_completion::validate_request(&request)
            })
            .is_ok(),
            "captured provider request rejected; see finite diagnostics"
        );
    }
}
