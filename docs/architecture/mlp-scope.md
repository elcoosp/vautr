# Vautr — MLP v1 Feature Scope

**Author:** Jcode
**Date:** 2026-08-11
**Status:** Approved scope for Minimum Lovable Product v1
**Audience:** Development, product, and any parallel build agents.

This document is the exhaustive, authoritative scope for **Vautr MLP v1**. It is the
single source of truth for *what* must be real and shipped. The parallel execution
plan that implements this scope lives in [`mlp-wave-plan.md`](./mlp-wave-plan.md).

Vautr is an **open-source, self-hosted, zero-knowledge** password *and* secrets
manager. It unifies Password Manager + Secrets Manager around one clear **Projects**
model, adds Machine Accounts + CLI + native Rust SDK, one-command Rust install,
idiot-proof backups/recovery, simple-but-sufficient permissions, and a modern UX.
It targets tech startups / SMBs (5–50 people) that are moving off
Excel / Bitwarden / Vaultwarden and care about privacy.

> Scope discipline: everything below is **in** scope for v1 and must be implemented
> for real (no demos, no mocks, no `todo!()` stubs). Section 6 lists what is
> explicitly **out** of scope so effort stays focused.

---

## 1. Architecture & Deployment (the core differentiator)

- **Open-source Rust server**: single lightweight binary, plus an optional one-click
  Docker deployment.
- **Ultra-simple install**: one command (or nearly so), with automatic reverse-proxy
  management, HTTPS / Let's Encrypt certificates.
- **Storage**: SQLite by default (simple) with PostgreSQL support.
- **Smart automatic backups** with a one-click restore test.
- **Simple, secure recovery / anti-lockout** system (backup keys, improved emergency
  access).
- **Downtime-free updates** wherever possible.
- **Basic integrated monitoring + alerts** (e.g., server-down alerting).

## 2. Organizational Model & Sharing (the UX core)

- **Single "Projects" concept** — replaces Folders + Collections with one simple
  mental model.
- An item belongs to **one Project only** (no multi-parent in v1, for clarity).
- A Project can be **personal** or **shared** within the organization.
- **Fixed, clear org roles:**
  - **Owner** — total control.
  - **Admin** — manages users, projects, permissions.
  - **Manager** — creates and manages access on *their own* projects only.
  - **Member** — standard user.
- **Per-Project permissions** (clear granularity without complicating UX):
  - **Can View** — see + autofill; optional *hide password in cleartext*.
  - **Can Edit** — create / modify / delete items.
  - **Can Manage** — manage who has access to the project.
- **Ultra-simple offboarding**: revoke all of a user's access in a few clicks.
- **Basic user groups** (optional but recommended for v1).

## 3. Password Manager (daily use)

- **Secure storage**: logins, secure notes, TOTP, passkeys.
- **Password generator** + **weak / reused password detection**.
- **Reliable, priority autofill** (browsers + mobile) — the single biggest pain
  point.
- **Native passkey + TOTP support**.
- **Mandatory MFA** (authenticator apps, FIDO2/WebAuthn, email).
- **Modern, fluid, pleasant web interface** (a strong differentiator vs Bitwarden /
  Vaultwarden).
- **Priority v1 clients**: Web vault + browser extensions + CLI.
- **Import / export** (Bitwarden, CSV, etc.).
- **Multi-device synchronization**.
- **Zero-knowledge, end-to-end encryption.**

## 4. Secrets Manager (for developers — essential)

- Secrets organized in the **same Projects** (password + secrets unification).
- **Machine Accounts** — non-human identities for CI/CD, apps, agents.
- **Access Tokens** with **expiration** and **revocation**.
- **Solid `bws`-style CLI**:
  - `get`, `list`, `run` (environment injection).
  - Basic create / edit.
- **Native Rust SDK** (a competitive advantage).
- **Secure programmatic access** (fine-grained scopes).
- **Basic audit logs** (who accessed what and when — important for tech SMBs).

## 5. Security & Privacy (ultra-privacy first)

- **End-to-end zero-knowledge encryption**.
- **Mandatory MFA** + basic policies (master password strength, etc.).
- **Basic audit logs** (org events + secret access).
- **No intrusive telemetry**.
- **Fully open source** (core is libre).

---

## 6. Explicitly Out of Scope for v1 (stay realistic)

- SSO / SAML / OIDC / SCIM / Directory sync.
- Ultra-granular custom roles.
- Kubernetes operator.
- Multi-language SDKs (Python, Go, JS…) beyond Rust.
- Full native mobile apps (responsive web + extensions first).
- MCP server / advanced AI-native features.
- Enterprise features (advanced policies, automated admin password reset, etc.).
- Managed hosting (that is part of the business model, not the open-source product).

---

## Reference

- This scope maps 1:1 to the parallel execution plan in
  [`mlp-wave-plan.md`](./mlp-wave-plan.md).
- Out-of-scope items in §6 must **not** receive build effort in v1.
