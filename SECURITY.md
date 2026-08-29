# Security Policy

## Scope

Smith is an Ubuntu/Linux workstation orchestration system under active hardening. Security-sensitive areas include agent capabilities, approval handling, Docker networking and mounts, filesystem roots, SSH and credential handling, Git/GitHub integration, local service exposure, persistence, and audit evidence.

## Reporting a vulnerability

Do **not** open a public issue containing an exploit, credential, private path, or reproduction that could expose a host.

Report privately to the repository owner through GitHub's private reporting channel when enabled, or use the contact method published by the repository owner. Include:

- affected revision and component;
- minimal reproduction and impact;
- prerequisites and trust boundary crossed;
- suggested mitigation, if known; and
- whether credentials, private data, or a network-exposed service may be involved.

Do not include secrets in reports. Redact tokens, keys, authorization headers, sensitive paths, and personal data.

## Response policy

- Reports are triaged against the current supported Ubuntu workstation target.
- A finding that expands privilege, host access, network exposure, credential access, or release authority is treated as high severity until assessed.
- Fixes follow the process in `docs/PROCESS.md`: evidence, independent review, and explicit approval for external effects.
- No release or accepted-risk decision is made by the reporting, implementation, or reviewing agent alone.

## Supported state

The active development branch and latest approved release are assessed case by case. The project does not claim a security guarantee for unreviewed or legacy behavior.
