# Issue #13 residual gaps (preflight / rollback / idempotency)

## Covered by automated tests (this branch)

- Restore prior content after tracked write (`write_and_rollback_restores_prior_content`)
- Idempotent skip when content is unchanged
- Failure injection then retry converges
- Preflight soft notes (ports, repos, outbound HTTPS / guest)
- Rollback scoped to current `run_id` (older runs survive)
- `WroteRepo` entries removable on rollback
- Honest failure messages (no false "de forma segura")

## Covered in installer runtime (merged earlier)

- Per-run journal `begin_install_run` / `end_install_run`
- Soft preflight before mutating stages (disk, root, OS, ports, HTTPS)
- Failure path calls `rollback_tracked_files` and surfaces `FailureKind`

## Residual gaps (keep issue open if any block acceptance)

1. **Full stage-injection suite against live dnf/apt** is still lab/operator territory. Unit tests mock tracked files, not a mid-`dnf` kill with package DB half-updated.
2. **Preexisting files CPN did not create** are not blindly reverted; only journaled paths for the active `run_id` are rolled back (by design).
3. **Network soft checks** warn only; they do not hard-fail installs on air-gapped hosts (operators may need offline mirrors).
4. **UI copy** after partial failure depends on `failure_message(FailureKind)`; if a stage fails outside the journal helpers, operators may still need log review.

Close #13 only after CI green for the expanded unit tests above and an operator note that live package-manager mid-transaction recovery remains best-effort.
