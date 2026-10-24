"""Fixed, currently unreachable inference graph. Construction is not authorization.

No exported function evaluates these nodes. Egress/resource enforcement and real
SDK/OpenCode interoperability must be reviewed before any activation change.
"""

from dataclasses import dataclass

from .protocol import Invalid

OPENCODE_VERSION = "1.19.3"
OPENCODE_ARCHIVE_SHA256 = "531cff2441129c30f0edd5878af494108ad07f5feb46d7ee29a2a5b830a5765d"
FLAGS = (
    "OPENCODE_PURE", "OPENCODE_DISABLE_DEFAULT_PLUGINS",
    "OPENCODE_DISABLE_PROJECT_CONFIG", "OPENCODE_DISABLE_AUTOUPDATE",
    "OPENCODE_DISABLE_PRUNE", "OPENCODE_DISABLE_AUTOCOMPACT",
    "OPENCODE_DISABLE_FFF",
    "OPENCODE_EXPERIMENTAL_DISABLE_FILEWATCHER", "OPENCODE_DISABLE_LSP_DOWNLOAD",
    "OPENCODE_DISABLE_EXTERNAL_SKILLS", "OPENCODE_DISABLE_CLAUDE_CODE",
    "OPENCODE_DISABLE_EMBEDDED_WEB_UI",
)
ENVIRONMENT = (
    "PATH=/usr/local/bin:/usr/bin:/bin", "HOME=/private/home",
    "XDG_CONFIG_HOME=/private/config", "XDG_CACHE_HOME=/private/cache",
    "XDG_DATA_HOME=/private/data", "XDG_STATE_HOME=/private/state",
    "TMPDIR=/private/tmp", "NO_COLOR=1",
) + tuple(flag + "=true" for flag in FLAGS)
CODE_PROMPT = (
    "Read TASK.md and clamp.py. Change only clamp.py to satisfy the task. "
    "Do not run commands, install dependencies, use the network, or claim checks passed."
)
REVIEW_PROMPT = (
    "Review TASK.md, clamp.py and /evidence/check.json in this fresh session. "
    "Report unsupported claims or defects against the exact candidate. "
    "Do not edit files or run commands. Your output is untrusted review data, not approval."
)


def _opencode_prefix(*, review=False):
    # Config has no model/provider/auth/plugin/endpoint fields. No --session,
    # --continue, --model or environment inheritance. Flags need live verification.
    config = ('{"share":"disabled","permission":{"*":"deny",'
              '"read":"allow","edit":' + ('"deny"' if review else '"allow"') + '}}')
    return ["/usr/bin/env", "-i", *ENVIRONMENT,
            "OPENCODE_CONFIG_CONTENT=" + config,
            "/usr/local/bin/opencode"]


def opencode_command(prompt, *, review=False):
    return _opencode_prefix(review=review) + ["run", "--format", "json", prompt]


# Fixed Python probe: no model, shell, imports from candidate, or network calls.
# It reports observed identity/version and an actual private workspace round-trip.
SETUP_PROBE = '''import hashlib
import json
import os
import sys
from pathlib import Path

workspace = Path("/workspace")
probe = workspace / ".smith-readiness"
with probe.open("xb") as output:
    output.write(b"smith-setup-v1")
try:
    writable = probe.read_bytes() == b"smith-setup-v1"
finally:
    probe.unlink()
print(json.dumps({
    "uid": os.getuid(), "gid": os.getgid(),
    "python_version": list(sys.version_info[:3]),
    "workspace": os.getcwd(), "workspace_ready": writable,
    "baseline_sha256": hashlib.sha256((workspace / "clamp.py").read_bytes()).hexdigest(),
    "task_sha256": hashlib.sha256((workspace / "TASK.md").read_bytes()).hexdigest(),
}, sort_keys=True, separators=(",", ":")))
'''


@dataclass(frozen=True)
class SetupGraph:
    version: object
    probe: object
    checker: object


def build_setup_graph(dag, request, baseline, task, checker_source, any_exit):
    """No inference: fixed image and independently observed prerequisite checks."""
    if request.phase != "setup":
        raise Invalid("invalid_graph_phase")
    image = (dag.container().build(dag.current_module().source().directory("image"))
             .with_user("1000:1000"))

    def fresh(role):
        return (image.with_workdir("/workspace")
                .with_env_variable("SMITH_ATTEMPT_IDENTITY", request.identity)
                .with_env_variable("SMITH_FIXED_ROLE", role))

    version = (fresh("setup-version")
               .with_exec(_opencode_prefix(review=True) + ["--version"],
                          expect=any_exit, redirect_stdout="/private/version.txt"))
    probe = (fresh("setup-probe")
             .with_new_file("/workspace/clamp.py", contents=baseline.decode("utf-8"),
                            owner="1000:1000", permissions=0o600)
             .with_new_file("/workspace/TASK.md", contents=task.decode("utf-8"))
             .with_exec([
                 "/usr/bin/env", "-i", "PATH=/usr/bin:/bin",
                 "/usr/bin/python3", "-I", "-B", "-c", SETUP_PROBE,
             ], expect=any_exit, redirect_stdout="/private/probe.json"))
    checker = (fresh("setup-checker")
               .with_new_file("/trusted/check_clamp.py", contents=checker_source.decode("utf-8"))
               .with_new_file("/candidate/clamp.py", contents=baseline.decode("utf-8"))
               .with_exec([
                   "/usr/bin/env", "-i", "PATH=/usr/bin:/bin",
                   "PYTHONDONTWRITEBYTECODE=1", "/usr/bin/python3", "-I", "-B",
                   "/trusted/check_clamp.py", "/candidate/clamp.py",
               ], expect=any_exit, redirect_stdout="/private/baseline.json"))
    return SetupGraph(version, probe, checker)


@dataclass(frozen=True)
class FixtureGraph:
    coding: object
    checker: object
    review: object


def build_graph(dag, request, baseline, task, checker_source, any_exit):
    """Construct only from fixed trusted source and explicit immutable snapshot.

    There is intentionally no policy-enabled argument and no execution helper.
    This is a source contract for fake tests, not an enforced network sandbox.
    """
    if request.phase != "execute":
        raise Invalid("invalid_graph_phase")
    context = dag.current_module().source().directory("image")
    image = dag.container().build(context).with_user("1000:1000")

    def fresh(role):
        return (image.with_workdir("/workspace")
                .with_env_variable("SMITH_ATTEMPT_IDENTITY", request.identity)
                .with_env_variable("SMITH_FIXED_ROLE", role))

    coding = (fresh("coding")
              .with_new_file("/workspace/clamp.py", contents=baseline.decode("utf-8"),
                             owner="1000:1000", permissions=0o600)
              .with_new_file("/workspace/TASK.md", contents=task)
              .with_exec(opencode_command(CODE_PROMPT)))
    # A new sibling container, never derived from the model-modified filesystem.
    checker = (fresh("checker")
               .with_new_file("/trusted/check_clamp.py", contents=checker_source.decode("utf-8"))
               .with_file("/candidate/clamp.py", coding.file("/workspace/clamp.py"),
                          owner="1000:1000")
               .with_exec([
                   "/usr/bin/env", "-i", "PATH=/usr/bin:/bin",
                   "PYTHONDONTWRITEBYTECODE=1", "/usr/bin/python3", "-I", "-B",
                   "/trusted/check_clamp.py", "/candidate/clamp.py",
               ], expect=any_exit, redirect_stdout="/private/check.json"))
    review = (fresh("review")
              .with_new_file("/workspace/TASK.md", contents=task)
              .with_file("/workspace/clamp.py", coding.file("/workspace/clamp.py"),
                         owner="1000:1000")
              .with_file("/evidence/check.json", checker.file("/private/check.json"),
                         owner="1000:1000")
              .with_exec(opencode_command(REVIEW_PROMPT, review=True)))
    return FixtureGraph(coding, checker, review)
