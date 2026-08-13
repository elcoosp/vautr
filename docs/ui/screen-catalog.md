# Vautr Screen Catalog (canonical screens)

> Phase 0 canon. Enumerates the canonical screens every Vautr client must
> implement (or document why it omits one). Stable screen ids are the contract
> later phases use for nav parity, primitive parity, and E2E parity.
>
> Companion docs: [design-system.md](./design-system.md),
> [patterns.md](./patterns.md), [copy.md](./copy.md).

---

## 1. Canonical screen list

Every client implements this set. A client may omit a screen only with a
documented reason recorded in §3.

| Stable id | Name | Purpose |
| --- | --- | --- |
| `unlock` | Unlock / Login / Register | Authenticate (OPAQUE) or create an account. |
| `dashboard` | Dashboard | Overview: stat tiles, recent projects, backup status. |
| `projects` | Projects | Project scopes + roles. |
| `vault` | Vault | Local item list + reveal (offline-first). |
| `secrets` | Secrets | Project-scoped, server-backed secret values. |
| `generator` | Generator | Password / TOTP / recovery-code generation. |
| `machine-accounts` | Machine accounts | Non-human identities. |
| `tokens` | Access tokens | Fine-grained API tokens. |
| `mfa` | MFA & security | TOTP / recovery codes / security settings. |
| `backup` | Backup (Import / Export) | Offline export (VTR-058) + import (VTR-027). |
| `settings` | Settings | User + client preferences. |

**One name, everywhere.** Pick `MFA & security` (not `MFA` on one client and
`MFA & Security` on another). Pick `Access tokens` (not `Tokens` in one place
and `API keys` in another). The noun canon lives in [copy.md](./copy.md).

---

## 2. Canonical nav order

Matches the desktop sidebar today (the most complete client). Every client
orders its top-level navigation identically; collapsing rules are per-client
(§3).

```
Dashboard · Projects · Vault · Generator · Secrets ·
Machine accounts · Access tokens · MFA & security · Backup · Settings
```

---

## 3. Per-client mapping (current state, grounded)

### Desktop — `apps/desktop/src/desktop_view.rs`
Full `Section` enum, all 10 implemented: `Dashboard, Projects, Vault,
Generator, Secrets, MachineAccounts, Tokens, Mfa, ImportExport, Settings`.
Sidebar is the canonical nav. **This is the reference implementation.**

### Web — `apps/web`
Should mirror desktop's 10 (top-nav or sidebar). Verify during Phase 2 that the
web nav uses the exact same labels + order as desktop.

### Extension popup (360×420) — `apps/extension/src/popup/components/`
Currently **5 tabs**: `VaultTab, ProjectsTab, SecretsTab, GeneratorTab,
MfaTab`. This matches the plan's "pick the 5 most relevant" collapse. The rest
(Secrets overview, Machine accounts, Access tokens, Dashboard, Backup,
Settings) are gated behind **"Open full app" → web**. **Canonize this collapse.**

### Mobile — `mobile`
The dump shows only a `Badge`/`Button`; the nav surface is undefined. **Plan:**
bottom-tab for the top-3 (`Vault`, `Generator`, `Settings`) + a "More" sheet
for the rest. Mobile must define its nav in Phase 2 (no nav exists today).

### CLI — `cli`
No screens; subcommands map to canonical screens. `--help` should use the
canonical screen names so a GUI-learned user can find `vautr-cli get` ≈
"Vault → Reveal". Document the mapping in Phase 2.

---

## 4. Brand header canon
- Desktop sidebar header: `size-8` brand tile + "Vautr" wordmark.
- Extension popup header: text-only today.
- Mobile: none today.
- **Canon:** brand tile (8×8 or 6×6) + wordmark in `--font-heading` semibold +
  optional username subtitle. Standardize across all three.

---

## 5. Login / register surface parity
- Desktop: segmented "Log in / Register" toggle.
- Extension: shadcn `Tabs` (`TabsList grid-cols-2`).
- Mobile: `/login` route.
- **Canon:** a single segmented control (two tabs, one form with a mode
  toggle). The desktop segmented toggle reads cleanest — adopt it everywhere.

---

## 6. Exit criteria (Phase 0)
- A screenshot of any section on any client is identifiable as "the Vautr X
  screen" within 1 second.
- Nav labels match exactly across clients (no `MFA` vs `MFA & security`).
- Each omitted screen has a recorded reason in §3.
