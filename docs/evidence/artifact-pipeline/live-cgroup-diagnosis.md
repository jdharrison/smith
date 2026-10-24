# Post-window read-only cgroup diagnosis

## Observed

At 2026-10-21T01:35:07Z, the original safety-adjusted live deadline (01:32:45.863Z) had expired. No restart or CLI request was attempted during this investigation.

Read-only Docker export of the exact retained container located `usr/local/bin/dagger-entrypoint.sh`. Its cgroup-v2 initialization contains:

```sh
mkdir -p /sys/fs/cgroup/init
xargs -rn1 < /sys/fs/cgroup/cgroup.procs > /sys/fs/cgroup/init/cgroup.procs || :
sed -e 's/ / +/g' -e 's/^/+/' < /sys/fs/cgroup/cgroup.controllers \
    > /sys/fs/cgroup/cgroup.subtree_control
```

This establishes that the pinned entrypoint moves root processes into a child before enabling delegation. The source was read as data, not executed. The Docker export stream was terminated after locating the file; no image filesystem was extracted onto the host.

A separate exact-CID read-only inspection still reported `Status=exited`, `Running=false`, `Pid=0`, `ExitCode=137`, with the same recorded start/finish timestamps. This is stopped-container evidence, **not complete descendant quiescence proof**.

## Reproduced historical defects

`test_live_cgroup_diagnosis.py` uses only mocked filesystem observations against the unchanged historical verifier:

1. Finite limits on the container root are accepted when PID membership is that root.
2. Unlimited local limits on `init` are rejected even when the owned parent has the required aggregate limits.
3. Checking an empty `init` leaf can return true without inspecting an active sibling worker cgroup.

Command:

```text
python3 -B -m unittest discover -s docs/evidence/artifact-pipeline -p test_live_cgroup_diagnosis.py -v
```

Final result: 3 tests passed, recorded in `live-cgroup-diagnosis-offline-final.log`. These tests confirm defective historical behavior; they do not implement a corrected verifier. The first run had a test-harness exception-type mismatch (expected RuntimeError rather than the helper's Refuse); the final run corrected only that expectation.

## Inference and limits

The entrypoint behavior is consistent with the continuation failing at its leaf-only resource check. The failed attempt did not preserve the actual leaf control-file values or original exception, so the exact failed condition cannot be established retrospectively. Do not report this hypothesis as a captured runtime diagnosis.

Historical cleanup remains **Unknown**. No executed supervisor, original test, receipt, capture, consumed/pending marker or outcome was changed. No transport, Setup, inference or product acceptance follows.

## Architecture review and next source delta

Independent architecture analysis identified the same two defects and recommended a separate verifier, not edits to executed supervisors:

- Bind the full captured CID, current process/start identity and exact owned systemd scope. Reject ambiguous hierarchy/layout and unsafe paths.
- Verify aggregate CPU/memory/PID limits at that owned root. Child `max` values may inherit those restrictions; bounded shared ancestors outside ownership cannot substitute.
- Return and retain owned-root identity for whole-subtree cleanup, including sibling workers. Missing/replaced roots and incomplete observations remain Unknown.
- Use bounded no-follow descriptor-relative observations and typed diagnostics; recheck identity/membership and refuse races.
- Preserve the existing resource limits, uncertainty fence and all historical evidence.

This verifier observes containment; it cannot alone guarantee that a privileged engine cannot modify its hierarchy or escape resource controls. This limitation is unresolved, not an accepted-risk decision.

A new operational supervisor needs independent review and a renewed exact human-approved window before any engine restart. Reusing the stopped engine would require explicit handling of the previous uncertainty state; it must never clear old markers or present a restart as an unconsumed original attempt. No provider, image build, Setup, credential, repository, installed-service or publication effects are included in the proposed transport follow-up.
