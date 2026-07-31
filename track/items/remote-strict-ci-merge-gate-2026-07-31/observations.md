## Branch-protection configuration handoff

- Repository: `Flip451/SoTOHE-core`; protected branch: `develop`.
- An authenticated repository administrator (`permissions.admin=true`) applied and re-read the protection configuration at `2026-07-31T13:57:41Z`.
- The branch was previously unprotected: `GET .../branches/develop/protection` returned `404 Branch not protected`.
- The current protection response has `required_status_checks.strict=true`, with API context `check` and `app_id=15368` (`GitHub Actions`); `enforce_admins.enabled=true`; force pushes and deletions are disabled.
- Settings reference: <https://github.com/Flip451/SoTOHE-core/settings/branches>.

## Branch-protection evidence

- GitHub's UI/check-rollup label is `CI / check`, while the REST check-run and branch-protection context is `check` under the GitHub Actions app.
- Failed pull-request check specimen: [PR](https://github.com/Flip451/SoTOHE-core/pull/214); [failed CI run](https://github.com/Flip451/SoTOHE-core/actions/runs/29972090241); [failed `check` job](https://github.com/Flip451/SoTOHE-core/actions/runs/29972090241/job/89096060229). Its conclusion is `failure` and its app id is `15368`.
- The failed run predates the protection change and is used only to prove the exact app/context identity; it was not blocked by a rule that did not yet exist. The current strict required-check response binds that same identity, so a failure of `CI / check` now cannot satisfy the protected-branch merge gate, including for administrators.
