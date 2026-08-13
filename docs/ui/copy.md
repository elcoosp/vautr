# Vautr Copy & Voice

One author, always. This is the canonical copy guide for every client (web, extension,
desktop, mobile, CLI). When two strings say the same thing in two ways, the version here
wins.

## Voice

- **Direct, technical, calm.** Write like a senior engineer talking to another senior
  engineer. No marketing, no hand-holding.
- **Never alarmist.** A vault action is a fact, not a crisis.
- **No exclamation marks. Ever.** Not even one.
- **No success-theatre.** Do not tell the user the system "successfully" did what they
  just asked. State the outcome.
- **Terse.** Drop articles where the meaning survives. "Vault is locked." not
  "Your vault has been successfully locked!".

### Good / Bad

| Bad | Good |
|-----|------|
| Your secret has been successfully revealed! | Secret revealed. |
| Access denied!!! | Vault is locked. |
| We couldn't find any projects. | No projects yet. |
| Creating your project... | Creating project… |
| Project created successfully! | Created project 'acme'. |

## Canonical verbs

Use these exact words. Do not substitute synonyms.

| Concept | Use | Never use |
|---------|-----|-----------|
| Make a vault item's value visible | **reveal** | show, view, display, decrypt, see |
| Make it invisible again | **hide** | conceal |
| Lock the vault (drop DEK/SVK from memory) | **lock** | sign out, log out, logout (for the vault surface) |
| End the user *session* (not the vault) | **Log out** | sign off (only for the user session, never the vault) |
| Invalidate a token / machine account | **revoke** | delete, remove (for tokens/machine accounts) |
| Invalidate + reissue a value in place | **rotate** | update, change, refresh, regenerate |
| Destroy a vault item / project / secret | **delete** | remove, erase (for vault items/projects/secrets) |
| Generate a new credential | **create** / **generate** | make, add (prefer create/generate) |
| Pull remote changes | **sync** | refresh, update |

Special case — **lock vs log out:** the *vault surface* uses **lock** ("Vault is locked.").
The *user session* (signing out of the account, dropping the auth token) uses **Log out**.
Do not mix them.

## Canonical nouns

| Concept | Use | Never use |
|---------|-----|-----------|
| A record in the local vault store | **vault item** | login, credential (in the local store), entry |
| A project-scoped, server-backed value | **secret** | password, key, credential (server-side) |
| A non-human account backed by a keypair | **machine account** | service account, bot, robot, device account |
| A bearer credential for API access | **access token** | API key, credential, token key, PAT |
| The encrypted root key material | **vault key** (SVK) | master key, main key, root key (avoid "master") |
| One-time backup codes for MFA | **recovery codes** | backup codes (use "recovery" only) |

## Status lines

Two shapes only:

- **In-flight:** progressive tense + ellipsis.
  - `Creating project…`
  - `Revealing secret…`
  - `Syncing…`
  - Use the typographic ellipsis `…` (U+2026), **not** three periods `...`.
- **Done:** past tense + period. No exclamation.
  - `Created project 'acme'.`
  - `Secret revealed.`
  - `Vault locked.`
- **Failed:** state the blocker plainly, no blame, no "!" .
  - `Couldn't reach the server.`
  - `Wrong password.`

Never use "Working…" as a generic status — name the operation.

## Recovery codes

When recovery codes are displayed (MFA enrollment), always show the canonical warning
immediately above or beside the codes:

> **Save these now. They won't be shown again.**

This string is exactly that — title-case "Save", period after "now", period after
"again". Do not reword it per client.

## Placeholders

When a name is interpolated, use single quotes: `Created project 'acme'.`,
`Revoked token 'ci-deploy'.` Keep the quotes in the format string; do not concatenate
bare.
