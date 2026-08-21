# Contributing to Vautr

First off, thank you for considering contributing to Vautr. Because Vautr is built on a foundation of trust and open governance, the way we accept contributions is structurally important to protect the community and the project's AGPL license.

## 🛡️ The Constitution

All contributions must align with the [Vautr Constitution](./CONSTITUTION.md). We will not accept pull requests that introduce closed-source telemetry, gate self-hosting features, or compromise user data sovereignty. 

## 📜 Developer Certificate of Origin (DCO)

To ensure that Vautr remains purely open-source and that contributors have the legal right to submit their work under the AGPL-3.0 license, we require a **Developer Certificate of Origin (DCO)**. This is the same legal mechanism used by the Linux kernel and many major open-source projects.

By making a commit to this repository, you certify that you wrote it or have the right to submit it under the project's license. 

**To certify this, you must add a `Signed-off-by` line to every commit message:**

```text
feat(crypto): implement argon2id hashing

Signed-off-by: Jane Doe <jane.doe@example.com>
```

You can automatically add this line to your commits by using the `-s` flag:
```bash
git commit -s -m "feat(crypto): implement argon2id hashing"
```

*Note: PRs containing commits without the `Signed-off-by` line will fail the automated DCO check and cannot be merged.*

## 🤖 AI-Assisted Review Pipeline

We move at lightspeed, but never at the expense of security. All Pull Requests are automatically scanned by our AI Security Review pipeline (PR-Agent). 

If the AI flags a potential security vulnerability (e.g., unsafe Rust blocks, weak cryptographic implementations, XSS vectors), the PR will be flagged for manual review by a core maintainer before merging.

## 🛠️ How to Contribute

### Reporting Bugs
If you find a security vulnerability, **please do not open a public GitHub issue**. Instead, follow our [Security Policy](./SECURITY.md) to report it responsibly.

For non-security bugs, please open a GitHub Issue and use the provided Bug Report template. Include your environment details, steps to reproduce, and expected vs. actual behavior.

### Suggesting Enhancements
Open a GitHub Issue using the Feature Request template. Please explain *why* the feature is needed and how it aligns with Vautr's philosophy of user sovereignty and simplicity.

### Pull Requests
1. **Fork** the repository.
2. **Create a branch** from `main` (e.g., `feat/crypto-core` or `fix/mobile-sync`).
3. **Make your changes.** Ensure your code passes local linting and testing.
4. **Commit with DCO** using `git commit -s`.
5. **Open a Pull Request** against the `main` branch.
6. Fill out the PR template completely, describing the change and linking any relevant issues.
7. Wait for the AI security review and human code review.

## 📏 Code Style & Standards

- **Rust:** Follow standard `rustfmt` and `clippy` guidelines. No `unsafe` code without an exhaustive safety comment and justification.
- **React/TypeScript:** Follow the existing biome configuration.
- **Commit Messages:** We follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) (e.g., `feat:`, `fix:`, `docs:`, `refactor:`).

## 🚀 Local Onboarding & Build Scripts

The repo provides thin wrappers so a fresh checkout builds with one command. See `AGENTS.md` for the full dev reference.

- **`make help`** — list all targets.
- **`make web-bootstrap`** — builds the WASM crypto modules (`scripts/prepare-wasm.sh`) and installs web deps.
- **`make mobile-bootstrap`** — builds the `vautr-ffi` native lib, regenerates the Swift/Kotlin bindings, and installs mobile deps. The Android `.so` cross-compile needs the NDK (`cargo ndk -t arm64-v8a build -p vautr-ffi --release`) — see the `vautr-mobile-uniffi` skill.
- **`make server-build`** — release build of the Rust server.
- **`make test-all`** — `cargo test --workspace` plus the web/extension TS suites.
- **`make prepare-wasm` / `make check-wasm`** — (re)build or verify the prebuilt WASM artifacts. The clients `build` scripts run `check-wasm` so a missing `.wasm` fails the build fast instead of silently shipping the throwing dev shim.

### Verification gate

Before opening a PR, run the same gates CI runs:

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -D warnings
cargo test --workspace
pnpm -r typecheck && pnpm -r test
pnpm prepare:wasm   # ensure real crypto is built
```

Thank you for building the moat with us.
