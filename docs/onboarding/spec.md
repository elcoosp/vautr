# Vautr Onboarding — Shared Spec (all clients)

This document is the **single source of truth** for the first-run guided onboarding
flow. Web and extension already implement it (VTR-073). Mobile (VTR-074) and
desktop (VTR-075) must render the *same* steps and copy, on their own stack.

The flow is a **headless multi-step engine** (OnboardJS on the web/extension/mobile
React clients; a faithful Rust re-implementation on desktop). It shows **once** after
a user's first register/login, then never blocks the app again unless replayed.

## Steps (ordered, exact copy is the contract)

| # | id | skippable | title | body | primary CTA | CTA target |
|---|----|-----------|-------|------|-------------|------------|
| 1 | `welcome` | no | Welcome to Vautr | Vautr is a zero-knowledge vault: your secrets are encrypted on your device and the server never sees them. Let's set up the essentials in about a minute. | (Next) | — |
| 2 | `create-vault` | no | Create your first vault | A vault (project) groups your secrets. You can create more later. | Create vault & continue | Calls createProject({ name: <default "My Vault"> }); on success advances. |
| 3 | `emergency-kit` | yes | Save your Emergency Kit | The Emergency Kit lets you recover your account if you forget your master password. It is generated and encrypted locally — store it somewhere safe (password manager, printed copy). | Open security settings | Navigates to the Emergency Kit / recovery surface. |
| 4 | `add-secret` | yes | Add your first secret | Open your vault and add a login, note, or card. Everything is encrypted before it leaves your device. | Go to my vault | Navigates to the secrets surface. |
| 5 | `done` | n/a (last) | You're all set | That's the core loop: vault → Emergency Kit → secrets. You can replay this tour anytime from Settings. | (Finish) | — |

- Navigation: Next / Back / Skip (skip only on steps 3 & 4). Finish on step 5.
- Progress indicator: "Step N of 5".

## Per-platform mapping

| Platform | Engine | Styling | First-run persistence | Replay affordance |
|----------|--------|---------|----------------------|-------------------|
| web | `@onboardjs/react` | shadcn Card/Button | `localStorage['vautr_onboarding_v1']` | Settings card |
| extension | `@onboardjs/react` | shadcn Card/Button | `localStorage['vautr_onboarding_v1']` | popup footer button |
| mobile (VTR-074) | `@onboardjs/react` | nativewind (className) | AsyncStorage key `vautr_onboarding_v1` via custom persistence hooks | Settings screen entry |
| desktop (VTR-075) | Rust flow in `crate::onboarding` | gpui-component widgets + `theme.rs` | JSON file `~/.config/vautr/onboarding.json` `{ seen_v1: bool }` | Settings entry |

## Hard rules (apply to every platform)
- **No stubs.** Every CTA drives a real surface: create vault calls the client
  `createProject`; "go to my vault"/"open security settings" navigate to the real
  destination. If a target surface does not yet exist, that is a separate gap — do
  NOT fake the CTA.
- **Zero-knowledge preserved.** The flow only navigates and calls existing client
  APIs. No secret plaintext is rendered by the flow itself. The Emergency Kit step
  points at the existing (ZK) recovery flow.
- **First-run only.** After completion the flag is set and the overlay never shows
  again on normal launch. Replay explicitly clears the flag and re-runs.
- **Step copy is the contract** above — do not reword per platform without updating
  this spec and all four clients together.

## Known cross-platform gap (tracked separately)
- **Emergency Kit UI surface.** No dedicated Emergency Kit / recovery UI was found
  on any client at spec-authoring time. Web/ext currently point the kit step at
  Settings/MFA. Mobile + desktop must point at the same eventual surface. Building
  that UI is OUT OF SCOPE for VTR-074/075 — flag it as a follow-up, don't block.

## Deferred (not in this spec)
- **Feature/product tour** (anchored tooltips highlighting UI elements). OnboardJS
  has no element-anchor API; desktop would need a GPUI spotlight overlay. Tracked
  as a follow-up issue.
