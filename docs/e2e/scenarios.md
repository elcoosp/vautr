# Cross-Client E2E Scenarios

Canonical end-to-end flows that every Vautr client must implement identically so
the UX stays in parity (ui-ux Phase 9). Each scenario lists the steps every client
shares, then a per-client coverage matrix that records **what the existing harness
actually asserts today** — gaps are named explicitly rather than implied.

Server under test: a running Vautr server (default `http://127.0.0.1:8080` /
`http://localhost:8080`). All flows are zero-knowledge: the server never sees a
plaintext secret.

---

## Scenario 1 — Register → Login → Unlock

Steps (canonical):
1. Register a new user (OPAQUE register-start / register-finish) with a master password.
2. Receive and retain the recovery key (mnemonic) — shown once.
3. Login (OPAQUE login-start / login-finish) → session token + wrapped SVK + kdf_salt.
4. Derive MK → KEK → SVK → DEK locally; unlock the client.

| Client | Harness | Asserts |
| --- | --- | --- |
| CLI | `apps/cli/tests/live_e2e.rs` (`live_server_cli_e2e`) | register + login succeed; output contains `registered` / `logged in as`. |
| Desktop | `apps/desktop/tests/live_server_e2e.rs` (`full_auth_add_reveal_roundtrip`) | register returns 32-byte kdf_salt + non-empty mnemonic; login returns token + wrapped SVK + `min_enc_key_gen >= 1`; `unlock_with_password` succeeds. |
| Extension | `apps/extension/e2e/live-e2e.mjs` | full OPAQUE register/login via node wasm; SVK recovered from `/account/status`. |
| Web | — | **GAP:** no live_e2e harness in `apps/web/e2e` (only `webauthn.spec.ts` via Playwright). |

---

## Scenario 2 — Create project → create secret → reveal → decrypt

Steps (canonical):
1. Create a project (shared/personal).
2. Create a secret (client-side encrypted; server stores only ciphertext + key).
3. List projects / list secrets → item appears.
4. Reveal the secret value; decrypt locally; assert plaintext round-trips.

| Client | Harness | Asserts |
| --- | --- | --- |
| CLI | `live_e2e.rs` | create project (`created project`); create secret by flag + stdin; list shows both; `get` returns exact plaintext; `run --` injects secrets as env vars. |
| Desktop | `live_server_e2e.rs` (`projects_and_secrets_roundtrip`) | create project (name + kind); create secret (AEAD encrypt client-side); list shows it; `get_secret_value` + local decrypt asserts plaintext matches. |
| Extension | `live-e2e.mjs` | push item via `/sync/push-batch`; **stateless decrypt** via SW wasm (`decrypt_secret_with_svk`) asserts password matches; also full `decrypt_item_js`. |
| Web | `apps/web/e2e/mlp-web.spec.ts` | covers MLP auth path (see Scenario 4); **GAP:** no dedicated project/secret reveal round-trip like the others. |

---

## Scenario 3 — Edit / rotate a secret value

Steps (canonical):
1. Edit a secret's value (canonical verb: **rotate** for value updates).
2. Reveal again; assert the new value is returned (old value gone).

| Client | Harness | Asserts |
| --- | --- | --- |
| CLI | `live_e2e.rs` | `edit secret` → `get` returns rotated value (`rotated-xyz`). |
| Desktop | — | **GAP:** `live_server_e2e.rs` does not test edit/rotate of a project secret. |
| Extension | — | **GAP:** `live-e2e.mjs` pushes once, never edits. |
| Web | — | **GAP:** not covered. |

> Parity action: add an edit/rotate step to the desktop + extension + web harnesses
> so all four assert the post-rotate reveal matches.

---

## Scenario 4 — Scope denial (machine account cannot reveal)

Steps (canonical):
1. Create a project + secret (user token can reveal).
2. Provision a machine account scoped `secrets:read` (read metadata, **not** `secrets:reveal`).
3. Issue an access token for that machine account.
4. Attempt to reveal the secret value with the machine token → must be **denied** (401/403).
5. Reading secret *metadata* with the machine token → allowed.

| Client | Harness | Asserts |
| --- | --- | --- |
| Extension | `apps/extension/e2e/mlp-probe.cjs` | creates machine-account + token with `secrets:read`; prints the **denied** status for `GET /secrets/{uuid}/value` with the machine token, and the allowed status for `GET /secrets/{uuid}` (meta). Reference probe for scope enforcement (prints, does not hard-assert a 40x, but the denied status is emitted for inspection). ✅ |
| CLI | `live_e2e.rs` | provisions a machine account + prints `access token (shown once):`; **GAP:** does not assert that the machine token is denied reveal. |
| Desktop | `live_server_e2e.rs` | **GAP:** projects e2e does not test machine-account reveal scope at all. |
| Web | `apps/web/e2e/webauthn.spec.ts` | covers WebAuthn login path; **GAP:** no explicit scope-denial assertion. |

> Parity action: the `mlp-probe.cjs` scope-denial sequence is the canonical reference.
> Port the same 5 steps into the CLI, desktop, and web harnesses so scope enforcement
> is asserted on every client.

---

## Scenario 5 — Logout / session invalidation

Steps (canonical):
1. Logout (invalidates the current session token).
2. A protected call after logout must fail (401).

| Client | Harness | Asserts |
| --- | --- | --- |
| CLI | `live_e2e.rs` | `logout` succeeds; subsequent `list` fails (non-zero exit). |
| Desktop | — | **GAP:** no logout/invalidation assertion in `live_server_e2e.rs`. |
| Extension | — | **GAP:** `live-e2e.mjs` exits after decrypt, no logout. |
| Web | — | **GAP:** not covered. |

---

## Scenario 6 — MFA enrollment + recovery codes (canonical copy)

Steps (canonical):
1. Enroll a TOTP authenticator (`/auth/mfa/totp/issue` → verify).
2. After (re)verification, the server returns recovery codes **once**.
3. The client shows the recovery codes with the canonical warning:
   **"Save these now. They won't be shown again."**

| Client | Harness | Asserts |
| --- | --- | --- |
| Desktop | `live_server_e2e.rs` (none) / UI | `desktop_view.rs` now renders `mfa_recovery_codes` with the canonical warning (Phase 8). **GAP:** no e2e asserts the warning string. |
| Extension | — | **GAP:** not covered. |
| CLI | — | **GAP:** not covered. |
| Web | — | **GAP:** not covered. |

---

## How to run

- CLI: `cargo test -p vautr-cli --test live_e2e` (skips if no server; set `VAUTR_SERVER` / `VAUTR_SERVER_BIN`).
- Desktop: `cargo test -p vautr-desktop --test live_server_e2e -- --ignored` (requires `VAUTR_API_URL`).
- Extension: `node apps/extension/e2e/live-e2e.mjs` and `node apps/extension/e2e/mlp-probe.cjs` (requires server at :8080).
- Web: `pnpm --filter @vautr/web test` (Playwright; `apps/web/e2e` — note `webauthn.spec.ts`, not a full MLP round-trip yet).

## CI

The token-drift gate (`pnpm --filter @vautr/design-tokens check:tokens`) runs on
every push/PR in `.github/workflows/verify-isolation.yml` (Phase 9, deliverable 1).
The live E2E suites are `#[ignore]` / manual (require a running server) and are not
yet gated in CI — tracking issue for Phase 9 follow-up.
