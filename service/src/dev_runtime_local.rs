//! Host supervisor for the local-only worker. Docker helpers retain exact owned cleanup.
use super::*;
use crate::dev_broker::Broker;
use crate::dev_fetch::FetchSession;
use crate::dev_local::{
    self as protocol, Frame, LocalError, LocalEvent, LocalFailure, LocalJob, LocalReceipt,
    LocalResult, LocalRuntime,
};
use crate::providers::AppResult;

use std::io::{BufReader, Read, Write};

/// Network-less, unprivileged, read-only worker shape shared by inspection and export.
#[rustfmt::skip]
pub(super) const ISOLATION: &[&str] = &[
    "--no-healthcheck", "--network=none", "--user=1000:1000", "--read-only",
    "--cap-drop=ALL", "--security-opt=no-new-privileges:true", "--cpus=2",
    "--memory=2g", "--memory-swap=2g", "--pids-limit=128", "--restart=no",
    "--tmpfs=/tmp:rw,nosuid,nodev,noexec,size=268435456,nr_inodes=32768,mode=1777",
    "--tmpfs=/workspace:rw,nosuid,nodev,noexec,size=268435456,nr_inodes=32768,mode=0700,uid=1000,gid=1000",
    "--workdir=/workspace", "--interactive",
];

pub(super) fn expect_eof<R: Read + AsRawFd>(
    output: &mut BufReader<R>,
    work: Instant,
    cancel: &dyn Fn() -> bool,
) -> LocalResult<()> {
    let mut byte = [0];
    loop {
        if cancel() || Instant::now() >= work {
            return Err(LocalError::Shutdown);
        }
        if output.buffer().is_empty() {
            dev_worker::wait_io(output.get_ref().as_raw_fd(), libc::POLLIN, work, cancel)
                .map_err(|_| LocalError::Shutdown)?;
        }
        match output.read(&mut byte) {
            Ok(0) => return Ok(()),
            Ok(_) => return Err(LocalError::Protocol),
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) => {}
            Err(_) => return Err(LocalError::Shutdown),
        }
    }
}

pub(crate) struct DockerLocalRuntime {
    docker: DockerRuntime,
}
impl DockerLocalRuntime {
    pub(crate) fn new(runtime_root: PathBuf) -> Self {
        Self {
            docker: DockerRuntime::new_rootful(runtime_root),
        }
    }
}
pub(super) fn prerequisite(error: RuntimeDiagnostic) -> LocalError {
    LocalError::Prerequisite(error)
}
pub(super) fn next_sequence(value: u32, expected: &mut u32) -> LocalResult<()> {
    if value != *expected || value > 512 {
        return Err(LocalError::Protocol);
    }
    *expected += 1;
    Ok(())
}
pub(super) fn respond<W: Write + AsRawFd>(
    input: &mut W,
    id: &str,
    sequence: u32,
    content_type: &str,
    bytes: &[u8],
    until: Instant,
    cancel: &dyn Fn() -> bool,
) -> LocalResult<()> {
    protocol::send(
        input,
        &Frame::ResponseStart {
            request_id: id.into(),
            sequence,
            success: true,
            content_type: content_type.into(),
        },
        until,
        cancel,
    )?;
    if bytes.is_empty() {
        protocol::send(
            input,
            &Frame::ResponseChunk {
                request_id: id.into(),
                sequence,
                bytes: vec![],
                last: true,
            },
            until,
            cancel,
        )?;
    } else {
        let count = bytes.len().div_ceil(protocol::CHUNK);
        for (index, bytes) in bytes.chunks(protocol::CHUNK).enumerate() {
            protocol::send(
                input,
                &Frame::ResponseChunk {
                    request_id: id.into(),
                    sequence,
                    bytes: bytes.to_vec(),
                    last: index + 1 == count,
                },
                until,
                cancel,
            )?;
        }
    }
    Ok(())
}
fn token(value: &str, prefix: &str) -> bool {
    value.starts_with(prefix)
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}
impl LocalRuntime for DockerLocalRuntime {
    fn run(
        &self,
        job: &LocalJob,
        cancelled: &AtomicBool,
        event: &mut dyn FnMut(LocalEvent) -> AppResult<()>,
        completion: &mut dyn FnMut(&Value, Instant) -> AppResult<Value>,
    ) -> std::result::Result<LocalReceipt, LocalFailure> {
        let started = Instant::now();
        let total = started + Duration::from_secs(protocol::JOB_SECONDS);
        let work = total - Duration::from_secs(protocol::CLEANUP_SECONDS);
        let prep = (started + Duration::from_secs(protocol::PREP_SECONDS)).min(work);
        let cancel = || cancelled.load(Ordering::Acquire);
        let mut may_have_sent = false;
        let mut guard = ContainerGuard {
            runtime: &self.docker,
            request: &job.request_id,
            id: None,
            owner: String::new(),
            ambiguous: false,
            armed: false,
        };
        let result = (|| -> LocalResult<LocalReceipt> {
            if !crate::dev_worker::valid_id(&job.request_id)
                || job.repositories.is_empty()
                || job.repositories.len() > 16
                || job.prompt.is_empty()
                || job.prompt.len() > protocol::MAX_PROMPT
            {
                return Err(LocalError::Protocol);
            }
            let mut fetch = FetchSession::new(job.repositories.clone())?;
            let container_name = name(&job.request_id).map_err(|_| LocalError::Protocol)?;
            self.docker
                .verify_daemon(prep.min(Instant::now() + Duration::from_secs(5)), &cancel)
                .map_err(prerequisite)?;
            if self
                .docker
                .owned(&job.request_id, prep, &cancel)
                .map_err(prerequisite)?
                .is_some()
            {
                return Err(LocalError::RuntimeUnavailable);
            }
            let image = self
                .docker
                .json(&["image", "inspect", protocol::IMAGE], prep, &cancel)
                .map_err(|e| prerequisite(e.during(RuntimeDiagnostic::ImageInspectionFailed)))?;
            let image = worker_image(&image).map_err(prerequisite)?;
            event(LocalEvent::Preparing {
                image_id: image.clone(),
            })
            .map_err(|_| LocalError::Publication)?;
            let mut nonce = [0u8; 16];
            rand::rngs::OsRng
                .try_fill_bytes(&mut nonce)
                .map_err(|_| LocalError::RuntimeUnavailable)?;
            guard.owner = nonce.iter().map(|b| format!("{b:02x}")).collect();
            guard.armed = true;
            guard.ambiguous = true;
            let owner_label = format!("{OWNER_LABEL}={}", guard.owner);
            let request_label = format!("{LABEL}={}", job.request_id);
            let kind_label = format!("{KIND_LABEL}={}", dev_worker::MARKER);
            let marker = format!("SMITH_DEV_LOCAL_WORKER={}", protocol::POLICY);
            let mut options = vec![
                "--pull=never",
                "--name",
                &container_name,
                "--label",
                &request_label,
                "--label",
                &kind_label,
                "--label",
                &owner_label,
            ];
            options.extend_from_slice(ISOLATION);
            options.extend([
                "--env",
                &marker,
                "--entrypoint=/usr/local/bin/smith-service",
                &image,
                "dev-local-worker",
                &job.request_id,
            ]);
            let created = self
                .docker
                .capture(&create_arguments(&options), prep, &cancel)
                .map_err(prerequisite)?;
            let id = std::str::from_utf8(&created)
                .map_err(|_| LocalError::Protocol)?
                .trim();
            if !hex(id, 64) {
                return Err(LocalError::Protocol);
            }
            guard.id = Some(id.into());
            let identity = self
                .docker
                .owned(&job.request_id, prep, &cancel)
                .map_err(prerequisite)?
                .ok_or(LocalError::Protocol)?;
            if identity.id != id
                || identity.owner != guard.owner
                || identity.image != image
                || identity.running
            {
                return Err(LocalError::Protocol);
            }
            guard.ambiguous = false;
            event(LocalEvent::Cloning).map_err(|_| LocalError::Publication)?;
            let mut command = self.docker.command().map_err(prerequisite)?;
            command
                .args(["container", "start", "--attach", "--interactive", id])
                .stdin(Stdio::piped());
            guard.ambiguous = true;
            let mut child = OwnedChild(
                command
                    .spawn()
                    .map_err(|_| LocalError::RuntimeUnavailable)?,
            );
            let mut input = child.0.stdin.take().ok_or(LocalError::Protocol)?;
            let output = child.0.stdout.take().ok_or(LocalError::Protocol)?;
            dev_worker::nonblocking(input.as_raw_fd()).map_err(|_| LocalError::Protocol)?;
            dev_worker::nonblocking(output.as_raw_fd()).map_err(|_| LocalError::Protocol)?;
            let mut output = BufReader::new(output);
            protocol::send(
                &mut input,
                &Frame::Initialize {
                    request_id: job.request_id.clone(),
                    version: 1,
                    repositories: job.repositories.clone(),
                },
                prep,
                &cancel,
            )?;
            let mut sequence = 1;
            let mut checkouts = None;
            let mut session = None;
            let mut broker: Option<Broker> = None;
            let mut inference_deadline = None;
            let receipt = loop {
                let until = inference_deadline.unwrap_or(prep).min(work);
                let frame = protocol::receive(&mut output, until, &cancel)?;
                if frame.request_id() != job.request_id {
                    return Err(LocalError::Protocol);
                }
                match frame {
                    Frame::RepositoryRequest {
                        sequence: current,
                        repository,
                        advertisement,
                        body,
                        ..
                    } => {
                        if checkouts.is_some() || session.is_some() {
                            return Err(LocalError::RepositoryRejected);
                        }
                        next_sequence(current, &mut sequence)?;
                        // Correlated output from this exact attached container proves start occurred.
                        guard.ambiguous = false;
                        let (content_type, bytes) =
                            fetch.request(repository, advertisement, &body, prep, cancelled)?;
                        respond(
                            &mut input,
                            &job.request_id,
                            current,
                            &content_type,
                            &bytes,
                            prep,
                            &cancel,
                        )?;
                    }
                    Frame::Repositories { repositories, .. } => {
                        if checkouts.is_some() || session.is_some() {
                            return Err(LocalError::Protocol);
                        }
                        guard.ambiguous = false;
                        fetch.verify_checkouts(&repositories)?;
                        checkouts = Some(fetch.pinned_checkouts()?);
                        // Task/global/project text has not entered the container during fetch.
                        respond(
                            &mut input,
                            &job.request_id,
                            0,
                            "application/json",
                            b"true",
                            prep,
                            &cancel,
                        )?;
                    }
                    Frame::Ready {
                        session_id,
                        policy,
                        guard_sha256,
                        ..
                    } => {
                        if session.is_some()
                            || policy != protocol::POLICY
                            || guard_sha256 != crate::dev_worker_local::guard_sha256()
                            || !token(&session_id, "ses")
                        {
                            return Err(LocalError::Protocol);
                        }
                        let repositories = checkouts.as_ref().ok_or(LocalError::Protocol)?;
                        event(LocalEvent::Ready {
                            session_id: session_id.clone(),
                            repositories: repositories.clone(),
                        })
                        .map_err(|_| LocalError::Publication)?;
                        let prompt = protocol::prompt_for(job, repositories)?;
                        broker = Some(Broker::new(prompt.clone(), repositories.len())?);
                        event(LocalEvent::Dispatching).map_err(|_| LocalError::Publication)?;
                        if cancel() || Instant::now() >= work {
                            return Err(LocalError::Interrupted);
                        }
                        let deadline = (Instant::now()
                            + Duration::from_secs(protocol::INFERENCE_SECONDS))
                        .min(work);
                        inference_deadline = Some(deadline);
                        may_have_sent = true;
                        protocol::send(
                            &mut input,
                            &Frame::Prompt {
                                request_id: job.request_id.clone(),
                                session_id: session_id.clone(),
                                text: prompt,
                            },
                            deadline,
                            &cancel,
                        )?;
                        session = Some(session_id);
                    }
                    Frame::ModelRequest {
                        sequence: current,
                        body,
                        ..
                    } => {
                        next_sequence(current, &mut sequence)?;
                        let deadline = inference_deadline.ok_or(LocalError::Protocol)?;
                        let broker = broker.as_mut().ok_or(LocalError::Protocol)?;
                        let request = crate::inspection_diagnostics::local(
                            crate::inspection_diagnostics::Phase::BrokerRequest,
                            || broker.request(&body),
                        )?;
                        let response = completion(&request, deadline).map_err(LocalError::from)?;
                        if cancel() || Instant::now() >= deadline {
                            return Err(LocalError::Deadline);
                        }
                        let response = crate::inspection_diagnostics::local(
                            crate::inspection_diagnostics::Phase::BrokerResponse,
                            || broker.response(&response),
                        )?;
                        let bytes =
                            serde_json::to_vec(&response).map_err(|_| LocalError::ModelProtocol)?;
                        respond(
                            &mut input,
                            &job.request_id,
                            current,
                            "application/json",
                            &bytes,
                            deadline,
                            &cancel,
                        )?;
                    }
                    Frame::ReadRequest {
                        sequence: current,
                        call_id,
                        arguments,
                        ..
                    } => {
                        next_sequence(current, &mut sequence)?;
                        let deadline = inference_deadline.ok_or(LocalError::Protocol)?;
                        broker
                            .as_mut()
                            .ok_or(LocalError::Protocol)?
                            .authorize_read(&call_id, &arguments)?;
                        respond(
                            &mut input,
                            &job.request_id,
                            current,
                            "application/json",
                            b"true",
                            deadline,
                            &cancel,
                        )?;
                    }
                    Frame::ReadResult {
                        sequence: current,
                        call_id,
                        text,
                        ..
                    } => {
                        next_sequence(current, &mut sequence)?;
                        let deadline = inference_deadline.ok_or(LocalError::Protocol)?;
                        broker
                            .as_mut()
                            .ok_or(LocalError::Protocol)?
                            .record_read(&call_id, text)?;
                        // Record evidence BEFORE acknowledging/releasing the result to OpenCode.
                        respond(
                            &mut input,
                            &job.request_id,
                            current,
                            "application/json",
                            b"true",
                            deadline,
                            &cancel,
                        )?;
                    }
                    Frame::Reply {
                        session_id,
                        message_id,
                        text,
                        ..
                    } => {
                        if session.as_deref() != Some(&session_id)
                            || !token(&message_id, "msg")
                            || message_id == format!("msg_{}", job.request_id)
                            || text.len() > protocol::MAX_REPLY
                        {
                            return Err(LocalError::Protocol);
                        }
                        broker
                            .as_ref()
                            .ok_or(LocalError::Protocol)?
                            .verify_reply(&text)?;
                        event(LocalEvent::Reply {
                            session_id: session_id.clone(),
                            message_id: message_id.clone(),
                            text: text.clone(),
                        })
                        .map_err(|_| LocalError::Publication)?;
                        break LocalReceipt {
                            session_id,
                            message_id,
                            reply: text,
                            repositories: checkouts.take().ok_or(LocalError::Protocol)?,
                        };
                    }
                    Frame::Failed { kind, .. } if kind == "local_worker_failed" => {
                        guard.ambiguous = false;
                        return Err(if checkouts.is_none() {
                            LocalError::RepositoryUnavailable
                        } else {
                            LocalError::ModelProtocol
                        });
                    }
                    _ => return Err(LocalError::Protocol),
                }
            };
            drop(input);
            expect_eof(&mut output, work, &cancel)?;
            loop {
                if cancel() || Instant::now() >= work {
                    return Err(LocalError::Shutdown);
                }
                if let Some(status) = child.0.try_wait().map_err(|_| LocalError::Shutdown)? {
                    if !status.success() {
                        return Err(LocalError::Shutdown);
                    }
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(receipt)
        })();
        let cleanup = if guard.armed {
            guard.stop(total - Duration::from_secs(2))
        } else {
            Ok(())
        };
        match (result, cleanup) {
            (Ok(receipt), Ok(())) => Ok(receipt),
            (result, cleanup) => Err(LocalFailure {
                error: result.err().unwrap_or(LocalError::Shutdown),
                may_have_sent,
                cleanup_unknown: cleanup.is_err(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixStream;

    #[test]
    fn current_local_runtime_selects_rootful_adapter_without_rootless_fallback() {
        let runtime = DockerLocalRuntime::new(PathBuf::from("/private/runtime"));
        assert_eq!(runtime.docker.mode, DockerMode::Rootful);
        assert_eq!(
            runtime.docker.runtime_root,
            PathBuf::from("/private/runtime")
        );
        let mut command = Command::new("/usr/bin/docker");
        runtime.docker.configure_command(&mut command).unwrap();
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            [
                "--config",
                "/nonexistent/smith-docker-config",
                "--host",
                "unix:///run/docker.sock",
            ]
        );
    }

    #[test]
    fn post_reply_eof_check_rejects_buffered_trailing_data_without_waiting() {
        use std::io::BufRead;
        let (mut writer, socket) = UnixStream::pair().unwrap();
        socket.set_nonblocking(true).unwrap();
        writer.write_all(b"\n").unwrap();
        let mut output = BufReader::new(socket);
        assert_eq!(output.fill_buf().unwrap(), b"\n");
        let until = Instant::now() + Duration::from_secs(1);
        assert_eq!(
            expect_eof(&mut output, until, &|| true),
            Err(LocalError::Shutdown)
        );
        assert_eq!(
            expect_eof(&mut output, Instant::now(), &|| false),
            Err(LocalError::Shutdown)
        );
        assert_eq!(
            expect_eof(&mut output, until, &|| false),
            Err(LocalError::Protocol)
        );
        assert_eq!(
            expect_eof(
                &mut output,
                Instant::now() + Duration::from_millis(15),
                &|| false
            ),
            Err(LocalError::Shutdown)
        );
        drop(writer);
        assert_eq!(expect_eof(&mut output, until, &|| false), Ok(()));
    }

    #[test]
    fn local_responses_are_chunked_correlated_and_bounded() {
        let (mut sender, receiver) = UnixStream::pair().unwrap();
        let mut receiver = BufReader::new(receiver);
        let id = "a".repeat(32);
        let bytes = vec![255u8; protocol::CHUNK + 7];
        let copy = bytes.clone();
        let job = id.clone();
        let thread = std::thread::spawn(move || {
            respond(
                &mut sender,
                &job,
                3,
                "application/json",
                &copy,
                Instant::now() + Duration::from_secs(2),
                &|| false,
            )
            .unwrap()
        });
        let until = Instant::now() + Duration::from_secs(2);
        assert!(matches!(
            protocol::receive(&mut receiver, until, &|| false).unwrap(),
            Frame::ResponseStart {
                sequence: 3,
                success: true,
                ..
            }
        ));
        let mut collected = Vec::new();
        for last_expected in [false, true] {
            match protocol::receive(&mut receiver, until, &|| false).unwrap() {
                Frame::ResponseChunk {
                    request_id,
                    sequence,
                    bytes,
                    last,
                } => {
                    assert_eq!(request_id, id);
                    assert_eq!(sequence, 3);
                    assert_eq!(last, last_expected);
                    assert!(bytes.len() <= protocol::CHUNK);
                    collected.extend(bytes);
                }
                _ => panic!("chunk"),
            }
        }
        thread.join().unwrap();
        assert_eq!(collected, bytes);
    }
    #[test]
    fn local_rpc_sequences_are_monotonic_and_single_use() {
        let mut next = 1;
        next_sequence(1, &mut next).unwrap();
        assert!(next_sequence(1, &mut next).is_err());
        assert!(next_sequence(3, &mut next).is_err());
        next_sequence(2, &mut next).unwrap();
        assert_eq!(next, 3);
        assert!(!token("../../bad", "msg"));
    }
}
