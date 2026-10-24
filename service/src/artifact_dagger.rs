//! Host adapter boundary, unwired from service execution. The optional process
//! launcher lives in `artifact_dagger_process`; this module performs no I/O itself.
//!
//! `Runner` is a trusted injection point, not an implementation of Dagger lifecycle
//! or network policy. A future launcher must verify the declared CLI/module bytes,
//! isolate its environment, enforce limits and positively establish owned cleanup.
//! The JSON below is a proposed module protocol, not a claim about Dagger CLI stdin.
//! Returned production text is never check evidence or an accepted artifact.

use crate::artifact_pipeline::{AttemptId, ContentHash, Failure, Phase, Pipeline, Status};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

pub const PROTOCOL_VERSION: u8 = 1;
pub const MAX_INPUT: usize = 16 * 1024;
/// Complete serialized request, including JSON escaping and identity fields.
pub const MAX_REQUEST: usize = 32 * 1024;
pub const MAX_STDOUT: usize = 64 * 1024;
pub const MAX_STDERR: usize = 64 * 1024;

#[derive(Clone)]
pub struct ModuleBinding {
    executable: PathBuf,
    module: PathBuf,
    executable_hash: ContentHash,
    module_hash: ContentHash,
}

impl ModuleBinding {
    /// Expected identities supplied by the trusted owner; performs no filesystem I/O.
    pub fn new(
        executable: PathBuf,
        module: PathBuf,
        executable_hash: ContentHash,
        module_hash: ContentHash,
    ) -> Result<Self, Error> {
        if !absolute_leaf(&executable) || !absolute_leaf(&module) {
            return Err(Error::InvalidBinding);
        }
        Ok(Self {
            executable,
            module,
            executable_hash,
            module_hash,
        })
    }
    pub fn executable(&self) -> &Path {
        &self.executable
    }
    pub fn module(&self) -> &Path {
        &self.module
    }
    pub fn executable_hash(&self) -> ContentHash {
        self.executable_hash
    }
    pub fn module_hash(&self) -> ContentHash {
        self.module_hash
    }
}

fn absolute_leaf(path: &Path) -> bool {
    path.is_absolute()
        && path.file_name().is_some()
        && !path.as_os_str().as_encoded_bytes().contains(&0)
        && path
            .components()
            .all(|c| matches!(c, Component::RootDir | Component::Normal(_)))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[derive(Serialize)]
struct Request<'a> {
    schema_version: u8,
    run_id: String,
    phase: &'static str,
    attempt: u8,
    contract_sha256: String,
    executable_sha256: String,
    module_sha256: String,
    input: &'a str,
}

/// Data for the eventual fixed module call, never a shell command or argv template.
pub struct Invocation {
    binding: ModuleBinding,
    attempt: AttemptId,
    request: Vec<u8>,
    request_hash: ContentHash,
    remaining: Duration,
    environment: BTreeMap<&'static str, &'static str>,
}
impl Invocation {
    pub fn binding(&self) -> &ModuleBinding {
        &self.binding
    }
    pub fn attempt(&self) -> AttemptId {
        self.attempt
    }
    pub fn request(&self) -> &[u8] {
        &self.request
    }
    pub fn request_hash(&self) -> ContentHash {
        self.request_hash
    }
    pub fn remaining(&self) -> Duration {
        self.remaining
    }
    /// Exact allowlist. A launcher must clear ambient variables before applying it.
    /// These settings alone do not prove that engine-side traces are disabled.
    pub fn environment(&self) -> &BTreeMap<&'static str, &'static str> {
        &self.environment
    }
}

pub struct Observation {
    pub exit_code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// Elapsed from original run admission, not from this individual invocation.
    pub elapsed: Duration,
    /// Trusted runner observation, never copied from the module's JSON response.
    pub owned_cleanup_verified: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunnerError {
    Unavailable,
    Interrupted,
    OutcomeUnknown,
}

pub trait Runner {
    /// Enforce `remaining` and bounded captures while running, not just afterward.
    /// Failure must retain exclusion if owned engine execution may still be active.
    fn run(&mut self, invocation: &Invocation) -> Result<Observation, RunnerError>;
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    schema_version: u8,
    request_sha256: String,
    outcome: Outcome,
    body: Option<String>,
    error: Option<UpstreamError>,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Outcome {
    Produced,
    Failed,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum UpstreamError {
    Authentication,
    DefaultUnavailable,
    Quota,
    Provider,
    PolicyDenied,
    Prerequisite,
}

pub struct Produced {
    attempt: AttemptId,
    request_hash: ContentHash,
    body: String,
}
impl Produced {
    pub fn attempt(&self) -> AttemptId {
        self.attempt
    }
    pub fn request_hash(&self) -> ContentHash {
        self.request_hash
    }
    pub fn body(&self) -> &str {
        &self.body
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidBinding,
    InvalidInput,
    DuplicateAttempt,
    State,
    Deadline,
    Protocol,
    Limit,
    Authentication,
    DefaultUnavailable,
    Quota,
    Provider,
    Interrupted,
    OutcomeUnknown,
    CleanupUnknown,
    PolicyDenied,
    Prerequisite,
}

pub struct Adapter {
    binding: ModuleBinding,
    dispatched: Vec<AttemptId>,
}
impl Adapter {
    pub fn new(binding: ModuleBinding) -> Self {
        Self {
            binding,
            dispatched: Vec::new(),
        }
    }

    /// Consumes an attempt before invoking the runner. No retry on a lost response.
    /// This in-memory guard is not durable replay protection across service restart.
    pub fn dispatch(
        &mut self,
        pipeline: &mut Pipeline,
        input: &str,
        elapsed: Duration,
        runner: &mut dyn Runner,
    ) -> Result<Produced, Error> {
        if input.trim().is_empty() || input.len() > MAX_INPUT || input.contains('\0') {
            return Err(Error::InvalidInput);
        }
        pipeline.tick(elapsed).map_err(|_| Error::State)?;
        if pipeline.status() == &Status::Stopped(Failure::Deadline) {
            return Err(Error::Deadline);
        }
        let attempt = pipeline.current_attempt().ok_or(Error::State)?;
        if self.dispatched.contains(&attempt) {
            return Err(Error::DuplicateAttempt);
        }
        // One adapter belongs to one run: prevent unbounded retained attempt identities.
        if self
            .dispatched
            .first()
            .is_some_and(|id| id.run() != attempt.run())
        {
            return Err(Error::State);
        }
        let phase = match attempt.phase() {
            Phase::Setup => "setup",
            Phase::Execute => "execute",
            Phase::Submit => "submit",
        };
        let request = serde_json::to_vec(&Request {
            schema_version: PROTOCOL_VERSION,
            run_id: hex(attempt.run().bytes()),
            phase,
            attempt: attempt.number(),
            contract_sha256: hex(pipeline.contract_hash().bytes()),
            executable_sha256: hex(self.binding.executable_hash.bytes()),
            module_sha256: hex(self.binding.module_hash.bytes()),
            input,
        })
        .map_err(|_| Error::Protocol)?;
        if request.len() > MAX_REQUEST {
            return Err(Error::InvalidInput);
        }
        let invocation = Invocation {
            binding: self.binding.clone(),
            attempt,
            request_hash: ContentHash::of(&request),
            request,
            remaining: pipeline.deadline() - elapsed,
            environment: BTreeMap::from([
                ("DAGGER_CLOUD_TOKEN", ""),
                ("DAGGER_NO_NAG", "1"),
                ("DAGGER_PROGRESS", "plain"),
                ("DO_NOT_TRACK", "1"),
            ]),
        };
        self.dispatched.push(attempt);
        let observation = runner.run(&invocation).map_err(|error| {
            let (failure, result) = match error {
                RunnerError::Unavailable => (Failure::Provider, Error::Provider),
                RunnerError::Interrupted => (Failure::Interrupted, Error::Interrupted),
                RunnerError::OutcomeUnknown => (Failure::Unknown, Error::OutcomeUnknown),
            };
            stop(pipeline, failure, elapsed, result)
        })?;
        let observed = observation.elapsed;
        if observed < elapsed {
            return Err(stop(pipeline, Failure::Protocol, elapsed, Error::Protocol));
        }
        pipeline.tick(observed).map_err(|_| Error::State)?;
        if pipeline.status() == &Status::Stopped(Failure::Deadline) {
            return Err(Error::Deadline);
        }
        if !observation.owned_cleanup_verified {
            return Err(stop(
                pipeline,
                Failure::Cleanup,
                observed,
                Error::CleanupUnknown,
            ));
        }
        if observation.stdout.len() > MAX_STDOUT || observation.stderr.len() > MAX_STDERR {
            return Err(stop(pipeline, Failure::Protocol, observed, Error::Limit));
        }
        if observation.exit_code.is_none() {
            return Err(stop(
                pipeline,
                Failure::Unknown,
                observed,
                Error::OutcomeUnknown,
            ));
        }
        // A returned failed envelope is still a successful Dagger function call.
        // Conversely, nonzero CLI exit is transport failure, not a provider classifier.
        if observation.exit_code != Some(0) {
            return Err(stop(pipeline, Failure::Protocol, observed, Error::Protocol));
        }
        let response = crate::cloud::unique_json(&observation.stdout)
            .ok()
            .and_then(|value| serde_json::from_value::<Response>(value).ok())
            .filter(|r| {
                r.schema_version == PROTOCOL_VERSION
                    && r.request_sha256 == hex(invocation.request_hash.bytes())
            })
            .ok_or_else(|| stop(pipeline, Failure::Protocol, observed, Error::Protocol))?;
        match response.outcome {
            Outcome::Produced => {
                let body = response.body.filter(|body| {
                    !body.trim().is_empty() && body.len() <= MAX_INPUT && !body.contains('\0')
                });
                if response.error.is_some() || body.is_none() {
                    return Err(stop(pipeline, Failure::Protocol, observed, Error::Protocol));
                }
                Ok(Produced {
                    attempt,
                    request_hash: invocation.request_hash,
                    body: body.expect("validated body"),
                })
            }
            Outcome::Failed => {
                if response.body.is_some() {
                    return Err(stop(pipeline, Failure::Protocol, observed, Error::Protocol));
                }
                let (failure, error) = match response.error {
                    Some(UpstreamError::Authentication) => {
                        (Failure::Authentication, Error::Authentication)
                    }
                    Some(UpstreamError::DefaultUnavailable) => {
                        (Failure::Provider, Error::DefaultUnavailable)
                    }
                    Some(UpstreamError::Quota) => (Failure::Provider, Error::Quota),
                    Some(UpstreamError::Provider) => (Failure::Provider, Error::Provider),
                    Some(UpstreamError::PolicyDenied) => {
                        (Failure::PolicyDenied, Error::PolicyDenied)
                    }
                    Some(UpstreamError::Prerequisite) => {
                        (Failure::Prerequisite, Error::Prerequisite)
                    }
                    None => (Failure::Protocol, Error::Protocol),
                };
                Err(stop(pipeline, failure, observed, error))
            }
        }
    }
}

fn stop(pipeline: &mut Pipeline, failure: Failure, elapsed: Duration, error: Error) -> Error {
    let _ = pipeline.stop(failure, elapsed);
    error
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact_pipeline::{
        Artifact, Candidate, Check, CheckEvidence, CheckOutcome, CleanupOutcome, Review, RunId,
        SessionId,
    };
    use serde_json::json;

    struct Fake {
        calls: usize,
        response: serde_json::Value,
        exit: Option<i32>,
        cleanup: bool,
        elapsed: Duration,
        error: Option<RunnerError>,
        foreign: bool,
        extra_stderr: bool,
        expected_phase: &'static str,
        expected_attempt: u8,
        expected_remaining: Duration,
        replay_hash: Option<ContentHash>,
    }
    impl Fake {
        fn produced() -> Self {
            Self {
                calls: 0,
                response: json!({"outcome":"produced","body":"candidate only","error":null}),
                exit: Some(0),
                cleanup: true,
                elapsed: Duration::from_secs(2),
                error: None,
                foreign: false,
                extra_stderr: false,
                expected_phase: "setup",
                expected_attempt: 1,
                expected_remaining: Duration::from_secs(99),
                replay_hash: None,
            }
        }
    }
    impl Runner for Fake {
        fn run(&mut self, invocation: &Invocation) -> Result<Observation, RunnerError> {
            self.calls += 1;
            assert_eq!(invocation.remaining(), self.expected_remaining);
            let request: serde_json::Value = serde_json::from_slice(invocation.request()).unwrap();
            assert_eq!(request["phase"], self.expected_phase);
            assert_eq!(request["attempt"], self.expected_attempt);
            assert_eq!(invocation.attempt().number(), self.expected_attempt);
            assert!(request.get("model").is_none());
            assert!(request.get("provider").is_none());
            assert_eq!(invocation.environment().len(), 4);
            assert!(!invocation.environment().contains_key("HOME"));
            assert!(!invocation.environment().contains_key("OPENAI_API_KEY"));
            if let Some(error) = self.error {
                return Err(error);
            }
            let mut response = self.response.clone();
            response["schema_version"] = json!(PROTOCOL_VERSION);
            response["request_sha256"] = json!(if self.foreign {
                "0".repeat(64)
            } else {
                hex(self
                    .replay_hash
                    .unwrap_or(invocation.request_hash())
                    .bytes())
            });
            Ok(Observation {
                exit_code: self.exit,
                stdout: serde_json::to_vec(&response).unwrap(),
                stderr: if self.extra_stderr {
                    vec![b'x'; MAX_STDERR + 1]
                } else {
                    b"not forwarded".to_vec()
                },
                elapsed: self.elapsed,
                owned_cleanup_verified: self.cleanup,
            })
        }
    }
    fn pair() -> (Adapter, Pipeline) {
        let binding = ModuleBinding::new(
            PathBuf::from("/approved/dagger"),
            PathBuf::from("/approved/module"),
            ContentHash::of(b"cli"),
            ContentHash::of(b"module"),
        )
        .unwrap();
        let pipeline = Pipeline::new(
            RunId::new([1; 16]).unwrap(),
            b"fixed contract",
            Duration::from_secs(100),
        )
        .unwrap();
        (Adapter::new(binding), pipeline)
    }
    fn dispatch(
        adapter: &mut Adapter,
        pipeline: &mut Pipeline,
        fake: &mut Fake,
    ) -> Result<Produced, Error> {
        adapter.dispatch(
            pipeline,
            "input is data; not a shell command",
            Duration::from_secs(1),
            fake,
        )
    }
    #[test]
    fn produced_text_does_not_advance_gate_and_attempt_cannot_be_replayed() {
        let (mut adapter, mut pipeline) = pair();
        let mut fake = Fake::produced();
        let output = dispatch(&mut adapter, &mut pipeline, &mut fake).unwrap();
        assert_eq!(output.body(), "candidate only");
        assert_eq!(Some(output.attempt()), pipeline.current_attempt());
        assert!(pipeline.records().is_empty());
        assert!(matches!(pipeline.status(), Status::Producing(_)));
        assert!(matches!(
            adapter.dispatch(&mut pipeline, "same", Duration::from_secs(2), &mut fake),
            Err(Error::DuplicateAttempt)
        ));
        assert_eq!(fake.calls, 1);
    }
    #[test]
    fn full_fake_lifecycle_repairs_execute_and_rejects_old_attempt_responses() {
        for replay_old_response in [false, true] {
            let (mut adapter, mut pipeline) = pair();
            let mut hashes = Vec::new();
            let mut prior_evidence: Option<(Candidate, CheckEvidence, Option<Review>)> = None;
            for (index, (phase, number, start)) in [
                ("setup", 1, 1),
                ("execute", 1, 3),
                ("execute", 2, 5),
                ("submit", 1, 7),
            ]
            .into_iter()
            .enumerate()
            {
                let mut fake = Fake::produced();
                fake.expected_phase = phase;
                fake.expected_attempt = number;
                fake.expected_remaining = Duration::from_secs(100 - start);
                fake.elapsed = Duration::from_secs(start + 1);
                if replay_old_response && index == 2 {
                    fake.replay_hash = Some(hashes[1]);
                }
                let result = adapter.dispatch(
                    &mut pipeline,
                    "fixed input",
                    Duration::from_secs(start),
                    &mut fake,
                );
                if replay_old_response && index == 2 {
                    assert!(matches!(result, Err(Error::Protocol)));
                    assert_eq!(pipeline.status(), &Status::Stopped(Failure::Protocol));
                    assert_eq!(fake.calls, 1);
                    break;
                }
                let produced = result.unwrap();
                assert!(!hashes.contains(&produced.request_hash()));
                hashes.push(produced.request_hash());
                let observed = Duration::from_secs(start + 1);
                assert!(matches!(
                    adapter.dispatch(&mut pipeline, "fixed input", observed, &mut fake),
                    Err(Error::DuplicateAttempt)
                ));
                assert_eq!(fake.calls, 1);
                if index == 2 {
                    let (candidate, checks, review) = prior_evidence.clone().unwrap();
                    assert_eq!(
                        pipeline.evaluate(candidate, checks, review, observed),
                        Err(crate::artifact_pipeline::Error::StaleEvidence)
                    );
                    assert_eq!(pipeline.current_attempt(), Some(produced.attempt()));
                }
                let id = produced.attempt();
                let artifacts = if id.phase() == Phase::Execute {
                    vec![
                        Artifact::new("changes.patch", b"checked fixture changes".to_vec())
                            .unwrap(),
                    ]
                } else {
                    Vec::new()
                };
                let references = if id.phase() == Phase::Submit {
                    pipeline
                        .accepted_execute()
                        .unwrap()
                        .artifacts()
                        .iter()
                        .map(Artifact::hash)
                        .collect()
                } else {
                    Vec::new()
                };
                let candidate = Candidate::new(
                    id,
                    pipeline.contract_hash(),
                    SessionId::new([10 + index as u8; 16]).unwrap(),
                    produced.body().to_owned(),
                    artifacts,
                    references,
                )
                .unwrap();
                let checks = CheckEvidence::new(
                    id,
                    candidate.hash(),
                    id.phase()
                        .required_checks()
                        .iter()
                        .map(|check| {
                            let outcome = if index == 1 && *check == Check::FixedTests {
                                CheckOutcome::Rejected
                            } else {
                                CheckOutcome::Passed
                            };
                            (*check, outcome)
                        })
                        .collect(),
                )
                .unwrap();
                let review = id.phase().requires_review().then(|| {
                    Review::new(
                        id,
                        candidate.hash(),
                        checks.hash(),
                        SessionId::new([100 + index as u8; 16]).unwrap(),
                        true,
                        Vec::new(),
                    )
                    .unwrap()
                });
                if index == 1 {
                    prior_evidence = Some((candidate.clone(), checks.clone(), review.clone()));
                }
                pipeline
                    .evaluate(candidate, checks, review, observed)
                    .unwrap();
                if index == 1 {
                    assert!(matches!(pipeline.status(), Status::Repairable(_)));
                    let repaired = pipeline.repair(observed).unwrap().unwrap();
                    assert_eq!(repaired.phase(), Phase::Execute);
                    assert_eq!(repaired.number(), 2);
                    assert_eq!(pipeline.deadline(), Duration::from_secs(100));
                }
            }
            if !replay_old_response {
                assert_eq!(pipeline.status(), &Status::AwaitingCleanup);
                let binding = pipeline.cleanup_binding().unwrap();
                pipeline
                    .confirm_cleanup(
                        binding,
                        CleanupOutcome::VerifiedOwnedQuiescence,
                        Duration::from_secs(9),
                    )
                    .unwrap();
                assert_eq!(pipeline.status(), &Status::Complete);
            }
        }
    }

    #[test]
    fn declared_binding_rejects_ambient_and_parent_paths() {
        for path in ["dagger", "/", "/approved/../other"] {
            assert!(ModuleBinding::new(
                PathBuf::from(path),
                PathBuf::from("/module"),
                ContentHash::of(b"a"),
                ContentHash::of(b"b")
            )
            .is_err());
        }
    }
    #[test]
    fn qa_serialized_request_limit_refuses_before_attempt_consumption() {
        let (mut adapter, mut pipeline) = pair();
        let mut fake = Fake::produced();
        let baseline = "def clamp(value, low, high):\n    if value < low:\n        return high\n    if value > high:\n        return low\n    return value\n";
        let payload = serde_json::to_string(&json!({
            "schema_version": 1,
            "baseline": {"path": "clamp.py", "content": baseline,
                "sha256": hex(ContentHash::of(baseline.as_bytes()).bytes())}
        }))
        .unwrap();
        let input = "\n".repeat(MAX_INPUT - payload.len()) + &payload;
        assert_eq!(input.len(), MAX_INPUT);
        assert!(serde_json::from_str::<serde_json::Value>(&input).is_ok());
        assert!(matches!(
            adapter.dispatch(&mut pipeline, &input, Duration::from_secs(1), &mut fake),
            Err(Error::InvalidInput)
        ));
        assert_eq!(fake.calls, 0);
        assert!(adapter.dispatched.is_empty());
        assert!(pipeline.current_attempt().is_some());
        let shorter = &input[200..];
        assert!(adapter
            .dispatch(&mut pipeline, shorter, Duration::from_secs(1), &mut fake)
            .is_ok());
        assert_eq!(fake.calls, 1);
    }

    #[test]
    fn invalid_input_and_expired_deadline_never_invoke_runner() {
        let (mut adapter, mut pipeline) = pair();
        let mut fake = Fake::produced();
        for input in ["".to_string(), "\0".to_string(), "x".repeat(MAX_INPUT + 1)] {
            assert!(matches!(
                adapter.dispatch(&mut pipeline, &input, Duration::from_secs(1), &mut fake),
                Err(Error::InvalidInput)
            ));
        }
        assert!(matches!(
            adapter.dispatch(&mut pipeline, "task", Duration::from_secs(100), &mut fake),
            Err(Error::Deadline)
        ));
        assert_eq!(fake.calls, 0);
    }
    #[test]
    fn provider_errors_stop_without_repair_or_raw_error_forwarding() {
        for (code, expected) in [
            ("authentication", Error::Authentication),
            ("default_unavailable", Error::DefaultUnavailable),
            ("quota", Error::Quota),
            ("provider", Error::Provider),
            ("policy_denied", Error::PolicyDenied),
            ("prerequisite", Error::Prerequisite),
        ] {
            let (mut adapter, mut pipeline) = pair();
            let mut fake = Fake::produced();
            fake.response = json!({"outcome":"failed","body":null,"error":code});
            fake.exit = Some(0);
            assert!(
                matches!(dispatch(&mut adapter, &mut pipeline, &mut fake), Err(error) if error == expected)
            );
            assert!(matches!(pipeline.status(), Status::Stopped(_)));
            assert!(pipeline.repair(Duration::from_secs(3)).is_err());
        }
    }
    #[test]
    fn nonzero_cli_exit_does_not_turn_stderr_or_failed_envelope_into_authentication() {
        let (mut adapter, mut pipeline) = pair();
        let mut fake = Fake::produced();
        fake.response = json!({"outcome":"failed","body":null,"error":"authentication"});
        fake.exit = Some(1);
        assert!(matches!(
            dispatch(&mut adapter, &mut pipeline, &mut fake),
            Err(Error::Protocol)
        ));
        assert_eq!(pipeline.status(), &Status::Stopped(Failure::Protocol));
    }

    #[test]
    fn unknown_cleanup_foreign_response_and_malformed_success_never_advance() {
        for kind in 0..7 {
            let (mut adapter, mut pipeline) = pair();
            let mut fake = Fake::produced();
            match kind {
                0 => fake.cleanup = false,
                1 => fake.foreign = true,
                2 => fake.exit = Some(1),
                3 => fake.response["extra"] = json!("untrusted"),
                4 => fake.response["error"] = json!("authentication"),
                5 => fake.exit = None,
                _ => fake.extra_stderr = true,
            }
            assert!(dispatch(&mut adapter, &mut pipeline, &mut fake).is_err());
            assert!(matches!(pipeline.status(), Status::Stopped(_)));
            assert!(pipeline.records().is_empty());
            assert_eq!(fake.calls, 1);
        }
    }
    #[test]
    fn interrupted_or_unknown_execution_never_retries() {
        for error in [
            RunnerError::Unavailable,
            RunnerError::Interrupted,
            RunnerError::OutcomeUnknown,
        ] {
            let (mut adapter, mut pipeline) = pair();
            let mut fake = Fake::produced();
            fake.error = Some(error);
            assert!(dispatch(&mut adapter, &mut pipeline, &mut fake).is_err());
            assert!(dispatch(&mut adapter, &mut pipeline, &mut fake).is_err());
            assert_eq!(fake.calls, 1);
        }
    }
    #[test]
    fn original_deadline_and_monotonic_clock_cover_runner_observation() {
        for elapsed in [Duration::ZERO, Duration::from_secs(100)] {
            let (mut adapter, mut pipeline) = pair();
            let mut fake = Fake::produced();
            fake.elapsed = elapsed;
            assert!(dispatch(&mut adapter, &mut pipeline, &mut fake).is_err());
            assert!(matches!(pipeline.status(), Status::Stopped(_)));
        }
    }
}
