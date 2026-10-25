# Source-only artifact pipeline

**Implemented source:** the dependency-free clamp fixture/checker and the fixed,
unwired Python Dagger module for the [approved milestones](../docs/plans/LOCAL-SMITH-ARTIFACT-PIPELINE-MVP.md).
Execute is unconditionally **policy denied**. This is not a working model pipeline
or functional/installed acceptance. No Dagger engine, image preparation, OpenCode,
provider, credential or service/API/UI execution is authorized by these sources.

## Candidate and trusted inputs

- `fixtures/clamp/TASK.md`: baked task and allowed pure-function syntax.
- `fixtures/clamp/clamp.py`: deliberately wrong baseline, the only candidate
  source input. Below/above-bound returns are reversed; reversed bounds are not
  rejected. Do not replace this baseline with the solution.
- `tests/check_clamp.py`: trusted checker, **outside the candidate directory**.
  Never load a candidate's checker, tests or reports. Supply only the candidate's
  `clamp.py` bytes/path to this trusted implementation.
- `tests/test_clamp.py`: trusted unittest coverage, including a fixed candidate
  held only in the test harness.

`clamp(value, low, high)` returns the closest inclusive bound, or the original
value inside the interval. Equal bounds return that bound; reversed bounds
always raise `ValueError`. The fixture domain is finite integer/float inputs;
non-numeric values, NaN and infinity input semantics are not part of this task.
Fourteen fixed named cases cover lower/upper/interior, negative/fractional,
equal-bound and reversed-bound behavior. Case results come from actual AST
interpretation, not model assertions or candidate-provided test output.

## Trusted checker API

```python
from check_clamp import check_source

result = check_source(candidate_bytes)
```

`check_source(bytes)` is pure: no filesystem access, imports of candidate code,
`exec`, `eval`, subprocesses or compilation. The structured result contains:

- `check_id`: versioned trusted contract identity `smith.clamp.restricted-ast.v1`;
- `source_sha256`: SHA-256 of the exact bounded input bytes, including whitespace;
- `passed`, fixed `error` code (or null), and `cases` with names, actual outcomes,
  expected values and pass/fail. An expected null means `ValueError`.

Rejected syntax has no case results. Oversized/non-bytes inputs and denied paths
have no digest. Bounded invalid UTF-8/syntax inputs retain their exact byte digest.
The check ID is a contract version, **not** an attestation or checker-binary hash.
Callers must invoke the trusted API on independently obtained candidate bytes
and bind its result/digest to their attempt/artifact identity; a supplied JSON
report or digest is not trusted evidence. This fixture does not implement that
Rust orchestration or durable publication.

The command wrapper takes exactly one candidate path and prints one JSON result:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -B pipeline/tests/check_clamp.py /private/candidate/clamp.py
```

Exit 0 means every actual case passed; exit 1 means rejected input or failed
cases. This invocation illustrates the interface; run candidates only in the
approved isolated source-test environment. The deliberately wrong baseline is
expected to exit 1.

## Safety limits (Ubuntu/Linux)

This is **not a general sandbox or general Python coding checker**. It interprets
only the narrow pure-function grammar in `TASK.md`, not arbitrary Python:

- Source at most **4096 bytes**, strict UTF-8; AST at most **128 nodes**, depth
  **16** (including structural/operator nodes).
- Numeric literals must be finite, absolute value at most **1,000,000**; booleans
  are not numeric literals/results. Only unary signs, no binary arithmetic,
  exponentiation or value-growing operations. Source size also bounds literal
  spelling before parsing.
- Exactly one undecorated `clamp(value, low, high)` definition, no annotations,
  defaults, extra parameters or executable top-level statements. Only bounded
  branches, returns and `raise ValueError()`; no other calls, imports, attributes,
  loops, assignments, recursion or access to tests/files/processes. All branches
  are validated, including unreachable ones, before any case interpretation.
- No user-defined objects are passed to the interpreter. Fixed cases and bounded
  non-recursive-in-the-language expressions/statements bound evaluation work;
  the implementation traverses at most the admitted AST per case.
- Path wrapper accepts only a final `clamp.py`, rejects explicit `.`/`..`, and
  walks ancestor directory descriptors with Linux `O_NOFOLLOW`/`O_DIRECTORY`.
  Final open uses `O_NOFOLLOW`/`O_NONBLOCK`; `fstat` requires a regular file,
  rejects oversized metadata, and reads at most 4097 bytes before rejection.
  Symlinks, directories, FIFOs and other non-regular files cannot be evaluated.
  Metadata changes during reading are rejected; evidence hashes the bytes read.

No workspace-root authorization, hostile same-user filesystem isolation,
concurrent-writer snapshot guarantee, hard-link prohibition, or general resource
sandbox is claimed. The trusted caller owns the private candidate root and must
freeze the accepted bytes; descriptor traversal prevents symlink following but
does not confer broader filesystem authority. Special files are never read after
regular-file validation. No candidate-controlled strings/errors are emitted.

## Isolated validation

From the repository root, run only:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -B pipeline/tests/run_isolated.py
```

The standard-library runner copies the two trusted modules and baseline into a
private temporary directory, changes into it, then discovers/runs the tests
in-process. Path tests create disposable children under the temporary test
root. No candidate is executed as a subprocess. No installs, Docker, Dagger,
OpenCode, models or network are used. Bytecode writing is disabled. The runner
removes the temporary tree and prints bounded, secret-safe results without
writing logs or snapshots into the repository.

These are developer self-checks only. Fresh independent assurance and the other
parent-owned milestone checks remain separate; no full pipeline, provider,
installed or live-runtime success follows from a fixture pass.

## Second-milestone module contract

`SmithAgent.run_fixture(request: dagger.File) -> str` is an async `@function`;
its intended CLI name is `run-fixture`. It returns a **string containing one JSON
envelope**, not a Dagger object or an artifact directory. The exact CLI File
argument codec, scalar stdout framing, Python SDK discovery and SDK
interoperability have **not** been live-verified. Trusted assets are read through
`dag.current_module().source().file(...)`, not inferred from an installed package's
`__file__`. Do not assume a tested CLI invocation from fake-SDK method calls.

The module selects engine `v0.21.9` and its engine-managed Python SDK. Its
`dagger-io` dependency is unversioned because Dagger installs the supplied SDK
editable alongside the module; the SDK's Python distribution version need not
match the engine release. This does not guarantee offline or hermetic dependency
resolution. The source-only Ubuntu image recipe pins OpenCode `1.19.3` and archive SHA-256
`531cff2441129c30f0edd5878af494108ad07f5feb46d7ee29a2a5b830a5765d`,
matching `service/dev-worker/Dockerfile.local`. Exported Setup now contains the
source graph that builds this fixed recipe and evaluates no-model prerequisite
checks. **It has not been executed.** Evaluating it would require separate live
approval for image preparation/downloads and container effects. The base tag and
apt dependencies are not digest-locked; reproducible/hermetic preparation is not
claimed.

### Wire schema

Request is strict UTF-8 JSON, at most 32768 bytes, with exactly:

- `schema_version`: integer `1` (booleans/floats rejected).
- `run_id`: 32 lowercase hex characters.
- `phase`: `setup`, `execute` or `submit`; `attempt`: integer `1` or `2`.
- `contract_sha256`, `executable_sha256`, `module_sha256`: 64 lowercase hex.
- `input`: a string containing the phase JSON below, at most 16384 UTF-8 bytes,
  without NUL or invalid Unicode scalars.

Duplicate keys at every object level, unknown/missing fields, nonfinite JSON
constants and malformed values fail closed. Request identity hashes the **original
file bytes**, including whitespace/order, never a reserialized object. `File.size`
is checked before `File.contents`; oversize, unreadable or non-round-trippable
File inputs fail transport rather than fabricating an original-byte hash.

Response has `schema_version: 1`, `request_sha256`, and `outcome`:

- `produced`: nonempty `body` string at most 16384 UTF-8 bytes; no `error`.
- `failed`: `error`; no `body`. Allowlist: `authentication`,
  `default_unavailable`, `quota`, `provider`, `policy_denied`, `prerequisite`.

Only `policy_denied` and `prerequisite` are emitted currently. A failed envelope is
a valid return/CLI-exit0 transport; nonzero CLI exit is transport failure, not a
provider/authentication diagnosis. Host identity verification, durable attempt
consumption, timeout, acceptance and independently verified cleanup remain
parent-owned. Echoed hashes do not attest to execution or host verification.

### Explicit immutable phase inputs

All phase payloads require integer `schema_version: 1` and `baseline`. A snapshot
is exactly `{path: "clamp.py", content: <UTF-8 string>, sha256: <exact bytes hash>}`.
Sources are at most 4096 bytes. The baseline must match the baked deliberately
incorrect fixture **byte for byte**. No phase assumes a surviving workspace or a
prior Dagger session, and no host path/command/image/credential/model override is
accepted in a payload.

- **Setup:** exactly the common fields. The exported SDK function reads fixed
  baseline/checker/task Files from module source, builds the fixed image and
  evaluates three identified non-root container nodes: clean `opencode --version`,
  a fixed Python/workspace probe, and the trusted checker on the baked baseline.
  It requires observed version `1.19.3` with exit0; Python `3.12` with its observed
  patch version, UID/GID1000, cwd `/workspace`, an actual exclusive-write/read/delete
  round-trip, and exact observed baseline/task byte hashes. The checker must exit1
  and its complete report must equal the independently recomputed expected
  baseline failures. Missing, oversized, malformed, failed or mismatched
  observations return `prerequisite`, never an environment PASS from constants.
  Only validated observations produce `status: "runtime_ready"`,
  `runtime_verified: true`, request `identity`, observed `opencode_version`,
  `probe`, `checker_sha256`, `baseline_check`,
  `expected_engine_version: "v0.21.9"`, `inference: "policy_denied"` and
  `semantic_review: "not_implemented"` (plus `schema_version: 1`). This is only
  no-model Setup readiness, **not** Execute PASS, engine identity attestation,
  inference/egress readiness or overall runtime acceptance.
  The separate pure `run_source_request(raw, baseline, checker_source)` helper
  takes explicit trusted bytes and performs no SDK/filesystem/container effects.
  Its Setup body remains `source_ready`, `runtime_verified: false`, with expected
  engine/OpenCode versions and actual pure baseline checker results. Exported
  Setup cannot use this weaker result in place of command observations.
- **Execute:** exactly the common fields. Valid requests return `policy_denied`
  before any inference graph is constructed/evaluated. A request boolean cannot
  enable provider networking.
- **Submit:** common fields plus `candidate` snapshot, `patch`, `patch_sha256`,
  `checker_sha256`, `check`, `response`, `response_sha256`, `execute_identity`.
  `execute_identity` contains exactly `run_id`, `phase: "execute"`, `attempt`,
  `request_sha256`, `contract_sha256`, `executable_sha256`, `module_sha256`;
  run/contract/executable/module must match Submit, and attempt is `1` or `2`.
  The host must authenticate that prior identity against retained Execute
  evidence; the module cannot establish that Execute happened. Submit recomputes
  the checker, requires actual case success, checks every binding, then returns
  `prerequisite` because fresh semantic review is not implemented. Missing or
  supplied `review`/model PASS fields never manufacture review or acceptance.

`patch` is a deliberately narrow full-file replacement, exactly
`{format: "smith.replace.v1", path: "clamp.py", base_sha256, candidate_sha256,
replacement: <exact candidate string>}`. Reconstruction checks both byte hashes
and must equal the candidate snapshot. Patch hashing uses sorted-key, ASCII JSON
with separators `,` and `:` and no trailing newline (`protocol.encode`). Response
hashing uses exact UTF-8 response bytes, without normalization. `checker_sha256`
is the exact trusted `tests/check_clamp.py` source hash; `check` must exactly match
its recomputed result, including source digest and all cases. Candidate-supplied
reports are never evidence. Pure artifact helpers retain current request identity,
exact candidate/base/patch/checker hashes and, for Submit, the prior identity and
response hash. These are bindings, not attestations or semantic review.

### Fixed graph and activation boundary

`graph.build_graph` defines sibling coding, checker and fresh-session review
containers from the fixed `image/` recipe context; exported Setup instead uses
`graph.build_setup_graph` with no inference commands. The coding container receives
only the fixture source and baked task. Only the independently sourced trusted
checker receives `/trusted/check_clamp.py`; its candidate is the exact coding
`clamp.py` File. The review container receives task, candidate and checker output,
not coding-session state. All effectful nodes include run/phase/attempt/exact
request and contract/executable/module identity plus a distinct fixed role in
cache inputs. Fake graph assertions do **not** prove engine cache behavior.

OpenCode uses fixed `run --format json` commands through `env -i`, private
HOME/XDG/temp paths, no session reuse, clean default model/provider resolution,
no host auth, and explicit project-config/instruction/plugin/update/tool-download
suppression flags. Model discovery is not disabled: no
`OPENCODE_DISABLE_MODELS_FETCH` is set. The fixed permission config denies tools
by default, permits
read/edit for coding and read-only for review; shell/network tools are not
allowed. These source settings require pinned-binary verification and are **not**
an enforced egress, resource or filesystem security boundary.

No exported branch evaluates this inference graph. Enforceable reviewed egress,
finite resource/output/workspace limits, candidate containment/no-symlink export,
review parsing/acceptance, SDK/File interoperability, image preparation,
cache/cancellation/quiescence and real OpenCode default behavior remain live
acceptance blockers. Do not activate by adding a request-level permission boolean.
No fallback, fake generated candidate or model-success artifact is returned.

### Runtime source manifest

The exact runtime source allowlist, relative to `pipeline/`, is:

```text
dagger.json
pyproject.toml
image/Dockerfile
src/smith_agent/__init__.py
src/smith_agent/main.py
src/smith_agent/protocol.py
src/smith_agent/artifacts.py
src/smith_agent/graph.py
fixtures/clamp/TASK.md
fixtures/clamp/clamp.py
tests/check_clamp.py
```

The host must hash/verify the canonical manifest before dispatch. This list does
not include generated files, caches, host roots or user-selected modules. README
and test harnesses are non-runtime sources; packaging/discovery must not turn
extra module context into execution authority.

### Offline source checks

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -B pipeline/tests/run_module_isolated.py
```

This new runner copies an explicit source/test allowlist into a private temporary
tree and runs the unchanged fixture suite plus strict protocol, exact-byte
artifact, real-checker, fake-SDK graph and async File/export tests. Async tests
simulate missing/corrupt module assets, observed version/UID/GID/Python/workspace
and baseline failures, SDK failures and bounded output rejection, while requiring
Execute/Submit to construct no containers. Simulated Setup observations are not
live runtime evidence. The runner denies
socket connections and subprocess launches, installs nothing and never imports
the real Dagger SDK. It prints bounded, secret-safe results without generating
source inventories or persistent logs. This is self-check evidence only; fresh independent
assurance and parent-owned Rust/module-manifest checks remain separate.
