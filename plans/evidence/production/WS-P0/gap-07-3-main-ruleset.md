# GAP-07.3 — `main` ruleset

**Date:** 2026-08-31  
**Does not promote** any product-readiness ledger row.  
**Does not** claim independent human review (the repository currently has a single owner).

## Live ruleset

| Field | Value |
| --- | --- |
| Id | `21950476` |
| Name | `protect-main` |
| HTML | https://github.com/9thLevelSoftware/legion-ide/rules/21950476 |
| API | https://api.github.com/repos/9thLevelSoftware/legion-ide/rulesets/21950476 |
| Target | `refs/heads/main` |
| Enforcement | `active` |
| Bypass | none (`current_user_can_bypass=never`) |

Fetched with `gh api repos/9thLevelSoftware/legion-ide/rulesets/21950476` as `9thLevelSoftware`. Request body: `plans/evidence/production/WS-P0/gap-07-3-main-ruleset-request.json`.

## Rules

- **deletion** — `main` cannot be deleted
- **non_fast_forward** — no force-push
- **pull_request** — changes to `main` must arrive through a pull request. `required_approving_review_count=0` and `require_code_owner_review=false` because there is no second reviewer; turning those on would deadlock a solo owner. CODEOWNERS is still in `.github/CODEOWNERS` for when a second reviewer exists.
- **required_status_checks** (strict, branches must be up to date), contexts taken from hosted runs `32740385519` (gates) and `32740385497` (bench):
  - `Standing gates (ubuntu-latest)`
  - `Standing gates (windows-latest)`
  - `Standing gates (macos-latest)`
  - `cargo-deny (advisories, bans, licenses, sources)`
  - `Legion bench recorded (replayed execution)`

Smoke, preview, DAP dogfood, and live bench remain independent and are **not** required checks.

## Operator consequence

Direct `git push origin main` is rejected. Land further work through pull requests. Local `main` that is already ahead of `origin/main` must be published as a PR branch, not a fast-forward of the protected ref.

## Related

- QUAL.11 taxonomy: `plans/qual-11-release-blocker-taxonomy.md`
- Sequence: `plans/p0-installed-product-sequence-v0.1.md` GAP-07.3
