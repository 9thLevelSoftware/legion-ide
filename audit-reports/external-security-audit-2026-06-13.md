# Audit Report: External Security Audit + Pen Test

> Internal security audit and pen-test summary for WS20.T4. This report records the current security gate results for the Legion IDE repository and triages any findings into blockers vs advisories.

## Audit Metadata
- Date: 2026-06-13
- Workspace: `/Users/christopherwilloughby/legion-ide`
- Scope: repository-wide security audit with targeted validation of security-sensitive crates and eval harnesses
- Reviewed surfaces:
  - `crates/legion-security`
  - `crates/legion-plugin`
  - `crates/legion-ai-providers`
  - `evals/run_eval.py`
  - repo-wide secret/vuln/misconfig scans

## Verdict
- Blockers: 0
- Advisories: 1
- Secret leaks: 0
- Dependency vulns found by Trivy: 0
- Files changed by audit: 1 new report file only

Overall result: no blocking security issues were confirmed in the repository-wide scans and targeted test gates. One hardening advisory was found in the evaluation harness and is documented below.

## Verification Run

### Targeted security crate tests
- `cargo test -p legion-security --all-targets`
  - Result: 54 passed, 0 failed, 1 cross-platform test passed
- `cargo test -p legion-plugin --all-targets`
  - Result: 7 passed, 0 failed
- `cargo test -p legion-ai-providers --all-targets`
  - Result: 20 passed, 0 failed, 1 ignored
  - Additional conformance tests: 4 passed
  - Prompt stability tests: 3 passed

### Repo-wide scanners
- `semgrep scan --config auto --json --output /tmp/legion-audit/semgrep.json .`
  - Result: 1 finding
- `gitleaks git --redact --report-format json --report-path /tmp/legion-audit/gitleaks.json .`
  - Result: no leaks found
- `trivy fs --scanners vuln,secret,misconfig --format json --output /tmp/legion-audit/trivy.json .`
  - Result: 0 findings across Cargo.lock targets scanned

### Tool availability
- `cargo-deny` was not installed locally, so the deny gate could not be run in this environment.

## Triaged Findings

### Advisory 1: Endpoint mode accepts an arbitrary URL without scheme/host validation
- Location: `evals/run_eval.py:85-117`
- Why it was flagged: Semgrep reported dynamic URL use in the live endpoint call path.
- Root cause: `_call_endpoint()` appends `/v1/chat/completions` and sends a POST via `urllib.request.urlopen()` to whatever `--endpoint` value is supplied, without validating that the endpoint is `http://` or `https://` or restricting it to a trusted allowlist.
- Security impact: this is not a remotely reachable product attack surface by itself because the harness is a CLI tool and the endpoint is operator-supplied, but it is SSRF-prone if the harness is wrapped by untrusted automation or if the endpoint value comes from a less-trusted source.
- Triage decision: advisory, not blocker. The documented usage already treats endpoint mode as an explicit, consented operator action.
- Recommended follow-up: add endpoint scheme validation and, if the harness will ever accept external configuration, consider a trusted-host allowlist plus a regression test for invalid schemes.

## Summary for Public/Shared Reporting
- No secrets were found.
- No dependency vulnerabilities were found in the scanned lockfiles.
- Security-sensitive crate tests passed.
- One moderate hardening advisory remains in the evaluation harness; it is documented and triaged, but it does not block the current GA trust posture claim.

## Commands Executed
```bash
cargo test -p legion-security --all-targets
cargo test -p legion-plugin --all-targets
cargo test -p legion-ai-providers --all-targets
semgrep scan --config auto --json --output /tmp/legion-audit/semgrep.json .
gitleaks git --redact --report-format json --report-path /tmp/legion-audit/gitleaks.json .
trivy fs --scanners vuln,secret,misconfig --format json --output /tmp/legion-audit/trivy.json .
```

## Notes
- The audit intentionally avoided modifying user-owned in-flight files outside the report artifact.
- This report is the durable summary artifact for WS20.T4.
