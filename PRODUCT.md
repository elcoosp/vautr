# PRODUCT.md — Vautr

Durable product truth for Vautr. Visual decisions live in `DESIGN.md`; this file owns facts and constraints.

## Platform
- **Clients:** web app (React + TanStack Router + shadcn), React Native mobile (react-native-reusables + NativeWind), native desktop (Rust + GPUI/GPUI-component), and a browser extension (React + shadcn).
- **Server:** Rust (Axum) backend over SQLite (default) or PostgreSQL (opt-in), self-hosted.
- All clients are **live-server-connected**: they talk to a real running server at `http://localhost:8080`, not to mocks or demos.

## Users
Primary users are **developers, engineers, and security-conscious professionals** and teams. They sit at a desk with a real terminal and an IDE open, and they manage credentials daily: passwords, API keys, tokens, TOTP codes, and shared secrets. The same people use the mobile client to grab a credential on the go.

## Product Purpose
A **zero-knowledge password and secrets manager**. You store and manage secrets with the confidence that no server, no operator, and no attacker who compromises the host can ever read the plaintext.

## Positioning
**Zero-knowledge, self-hosted, live-connected.** Unlike a purely local vault, Vautr syncs to a server you control; unlike a hosted manager, that server is cryptographically unable to see your data. The meaningful difference is a proof: **the server holds only ciphertext**, and authentication uses OPAQUE so even the password never leaves the client.

## Operating Context
A professional security tool used at a desk (often a dark room, late, an IDE open) and on the phone between tasks. Precision, legibility, and trust matter more than spectacle. Users glance at the interface to verify a status, reveal a secret, generate a strong credential, or confirm an audit event — the scene rewards calm, deliberate, dense-but-clear presentation over decoration.

## Capabilities and Constraints
- **Vault** (local item list + reveal; `read_secret` is desktop-only), **Projects** (membership, roles, groups, offboarding), **Secrets** (store/reveal/rotate), **Generator** (strong credential generation), **MFA** (TOTP), **Settings**, **machine accounts + tokens**, **import/export**, **backups**, **audit log**.
- Canonical five-tab navigation applied across clients: **Projects / Secrets / Generator / MFA / Settings**.
- Zero-knowledge guarantees are real, not cosmetic: plaintext never reaches the server.
- MLP out of scope: SSO/SAML/OIDC, k8s, multi-language SDKs, native mobile OS integrations, MCP, enterprise/managed.

## Brand Commitments
- **Coherence across clients.** The same route map and the same visual system render on web, mobile, desktop, and extension. A user who switches surface should feel instantly at home.
- **Trust through precision.** Security software earns trust with legibility, correctness, and restraint, not with flash.
- **Accessibility:** WCAG AA contrast on text, keyboard-operable controls, visible focus, no colour-only meaning.
- **The interface never implies the server can see your secrets.** Zero-knowledge is the story; the UI reflects it.

## Evidence on Hand
- Real, working live-server clients across all four surfaces (no mocks).
- A committed, correct information architecture (five-tab map, per-entity screens) that must be preserved through any visual change.
- An incumbent look that is inconsistent: web and extension ship default light shadcn tokens while web hard-codes a dark navy body; mobile uses an unrelated dark-blue system; desktop uses GPUI defaults. This contradiction is the primary visual defect.

## Product Principles
- Operate mode: task completion, scanability, and consistency outrank expression; brand lives in precise details.
- Preserve the working information architecture and every functional behaviour; replace the look, never the function.
- One coherent visual world shared by all four clients, encoded as tokens in each surface's theming system.
- Light and dark are both first-class, chosen from the use scene, and consistent across clients.

## Accessibility & Inclusion
- Body and placeholder text ≥ 4.5:1 contrast; large text ≥ 3:1.
- Full keyboard navigation and visible focus on every interactive element.
- Semantic structure; labels paired with inputs; errors name the problem and the recovery.
