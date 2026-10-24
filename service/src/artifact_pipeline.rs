//! Source-only, in-memory gates for the fixed artifact pipeline; no execution authority.
//!
//! The caller supplies elapsed time since admission (never per-attempt time), fresh
//! session identities and trusted check/cleanup observations. Hashes bind evidence,
//! not its authenticity. An adapter must enforce session isolation, actual checks,
//! export containment/no symlinks and owned quiescence before reporting success.
//! There is deliberately no deserialization/resume path or production executor.

use sha2::{Digest, Sha256};
use std::time::Duration;

pub const MAX_ATTEMPTS: u8 = 2;
pub const MAX_CANDIDATE_BYTES: usize = 16 * 1024;
pub const MAX_ARTIFACT_BYTES: usize = 64 * 1024;
pub const MAX_TOTAL_ARTIFACT_BYTES: usize = 256 * 1024;
pub const MAX_ARTIFACTS: usize = 8;
pub const MAX_FINDINGS: usize = 8;
/// A source-contract ceiling, not a validated coding-runtime budget.
pub const MAX_DEADLINE: Duration = Duration::from_secs(3600);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RunId([u8; 16]);

impl RunId {
    pub fn new(value: [u8; 16]) -> Result<Self, Error> {
        if value == [0; 16] {
            return Err(Error::InvalidIdentity);
        }
        Ok(Self(value))
    }

    pub fn bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SessionId([u8; 16]);

impl SessionId {
    pub fn new(value: [u8; 16]) -> Result<Self, Error> {
        if value == [0; 16] {
            return Err(Error::InvalidIdentity);
        }
        Ok(Self(value))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    pub fn of(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    pub fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Phase {
    Setup,
    Execute,
    Submit,
}

impl Phase {
    pub fn required_checks(self) -> &'static [Check] {
        match self {
            Self::Setup => &[Check::Harness, Check::Inputs, Check::Workspace],
            Self::Execute => &[Check::FixedTests, Check::ArtifactIdentity],
            Self::Submit => &[Check::ArtifactReferences, Check::SupportedClaims],
        }
    }

    pub fn requires_review(self) -> bool {
        self != Self::Setup
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttemptId {
    run: RunId,
    phase: Phase,
    number: u8,
}

impl AttemptId {
    pub fn run(self) -> RunId {
        self.run
    }
    pub fn phase(self) -> Phase {
        self.phase
    }
    pub fn number(self) -> u8 {
        self.number
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    Harness,
    Inputs,
    Workspace,
    FixedTests,
    ArtifactIdentity,
    ArtifactReferences,
    SupportedClaims,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckOutcome {
    Passed,
    Rejected,
    Skipped,
    Unknown,
}

/// Allowlisted findings, not arbitrary upstream diagnostics or private reasoning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finding {
    Preparation,
    Tests,
    Artifact,
    Claims,
    Limitations,
    References,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artifact {
    path: String,
    bytes: Vec<u8>,
    hash: ContentHash,
}

impl Artifact {
    /// `path` is a logical export name, never authority to open a host path.
    pub fn new(path: &str, bytes: Vec<u8>) -> Result<Self, Error> {
        if path.is_empty()
            || path.len() > 128
            || path.split('/').any(|part| {
                part.is_empty()
                    || part == "."
                    || part == ".."
                    || !part
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            })
            || bytes.is_empty()
            || bytes.len() > MAX_ARTIFACT_BYTES
        {
            return Err(Error::InvalidArtifact);
        }
        Ok(Self {
            path: path.into(),
            hash: ContentHash::of(&bytes),
            bytes,
        })
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn hash(&self) -> ContentHash {
        self.hash
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    attempt: AttemptId,
    contract: ContentHash,
    producer: SessionId,
    body: String,
    artifacts: Vec<Artifact>,
    references: Vec<ContentHash>,
    hash: ContentHash,
}

impl Candidate {
    /// Setup has no exports; Execute has at least one; Submit references exactly
    /// the accepted Execute exports and cannot replace them. Body is bounded data.
    pub fn new(
        attempt: AttemptId,
        contract: ContentHash,
        producer: SessionId,
        body: String,
        artifacts: Vec<Artifact>,
        references: Vec<ContentHash>,
    ) -> Result<Self, Error> {
        if body.trim().is_empty()
            || body.len() > MAX_CANDIDATE_BYTES
            || body.contains('\0')
            || artifacts.len() > MAX_ARTIFACTS
            || references.len() > MAX_ARTIFACTS
            || artifacts.iter().map(|a| a.bytes.len()).sum::<usize>() > MAX_TOTAL_ARTIFACT_BYTES
        {
            return Err(Error::InvalidCandidate);
        }
        for (index, artifact) in artifacts.iter().enumerate() {
            if artifacts[..index]
                .iter()
                .any(|a| a.path == artifact.path || a.hash == artifact.hash)
            {
                return Err(Error::InvalidArtifact);
            }
        }
        for (index, reference) in references.iter().enumerate() {
            if references[..index].contains(reference) {
                return Err(Error::InvalidArtifact);
            }
        }
        match attempt.phase {
            Phase::Setup if !artifacts.is_empty() || !references.is_empty() => {
                return Err(Error::InvalidCandidate);
            }
            Phase::Execute if artifacts.is_empty() || !references.is_empty() => {
                return Err(Error::InvalidCandidate);
            }
            Phase::Submit if !artifacts.is_empty() || references.is_empty() => {
                return Err(Error::InvalidCandidate);
            }
            _ => {}
        }
        let mut digest = Sha256::new();
        digest.update(b"smith-artifact-candidate-v1");
        bind_attempt(&mut digest, attempt);
        digest.update(contract.0);
        digest.update(producer.0);
        bind_bytes(&mut digest, body.as_bytes());
        digest.update((artifacts.len() as u64).to_be_bytes());
        for artifact in &artifacts {
            bind_bytes(&mut digest, artifact.path.as_bytes());
            digest.update(artifact.hash.0);
            digest.update((artifact.bytes.len() as u64).to_be_bytes());
        }
        digest.update((references.len() as u64).to_be_bytes());
        for reference in &references {
            digest.update(reference.0);
        }
        Ok(Self {
            attempt,
            contract,
            producer,
            body,
            artifacts,
            references,
            hash: ContentHash(digest.finalize().into()),
        })
    }
    pub fn attempt(&self) -> AttemptId {
        self.attempt
    }
    pub fn hash(&self) -> ContentHash {
        self.hash
    }
    pub fn body(&self) -> &str {
        &self.body
    }
    pub fn artifacts(&self) -> &[Artifact] {
        &self.artifacts
    }
    pub fn references(&self) -> &[ContentHash] {
        &self.references
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckEvidence {
    attempt: AttemptId,
    candidate: ContentHash,
    outcomes: Vec<(Check, CheckOutcome)>,
    hash: ContentHash,
}

impl CheckEvidence {
    /// A missing, duplicate or unexpected check is malformed, not a pass.
    pub fn new(
        attempt: AttemptId,
        candidate: ContentHash,
        outcomes: Vec<(Check, CheckOutcome)>,
    ) -> Result<Self, Error> {
        let required = attempt.phase.required_checks();
        if outcomes.len() != required.len()
            || required
                .iter()
                .any(|check| outcomes.iter().filter(|(c, _)| c == check).count() != 1)
        {
            return Err(Error::InvalidChecks);
        }
        // Canonical order makes the binding independent of transport ordering.
        let outcomes: Vec<_> = required
            .iter()
            .map(|check| {
                *outcomes
                    .iter()
                    .find(|(c, _)| c == check)
                    .expect("validated check set")
            })
            .collect();
        let mut digest = Sha256::new();
        digest.update(b"smith-artifact-checks-v1");
        bind_attempt(&mut digest, attempt);
        digest.update(candidate.0);
        for (check, outcome) in &outcomes {
            digest.update([*check as u8, *outcome as u8]);
        }
        Ok(Self {
            attempt,
            candidate,
            outcomes,
            hash: ContentHash(digest.finalize().into()),
        })
    }
    pub fn hash(&self) -> ContentHash {
        self.hash
    }
    pub fn outcomes(&self) -> &[(Check, CheckOutcome)] {
        &self.outcomes
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Review {
    attempt: AttemptId,
    candidate: ContentHash,
    checks: ContentHash,
    session: SessionId,
    accepted: bool,
    findings: Vec<Finding>,
}

impl Review {
    pub fn new(
        attempt: AttemptId,
        candidate: ContentHash,
        checks: ContentHash,
        session: SessionId,
        accepted: bool,
        findings: Vec<Finding>,
    ) -> Result<Self, Error> {
        if findings.len() > MAX_FINDINGS || accepted != findings.is_empty() {
            return Err(Error::InvalidReview);
        }
        Ok(Self {
            attempt,
            candidate,
            checks,
            session,
            accepted,
            findings,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Authentication,
    Provider,
    PolicyDenied,
    Prerequisite,
    Protocol,
    Unknown,
    Interrupted,
    Cleanup,
    Deadline,
    AttemptsExhausted,
    /// Submit discovered an artifact defect: do not repair just the response.
    ArtifactDefect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Producing(AttemptId),
    Repairable(AttemptId),
    AwaitingCleanup,
    Complete,
    Stopped(Failure),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidIdentity,
    InvalidDeadline,
    InvalidCandidate,
    InvalidArtifact,
    InvalidChecks,
    InvalidReview,
    StaleEvidence,
    HashMismatch,
    SessionReused,
    WrongState,
    Terminal,
    ClockRegressed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CleanupBinding {
    run: RunId,
    submit: AttemptId,
    candidate: ContentHash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CleanupOutcome {
    VerifiedOwnedQuiescence,
    Unknown,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttemptRecord {
    candidate: Candidate,
    checks: CheckEvidence,
    review: Option<Review>,
    accepted: bool,
    findings: Vec<Finding>,
}

impl AttemptRecord {
    pub fn candidate(&self) -> &Candidate {
        &self.candidate
    }
    pub fn checks(&self) -> &CheckEvidence {
        &self.checks
    }
    pub fn review(&self) -> Option<&Review> {
        self.review.as_ref()
    }
    /// Historical phase-gate result, not overall completion or current validity.
    pub fn accepted(&self) -> bool {
        self.accepted
    }
    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Pipeline {
    run: RunId,
    contract: ContentHash,
    deadline: Duration,
    elapsed: Duration,
    status: Status,
    records: Vec<AttemptRecord>,
    sessions: Vec<SessionId>,
}

impl Pipeline {
    /// Contract is the exact task plus acceptance specification, admitted once.
    pub fn new(run: RunId, contract: &[u8], deadline: Duration) -> Result<Self, Error> {
        if contract.is_empty() || contract.len() > MAX_CANDIDATE_BYTES {
            return Err(Error::InvalidCandidate);
        }
        if deadline.is_zero() || deadline > MAX_DEADLINE {
            return Err(Error::InvalidDeadline);
        }
        Ok(Self {
            run,
            contract: ContentHash::of(contract),
            deadline,
            elapsed: Duration::ZERO,
            status: Status::Producing(AttemptId {
                run,
                phase: Phase::Setup,
                number: 1,
            }),
            records: Vec::new(),
            sessions: Vec::new(),
        })
    }
    pub fn status(&self) -> &Status {
        &self.status
    }
    pub fn contract_hash(&self) -> ContentHash {
        self.contract
    }
    pub fn deadline(&self) -> Duration {
        self.deadline
    }
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }
    pub fn records(&self) -> &[AttemptRecord] {
        &self.records
    }

    /// Validated exports can survive response/cleanup failure, but not a known
    /// artifact defect found during Submit. Historical evidence remains intact.
    pub fn accepted_execute(&self) -> Option<&Candidate> {
        if self.status == Status::Stopped(Failure::ArtifactDefect)
            || self.records.iter().any(|r| {
                r.candidate.attempt.phase == Phase::Submit
                    && r.findings.contains(&Finding::Artifact)
            })
        {
            return None;
        }
        self.records
            .iter()
            .find(|r| r.accepted && r.candidate.attempt.phase == Phase::Execute)
            .map(|r| &r.candidate)
    }

    pub fn completed_response(&self) -> Option<&Candidate> {
        if self.status != Status::Complete {
            return None;
        }
        self.records.last().map(|r| &r.candidate)
    }

    pub fn current_attempt(&self) -> Option<AttemptId> {
        match self.status {
            Status::Producing(id) => Some(id),
            _ => None,
        }
    }

    /// Identity/hash/session errors refuse atomically, including the clock.
    /// Valid but unusable observations stop; only known rejection permits repair.
    /// An error never authorizes redispatch. The adapter must report malformed
    /// transport/review responses with `stop(Failure::Protocol, elapsed)`.
    pub fn evaluate(
        &mut self,
        candidate: Candidate,
        checks: CheckEvidence,
        review: Option<Review>,
        elapsed: Duration,
    ) -> Result<(), Error> {
        self.preflight(elapsed)?;
        let Status::Producing(attempt) = self.status else {
            return Err(Error::WrongState);
        };
        if candidate.attempt != attempt
            || checks.attempt != attempt
            || review.as_ref().is_some_and(|r| r.attempt != attempt)
        {
            return Err(Error::StaleEvidence);
        }
        if candidate.contract != self.contract
            || checks.candidate != candidate.hash
            || review
                .as_ref()
                .is_some_and(|r| r.candidate != candidate.hash || r.checks != checks.hash)
        {
            return Err(Error::HashMismatch);
        }
        if self.sessions.contains(&candidate.producer)
            || review.as_ref().is_some_and(|r| {
                r.session == candidate.producer || self.sessions.contains(&r.session)
            })
        {
            return Err(Error::SessionReused);
        }
        if attempt.phase == Phase::Submit {
            let execute = self
                .records
                .iter()
                .find(|r| r.accepted && r.candidate.attempt.phase == Phase::Execute)
                .expect("Submit follows accepted Execute");
            let expected: Vec<_> = execute.candidate.artifacts.iter().map(|a| a.hash).collect();
            if candidate.references.len() != expected.len()
                || expected
                    .iter()
                    .any(|hash| !candidate.references.contains(hash))
            {
                return Err(Error::HashMismatch);
            }
        }
        if !self.observe_time(elapsed) {
            return Ok(());
        }
        self.sessions.push(candidate.producer);
        if let Some(review) = &review {
            self.sessions.push(review.session);
        }
        let uncertain = checks
            .outcomes
            .iter()
            .any(|(_, o)| *o == CheckOutcome::Unknown);
        let skipped = checks
            .outcomes
            .iter()
            .any(|(_, o)| *o == CheckOutcome::Skipped);
        let mut findings = Vec::new();
        for (check, outcome) in &checks.outcomes {
            if *outcome == CheckOutcome::Rejected {
                let finding = match check {
                    Check::Harness | Check::Inputs | Check::Workspace => Finding::Preparation,
                    Check::FixedTests => Finding::Tests,
                    Check::ArtifactIdentity => Finding::Artifact,
                    Check::ArtifactReferences => Finding::References,
                    Check::SupportedClaims => Finding::Claims,
                };
                if !findings.contains(&finding) {
                    findings.push(finding);
                }
            }
        }
        if let Some(review) = &review {
            for finding in &review.findings {
                if !findings.contains(finding) {
                    findings.push(*finding);
                }
            }
        }
        let failure = if uncertain {
            Some(Failure::Unknown)
        } else if skipped {
            Some(Failure::Protocol)
        } else if attempt.phase == Phase::Submit && findings.contains(&Finding::Artifact) {
            Some(Failure::ArtifactDefect)
        } else if findings.is_empty() && attempt.phase.requires_review() && review.is_none() {
            Some(Failure::Protocol)
        } else {
            None
        };
        let accepted = failure.is_none() && findings.is_empty();
        self.records.push(AttemptRecord {
            candidate,
            checks,
            review,
            accepted,
            findings,
        });
        self.status = if let Some(failure) = failure {
            Status::Stopped(failure)
        } else if !accepted {
            if attempt.number == MAX_ATTEMPTS {
                Status::Stopped(Failure::AttemptsExhausted)
            } else {
                Status::Repairable(attempt)
            }
        } else {
            match attempt.phase {
                Phase::Setup => Status::Producing(AttemptId {
                    run: self.run,
                    phase: Phase::Execute,
                    number: 1,
                }),
                Phase::Execute => Status::Producing(AttemptId {
                    run: self.run,
                    phase: Phase::Submit,
                    number: 1,
                }),
                Phase::Submit => Status::AwaitingCleanup,
            }
        };
        Ok(())
    }

    /// Returns a new attempt only if the original deadline has not expired.
    pub fn repair(&mut self, elapsed: Duration) -> Result<Option<AttemptId>, Error> {
        self.preflight(elapsed)?;
        let Status::Repairable(previous) = self.status else {
            return Err(Error::WrongState);
        };
        if !self.observe_time(elapsed) {
            return Ok(None);
        }
        let next = AttemptId {
            number: previous.number + 1,
            ..previous
        };
        self.status = Status::Producing(next);
        Ok(Some(next))
    }

    /// Report uncertain execution directly; never convert it into a repair.
    pub fn stop(&mut self, failure: Failure, elapsed: Duration) -> Result<(), Error> {
        self.preflight(elapsed)?;
        if self.observe_time(elapsed) {
            self.status = Status::Stopped(failure);
        }
        Ok(())
    }

    pub fn cleanup_binding(&self) -> Option<CleanupBinding> {
        if self.status != Status::AwaitingCleanup {
            return None;
        }
        let candidate = &self.records.last()?.candidate;
        Some(CleanupBinding {
            run: self.run,
            submit: candidate.attempt,
            candidate: candidate.hash,
        })
    }

    /// Only the trusted owner may report verified cleanup; this is not a probe.
    pub fn confirm_cleanup(
        &mut self,
        binding: CleanupBinding,
        outcome: CleanupOutcome,
        elapsed: Duration,
    ) -> Result<(), Error> {
        self.preflight(elapsed)?;
        let expected = self.cleanup_binding().ok_or(Error::WrongState)?;
        if binding != expected {
            return Err(Error::StaleEvidence);
        }
        if self.observe_time(elapsed) {
            self.status = match outcome {
                CleanupOutcome::VerifiedOwnedQuiescence => Status::Complete,
                CleanupOutcome::Unknown => Status::Stopped(Failure::Unknown),
                CleanupOutcome::Failed => Status::Stopped(Failure::Cleanup),
            };
        }
        Ok(())
    }

    /// Advance the original clock even when no phase evidence is available.
    pub fn tick(&mut self, elapsed: Duration) -> Result<(), Error> {
        self.preflight(elapsed)?;
        self.observe_time(elapsed);
        Ok(())
    }

    fn preflight(&self, elapsed: Duration) -> Result<(), Error> {
        if matches!(self.status, Status::Complete | Status::Stopped(_)) {
            return Err(Error::Terminal);
        }
        if elapsed < self.elapsed {
            return Err(Error::ClockRegressed);
        }
        Ok(())
    }
    fn observe_time(&mut self, elapsed: Duration) -> bool {
        self.elapsed = elapsed;
        if elapsed >= self.deadline {
            self.status = Status::Stopped(Failure::Deadline);
            false
        } else {
            true
        }
    }
}

fn bind_bytes(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
}

fn bind_attempt(digest: &mut Sha256, attempt: AttemptId) {
    digest.update(attempt.run.0);
    digest.update([attempt.phase as u8, attempt.number]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pipeline() -> Pipeline {
        Pipeline::new(
            RunId::new([1; 16]).unwrap(),
            b"task and acceptance",
            Duration::from_secs(100),
        )
        .unwrap()
    }
    fn session(value: u8) -> SessionId {
        SessionId::new([value; 16]).unwrap()
    }
    fn candidate(p: &Pipeline) -> Candidate {
        let id = p.current_attempt().unwrap();
        let producer = session(10 + id.phase as u8 * 4 + id.number * 2);
        let artifacts = if id.phase == Phase::Execute {
            vec![Artifact::new("changes/patch.txt", b"fixture patch".to_vec()).unwrap()]
        } else {
            vec![]
        };
        let references = if id.phase == Phase::Submit {
            p.records
                .iter()
                .find(|r| r.accepted && r.candidate.attempt.phase == Phase::Execute)
                .unwrap()
                .candidate
                .artifacts
                .iter()
                .map(Artifact::hash)
                .collect()
        } else {
            vec![]
        };
        Candidate::new(
            id,
            p.contract_hash(),
            producer,
            "bounded candidate".into(),
            artifacts,
            references,
        )
        .unwrap()
    }
    fn evidence(c: &Candidate, outcome: CheckOutcome) -> CheckEvidence {
        CheckEvidence::new(
            c.attempt,
            c.hash,
            c.attempt
                .phase
                .required_checks()
                .iter()
                .map(|check| (*check, outcome))
                .collect(),
        )
        .unwrap()
    }
    fn review(c: &Candidate, checks: &CheckEvidence, accepted: bool) -> Review {
        Review::new(
            c.attempt,
            c.hash,
            checks.hash,
            session(c.producer.0[0] + 1),
            accepted,
            if accepted {
                vec![]
            } else {
                vec![Finding::Claims]
            },
        )
        .unwrap()
    }
    fn pass(p: &mut Pipeline, elapsed: u64) {
        let c = candidate(p);
        let checks = evidence(&c, CheckOutcome::Passed);
        let review = c
            .attempt
            .phase
            .requires_review()
            .then(|| review(&c, &checks, true));
        p.evaluate(c, checks, review, Duration::from_secs(elapsed))
            .unwrap();
    }
    fn submit_ready() -> Pipeline {
        let mut p = pipeline();
        pass(&mut p, 1);
        pass(&mut p, 2);
        pass(&mut p, 3);
        p
    }

    #[test]
    fn positive_path_requires_separate_cleanup() {
        let mut p = submit_ready();
        assert_eq!(p.status(), &Status::AwaitingCleanup);
        assert!(p.completed_response().is_none());
        assert_eq!(p.records().len(), 3);
        assert!(p.records().iter().all(AttemptRecord::accepted));
        p.confirm_cleanup(
            p.cleanup_binding().unwrap(),
            CleanupOutcome::VerifiedOwnedQuiescence,
            Duration::from_secs(4),
        )
        .unwrap();
        assert_eq!(p.status(), &Status::Complete);
        assert!(p.completed_response().is_some());
    }

    #[test]
    fn deterministic_failure_overrides_accepted_review_and_repair_is_new() {
        let mut p = pipeline();
        pass(&mut p, 1);
        let c = candidate(&p);
        let old = c.attempt;
        let checks = evidence(&c, CheckOutcome::Rejected);
        let review = review(&c, &checks, true);
        p.evaluate(c, checks, Some(review), Duration::from_secs(2))
            .unwrap();
        assert_eq!(p.status(), &Status::Repairable(old));
        assert!(!p.records()[1].accepted());
        assert!(p.records()[1].findings().contains(&Finding::Tests));
        let new = p.repair(Duration::from_secs(3)).unwrap().unwrap();
        assert_eq!(new.number(), 2);
        assert_ne!(new, old);
        pass(&mut p, 4);
        assert_eq!(p.current_attempt().unwrap().phase(), Phase::Submit);
    }

    #[test]
    fn stale_attempt_and_hash_mismatch_refuse_atomically() {
        let mut p = pipeline();
        let c = candidate(&p);
        let stale = evidence(&c, CheckOutcome::Passed);
        pass(&mut p, 1);
        let before = format!("{p:?}");
        let next = candidate(&p);
        assert_eq!(
            p.evaluate(next, stale, None, Duration::from_secs(2)),
            Err(Error::StaleEvidence)
        );
        assert_eq!(format!("{p:?}"), before);
        let c = candidate(&p);
        let wrong = CheckEvidence::new(
            c.attempt,
            ContentHash::of(b"other"),
            c.attempt
                .phase
                .required_checks()
                .iter()
                .map(|check| (*check, CheckOutcome::Passed))
                .collect(),
        )
        .unwrap();
        assert_eq!(
            p.evaluate(c, wrong, None, Duration::from_secs(2)),
            Err(Error::HashMismatch)
        );
        assert_eq!(format!("{p:?}"), before);
    }

    #[test]
    fn exhausted_rejection_is_terminal() {
        let mut p = pipeline();
        for number in 1..=2 {
            let c = candidate(&p);
            let checks = evidence(&c, CheckOutcome::Rejected);
            p.evaluate(c, checks, None, Duration::from_secs(number * 2))
                .unwrap();
            if number == 1 {
                p.repair(Duration::from_secs(3)).unwrap();
            }
        }
        assert_eq!(p.status(), &Status::Stopped(Failure::AttemptsExhausted));
        assert_eq!(p.repair(Duration::from_secs(5)), Err(Error::Terminal));
    }

    #[test]
    fn original_deadline_never_resets_and_clock_cannot_regress() {
        let mut p = pipeline();
        let c = candidate(&p);
        let checks = evidence(&c, CheckOutcome::Rejected);
        p.evaluate(c, checks, None, Duration::from_secs(90))
            .unwrap();
        p.repair(Duration::from_secs(95)).unwrap();
        assert_eq!(p.tick(Duration::from_secs(94)), Err(Error::ClockRegressed));
        assert_eq!(p.elapsed(), Duration::from_secs(95));
        pass(&mut p, 99);
        p.tick(Duration::from_secs(100)).unwrap();
        assert_eq!(p.status(), &Status::Stopped(Failure::Deadline));
        assert_eq!(p.deadline(), Duration::from_secs(100));
    }

    #[test]
    fn cleanup_unknown_failed_or_expired_cannot_complete() {
        for (outcome, elapsed, failure) in [
            (CleanupOutcome::Unknown, 4, Failure::Unknown),
            (CleanupOutcome::Failed, 4, Failure::Cleanup),
            (
                CleanupOutcome::VerifiedOwnedQuiescence,
                100,
                Failure::Deadline,
            ),
        ] {
            let mut p = submit_ready();
            p.confirm_cleanup(
                p.cleanup_binding().unwrap(),
                outcome,
                Duration::from_secs(elapsed),
            )
            .unwrap();
            assert_eq!(p.status(), &Status::Stopped(failure));
            assert_eq!(p.records().len(), 3);
            assert!(p.accepted_execute().is_some());
            assert!(p.completed_response().is_none());
            assert_eq!(p.tick(Duration::from_secs(101)), Err(Error::Terminal));
        }
    }

    #[test]
    fn terminal_state_is_immutable_for_every_mutator() {
        for complete in [false, true] {
            let mut p = submit_ready();
            let binding = p.cleanup_binding().unwrap();
            if complete {
                p.confirm_cleanup(
                    binding,
                    CleanupOutcome::VerifiedOwnedQuiescence,
                    Duration::from_secs(4),
                )
                .unwrap();
            } else {
                p.stop(Failure::Interrupted, Duration::from_secs(4))
                    .unwrap();
            }
            let before = format!("{p:?}");
            let record = p.records[2].clone();
            assert_eq!(
                p.evaluate(
                    record.candidate,
                    record.checks,
                    record.review,
                    Duration::from_secs(5)
                ),
                Err(Error::Terminal)
            );
            assert_eq!(p.repair(Duration::from_secs(5)), Err(Error::Terminal));
            assert_eq!(
                p.stop(Failure::Unknown, Duration::from_secs(5)),
                Err(Error::Terminal)
            );
            assert_eq!(
                p.confirm_cleanup(
                    binding,
                    CleanupOutcome::VerifiedOwnedQuiescence,
                    Duration::from_secs(5)
                ),
                Err(Error::Terminal)
            );
            assert_eq!(p.tick(Duration::from_secs(5)), Err(Error::Terminal));
            assert_eq!(format!("{p:?}"), before);
        }
    }

    #[test]
    fn missing_skipped_unknown_and_rejected_reviews_never_advance() {
        for outcome in [
            CheckOutcome::Passed,
            CheckOutcome::Skipped,
            CheckOutcome::Unknown,
        ] {
            let mut p = pipeline();
            pass(&mut p, 1);
            let c = candidate(&p);
            let checks = evidence(&c, outcome);
            p.evaluate(c, checks, None, Duration::from_secs(2)).unwrap();
            assert!(matches!(p.status(), Status::Stopped(_)));
        }
        let mut p = pipeline();
        pass(&mut p, 1);
        let c = candidate(&p);
        let checks = evidence(&c, CheckOutcome::Passed);
        let rejected = review(&c, &checks, false);
        p.evaluate(c, checks, Some(rejected), Duration::from_secs(2))
            .unwrap();
        assert!(matches!(p.status(), Status::Repairable(_)));
    }

    #[test]
    fn strict_check_and_review_construction() {
        let p = pipeline();
        let c = candidate(&p);
        assert_eq!(
            CheckEvidence::new(c.attempt, c.hash, vec![]),
            Err(Error::InvalidChecks)
        );
        assert_eq!(
            CheckEvidence::new(
                c.attempt,
                c.hash,
                vec![(Check::Harness, CheckOutcome::Passed); 3]
            ),
            Err(Error::InvalidChecks)
        );
        assert_eq!(
            Review::new(
                c.attempt,
                c.hash,
                c.hash,
                session(2),
                true,
                vec![Finding::Claims]
            ),
            Err(Error::InvalidReview)
        );
        assert_eq!(
            Review::new(c.attempt, c.hash, c.hash, session(2), false, vec![]),
            Err(Error::InvalidReview)
        );
    }

    #[test]
    fn review_requires_fresh_session_and_current_check_hash() {
        let mut p = pipeline();
        pass(&mut p, 1);
        let c = candidate(&p);
        let checks = evidence(&c, CheckOutcome::Passed);
        for session in [c.producer, p.sessions[0]] {
            let r = Review::new(c.attempt, c.hash, checks.hash, session, true, vec![]).unwrap();
            assert_eq!(
                p.evaluate(c.clone(), checks.clone(), Some(r), Duration::from_secs(2)),
                Err(Error::SessionReused)
            );
        }
        let r = Review::new(
            c.attempt,
            c.hash,
            ContentHash::of(b"stale checks"),
            session(99),
            true,
            vec![],
        )
        .unwrap();
        assert_eq!(
            p.evaluate(c, checks, Some(r), Duration::from_secs(2)),
            Err(Error::HashMismatch)
        );
        assert_eq!(p.elapsed(), Duration::from_secs(1));
    }

    #[test]
    fn artifact_paths_sizes_duplicates_and_submit_references_are_bounded() {
        for path in [
            "/abs",
            "../escape",
            "a/../b",
            "a//b",
            "a\\b",
            "a/./b",
            "a/",
            "",
        ] {
            assert_eq!(Artifact::new(path, vec![1]), Err(Error::InvalidArtifact));
        }
        assert_eq!(
            Artifact::new("x", vec![0; MAX_ARTIFACT_BYTES + 1]),
            Err(Error::InvalidArtifact)
        );
        let mut p = pipeline();
        pass(&mut p, 1);
        let id = p.current_attempt().unwrap();
        let a = Artifact::new("x", vec![1]).unwrap();
        assert_eq!(
            Candidate::new(
                id,
                p.contract,
                session(55),
                "x".into(),
                vec![a.clone(), a],
                vec![]
            ),
            Err(Error::InvalidArtifact)
        );
        pass(&mut p, 2);
        let c = Candidate::new(
            p.current_attempt().unwrap(),
            p.contract,
            session(56),
            "reply".into(),
            vec![],
            vec![ContentHash::of(b"wrong")],
        )
        .unwrap();
        let checks = evidence(&c, CheckOutcome::Passed);
        let r = review(&c, &checks, true);
        assert_eq!(
            p.evaluate(c, checks, Some(r), Duration::from_secs(3)),
            Err(Error::HashMismatch)
        );
        assert_eq!(p.records().len(), 2);
    }

    #[test]
    fn uncertain_failures_never_offer_repair() {
        for failure in [
            Failure::Authentication,
            Failure::Provider,
            Failure::Protocol,
            Failure::Unknown,
            Failure::Interrupted,
            Failure::Cleanup,
        ] {
            let mut p = pipeline();
            p.stop(failure, Duration::from_secs(1)).unwrap();
            assert_eq!(p.status(), &Status::Stopped(failure));
            assert_eq!(p.repair(Duration::from_secs(2)), Err(Error::Terminal));
        }
    }

    #[test]
    fn deadline_at_repair_returns_no_dispatch_identity() {
        let mut p = pipeline();
        let c = candidate(&p);
        let checks = evidence(&c, CheckOutcome::Rejected);
        p.evaluate(c, checks, None, Duration::from_secs(99))
            .unwrap();
        assert_eq!(p.repair(Duration::from_secs(100)).unwrap(), None);
        assert_eq!(p.status(), &Status::Stopped(Failure::Deadline));
        assert!(p.current_attempt().is_none());
    }

    #[test]
    fn repaired_attempt_cannot_consume_previous_attempt_evidence() {
        let mut p = pipeline();
        let c = candidate(&p);
        let checks = evidence(&c, CheckOutcome::Rejected);
        p.evaluate(c.clone(), checks.clone(), None, Duration::from_secs(1))
            .unwrap();
        p.repair(Duration::from_secs(2)).unwrap();
        let before = format!("{p:?}");
        assert_eq!(
            p.evaluate(c, checks, None, Duration::from_secs(3)),
            Err(Error::StaleEvidence)
        );
        assert_eq!(format!("{p:?}"), before);
    }

    #[test]
    fn cleanup_binding_cannot_cross_runs_or_precede_submit() {
        let mut p = pipeline();
        let other = submit_ready().cleanup_binding().unwrap();
        assert_eq!(
            p.confirm_cleanup(
                other,
                CleanupOutcome::VerifiedOwnedQuiescence,
                Duration::from_secs(1)
            ),
            Err(Error::WrongState)
        );
        let mut p = Pipeline::new(
            RunId::new([2; 16]).unwrap(),
            b"task and acceptance",
            Duration::from_secs(100),
        )
        .unwrap();
        pass(&mut p, 1);
        pass(&mut p, 2);
        pass(&mut p, 3);
        let before = format!("{p:?}");
        assert_eq!(
            p.confirm_cleanup(
                other,
                CleanupOutcome::VerifiedOwnedQuiescence,
                Duration::from_secs(4)
            ),
            Err(Error::StaleEvidence)
        );
        assert_eq!(format!("{p:?}"), before);
    }

    #[test]
    fn admission_and_candidate_limits_cannot_be_bypassed() {
        assert_eq!(RunId::new([0; 16]), Err(Error::InvalidIdentity));
        assert_eq!(SessionId::new([0; 16]), Err(Error::InvalidIdentity));
        let run = RunId::new([1; 16]).unwrap();
        for deadline in [Duration::ZERO, MAX_DEADLINE + Duration::from_secs(1)] {
            assert_eq!(
                Pipeline::new(run, b"task", deadline),
                Err(Error::InvalidDeadline)
            );
        }
        assert_eq!(
            Pipeline::new(run, &[], Duration::from_secs(1)),
            Err(Error::InvalidCandidate)
        );
        let mut p = pipeline();
        pass(&mut p, 1);
        let id = p.current_attempt().unwrap();
        let artifact = Artifact::new("patch", vec![1]).unwrap();
        assert_eq!(
            Candidate::new(
                id,
                p.contract,
                session(55),
                "x".repeat(MAX_CANDIDATE_BYTES + 1),
                vec![artifact],
                vec![]
            ),
            Err(Error::InvalidCandidate)
        );
        let artifacts: Vec<_> = (0..5)
            .map(|n| Artifact::new(&format!("export-{n}"), vec![n; MAX_ARTIFACT_BYTES]).unwrap())
            .collect();
        assert_eq!(
            Candidate::new(id, p.contract, session(55), "x".into(), artifacts, vec![]),
            Err(Error::InvalidCandidate)
        );
    }

    #[test]
    fn submit_reference_check_rejection_preserves_exports_and_bounds_response_repair() {
        for exhaust in [false, true] {
            let mut p = pipeline();
            pass(&mut p, 1);
            pass(&mut p, 2);
            let execute_hash = p.accepted_execute().unwrap().hash();

            let c = candidate(&p);
            let first = c.attempt();
            let checks = CheckEvidence::new(
                first,
                c.hash(),
                vec![
                    (Check::ArtifactReferences, CheckOutcome::Rejected),
                    (Check::SupportedClaims, CheckOutcome::Passed),
                ],
            )
            .unwrap();
            let r = review(&c, &checks, true);
            p.evaluate(c, checks, Some(r), Duration::from_secs(3))
                .unwrap();
            assert_eq!(p.status(), &Status::Repairable(first));
            assert_eq!(
                p.records().last().unwrap().findings(),
                &[Finding::References]
            );
            assert_eq!(p.accepted_execute().unwrap().hash(), execute_hash);
            assert!(p.completed_response().is_none());

            let second = p.repair(Duration::from_secs(4)).unwrap().unwrap();
            assert_eq!(second.phase(), Phase::Submit);
            assert_eq!(second.number(), 2);
            assert_ne!(first, second);
            if exhaust {
                let c = candidate(&p);
                let checks = CheckEvidence::new(
                    second,
                    c.hash(),
                    vec![
                        (Check::ArtifactReferences, CheckOutcome::Rejected),
                        (Check::SupportedClaims, CheckOutcome::Passed),
                    ],
                )
                .unwrap();
                let r = review(&c, &checks, true);
                p.evaluate(c, checks, Some(r), Duration::from_secs(5))
                    .unwrap();
                assert_eq!(p.status(), &Status::Stopped(Failure::AttemptsExhausted));
                assert_eq!(p.repair(Duration::from_secs(6)), Err(Error::Terminal));
                assert!(p.completed_response().is_none());
            } else {
                pass(&mut p, 5);
                assert_eq!(p.status(), &Status::AwaitingCleanup);
                p.confirm_cleanup(
                    p.cleanup_binding().unwrap(),
                    CleanupOutcome::VerifiedOwnedQuiescence,
                    Duration::from_secs(6),
                )
                .unwrap();
                assert!(p.completed_response().is_some());
            }
            assert_eq!(p.accepted_execute().unwrap().hash(), execute_hash);
        }
    }

    #[test]
    fn submit_artifact_defect_invalidates_delivery_not_response_repair() {
        let mut p = pipeline();
        pass(&mut p, 1);
        pass(&mut p, 2);
        let c = candidate(&p);
        let checks = evidence(&c, CheckOutcome::Passed);
        let r = Review::new(
            c.attempt,
            c.hash,
            checks.hash,
            session(99),
            false,
            vec![Finding::Artifact],
        )
        .unwrap();
        p.evaluate(c, checks, Some(r), Duration::from_secs(3))
            .unwrap();
        assert_eq!(p.status(), &Status::Stopped(Failure::ArtifactDefect));
        assert_eq!(p.repair(Duration::from_secs(4)), Err(Error::Terminal));
        assert!(p.accepted_execute().is_none());
        assert!(p.completed_response().is_none());
        assert!(p.cleanup_binding().is_none());
    }
}
