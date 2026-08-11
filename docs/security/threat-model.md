# Vautr Formal Threat Model

This document outlines the security assumptions, boundaries, and threat vectors for Vautr. It rigorously maps real-world attackers to the specific cryptographic and architectural mitigations built into the system. All code contributions and architectural decisions must align with this model.

## 1. Core Principle: Zero-Knowledge Architecture
Vautr operates on a strict Zero-Knowledge (Client-Side Encryption) model. 
*   The Vautr server **MUST NEVER** receive, process, or store unencrypted vault data or the keys to decrypt it.
*   Authentication is handled via the OPAQUE PAKE; the server never possesses a password equivalent.
*   If the server is fully compromised, the attacker gains zero access to user plaintext.

## 2. Attacker Profiles & Mitigations

### 2.1 The Passive Network Observer (Eve)
*   **Threat:** Intercepts API traffic between the client and server.
*   **Mitigation:** Strict TLS 1.3 enforcement. All API payloads contain only opaque ciphertext blobs. OPAQUE prevents MP derivation from intercepted traffic.

### 2.2 The Compromised/Malicious Server (Samy)
*   **Threat A (Data Exfiltration):** Attacker dumps the Vautr Postgres database.
    *   **Mitigation:** Data is XChaCha20-Poly1305 encrypted. The server lacks the SVK, MK, and MP. The data is mathematically irrecoverable.
*   **Threat B (Ciphertext Swap/Replay):** Attacker attempts to move a ciphertext from Item A to Item B, or replay an old version of Item A.
    *   **Mitigation:** Strict AEAD Associated Data (AD) binding (`uuid || enc_key_gen`). Swapping a ciphertext to a new UUID or key generation results in a `TagMismatch` error, marking the item as `ToxicIgnored` in the DashMap.
*   **Threat C (Replay/Downgrade):** Attacker attempts to force a lagged client to accept an old, potentially compromised ciphertext by overriding the `enc_key_gen`.
    *   **Mitigation:** The Server enforces `min_enc_key_gen` via `422 Unprocessable Entity`. The Client enforces the `KeyUpdateRequired` Read-Only Gate if `local_gen < server_gen`.

### 2.3 The Lagged Client (Stale SVK)
*   **Threat:** A legitimate user's offline device attempts to sync using an old Symmetric Vault Key (SVK) following a server-side key rotation. The device inadvertently attempts to overwrite newer, valid data with stale, older data.
*   **Mitigation:** The Server strictly rejects writes where `enc_key_gen < min_enc_key_gen` (422). The Client detects the epoch mismatch, transitions to the `KeyUpdateRequired` Read-Only Gate to prevent data destruction, and prompts for MP re-authentication to derive the new SVK.

### 2.4 The Compromised UI Bundle (Malicious JS/React)
*   **Threat:** A malicious script is injected into the React Native or Web bundle (e.g., via a compromised NPM package or XSS) attempting to exfiltrate decrypted secrets by calling the FFI bridge.
*   **Mitigation:** Compile-time feature flags strictly exclude the `read_secret` API from Mobile and WASM builds (CI Symbol Check enforces this). The Opaque Handle pattern ensures the JS layer never receives a string payload; it only receives a `SecretHandle` (u64), which is useless without the Native Module context.

### 2.5 The Offline Brute-Force Attacker (Mallory)
*   **Threat:** Attacker obtains a local device backup or a server DB dump and attempts to guess the Master Password.
*   **Mitigation:** Argon2id with high memory parameters (64 MiB baseline). The runtime automatically calibrates parameters to target ~300ms derivation time on the specific device, strictly bounding the guess rate without degrading UX.

### 2.6 The Physical Thief (Burglar)
*   **Threat:** Steals an unlocked or locked device.
*   **Mitigation (Locked):** The SVK is wrapped by the OS Keystore (Secure Enclave / TEE). Biometric authentication is required to unwrap the key.
*   **Mitigation (Unlocked/RAM):** Memory is locked via `mlock` to prevent OS paging to unencrypted swap files. `Zeroizing<T>` ensures keys are wiped from RAM the instant they leave scope.

### 2.7 The Crash Reporting SDK (Sentry/Bugsnag)
*   **Threat:** Third-party crash reporting libraries capture a memory dump (minidump) or stack trace containing local variables at the moment of a crash, exfiltrating decrypted secrets to their servers.
*   **Mitigation:** SDKs are strictly configured to disable minidump attachment and local variable capturing. Defense-in-depth is provided by `mlock` (preventing swap paging) and immediate `Zeroizing` on scope drop, ensuring secrets are erased *before* a crash reporter can snapshot them.

## 3. Out of Scope (Limits of the Architecture)
Security is about boundaries. Vautr cannot protect against the following:
1.  **Kernel-Level Malware:** If the user's OS is rooted/infected with a keylogger, keystrokes can be captured before Vautr encrypts them. RAM can be scraped by kernel drivers.
2.  **Phishing:** If a user enters their MP into a fake application, the vault can be decrypted.
3.  **Physical Access to Unlocked Device:** If a user leaves their device unlocked and authenticated, Vautr cannot prevent local access. (Mitigated by auto-lock timeouts and the Safety Reaper).

## 4. Trust Assumptions
1.  **Cryptography:** We assume standard primitives (XChaCha20-Poly1305, Argon2id, OPAQUE, HKDF) are secure.
2.  **Client Integrity:** We assume the Vautr binary running on the device has not been tampered with post-compilation (Enforced via Reproducible Builds and Code Signing).
3.  **OS CSPRNG:** We assume the OS-level Cryptographically Secure Pseudo-Random Number Generator is reliable.
4.  **Hardware Security:** We assume the OS Keystore (Secure Enclave / TEE) operates as specified and cannot be extracted.

---

# Document 2: The Vautr Security Whitepaper (Public-Facing)

## The Vautr Zero-Knowledge Guarantee

Vautr is engineered around a fundamental principle: **Your data is yours.** 

In a traditional password manager, the server acts as a trusted intermediary. It might encrypt your data at rest, but it holds the keys to decrypt it. If the server is breached, your data is exposed. 

Vautr operates on a strict, mathematically enforced Zero-Knowledge architecture. We cannot read your data, we cannot lose your data, and we cannot be compelled to hand your data over. This whitepaper details the cryptography and architecture that makes this guarantee possible.

---

## 1. The Key Hierarchy: Mathematical Isolation

Vautr never relies on a single point of failure. Your Master Password (MP) never leaves your device. Instead, it triggers a cascading derivation tree that isolates authentication from encryption.

1.  **Master Key (MK):** When you unlock Vautr, your MP is processed through **Argon2id**, a memory-hard Key Derivation Function. Vautr dynamically calibrates the computational cost on your specific device to target ~300ms derivation time. This makes offline brute-force attacks computationally infeasible without degrading your daily unlock experience.
2.  **Authentication (OPAQUE):** To prove your identity to the server without sending your password, Vautr uses the **OPAQUE PAKE protocol**. The server never receives a password, a hash of a password, or any data that could be used to derive your password. Even if the server is fully compromised, your MP remains safe.
3.  **Symmetric Vault Key (SVK):** Your vault data is encrypted by a completely random 256-bit SVK. This key is generated on your device and is wrapped (encrypted) by a key derived from your MK. The server only ever sees the encrypted blob of this SVK.

---

## 2. Data Encryption: XChaCha20-Poly1305

All vault data (passwords, notes, cards) is encrypted using **XChaCha20-Poly1305**, an Authenticated Encryption with Associated Data (AEAD) algorithm. 

**Why AEAD matters:** Standard encryption only guarantees secrecy. AEAD guarantees both secrecy and integrity. 

**Context Binding:** When Vautr encrypts an item, it binds the ciphertext to its specific context—its unique ID (`uuid`) and its encryption epoch (`enc_key_gen`). This is injected into the AEAD Associated Data (AD). 

**The Swap Attack Defense:** If a malicious server attempts to swap the ciphertext of "Bank Password A" into the slot of "Bank Password B," the AD context will mismatch upon decryption. Vautr will detect the tampering, refuse to decrypt the item, and flag it as `Toxic`. The server cannot tamper with your data without your client immediately knowing.

---

## 3. The Split Data Model: Minimizing the Attack Surface

When you view your vault list, you see your titles and usernames. When you view a specific item, you see the password. Vautr treats these as two entirely different security domains.

*   **Overviews:** Encrypted with a key derived specifically for fast list rendering (`OEK`).
*   **Secrets:** Encrypted with a separate, isolated key (`DEK`).

**The Server Boundary:** It is critical to understand that on the server, the encrypted Overview and the encrypted Secret are combined into a **single, atomic encrypted blob**. The server cannot differentiate between the two, nor can it decrypt either. 

**The Client Boundary:** The separation is strictly enforced in the client's memory. When you search your vault, the core never decrypts your passwords. Passwords are only decrypted the millisecond you explicitly request them, and are immediately zeroized from RAM when you navigate away.

---

## 4. Client Hardening: Protecting the RAM

Data at rest (on our servers) and data in transit (over the network) is protected by cryptography. Data in use (in your device's RAM) is the hardest to secure. Vautr implements strict mitigations:

*   **Memory Locking (`mlock`):** Vautr instructs the operating system to never page the memory regions holding your decrypted passwords to the SSD/Hard Drive swap file.
*   **Deterministic Zeroization:** The moment a password is no longer actively displayed or used for autofill, Vautr explicitly overwrites that memory region with zeros (`Zeroizing<T>`). We do not rely on the OS or the JavaScript garbage collector to clean up.
*   **Hardware-Backed Biometrics:** If you enable FaceID/TouchID, Vautr caches your Vault Key in the OS Secure Enclave (TEE). This allows instant unlocks without storing the derived key in standard application memory.
*   **Opaque Handles:** In our mobile and web applications, the user interface never handles raw password strings. When a password is decrypted, it is assigned an "Opaque Handle" (a secure reference). The UI passes this handle to the OS clipboard or autofill framework. The string itself never enters the JavaScript environment.

---

## 5. The No-Recovery Guarantee

Vautr is a true Zero-Knowledge system. This means **we do not have your data, and therefore we cannot give it back to you if you lose your Master Password.**

There are no backdoors, no master keys, and no "reset password" functionality. If you lose your Master Password, your data is cryptographically irrecoverable. This is not a flaw; it is the mathematical guarantee that no one—not even Vautr administrators or law enforcement—can access your vault without your consent.

We strongly recommend maintaining a secure, offline copy of your Master Password, or utilizing an Emergency Kit to safely store a recovery mechanism.

---

## 6. Incident Response & Transparency

*   **Crash Safety:** Vautr crash reporters are strictly configured to **never** attach core dumps, minidumps, or local variables to reports, ensuring decrypted secrets are never transmitted to our servers.
*   **Telemetry:** We track system health (sync durations, error rates) to detect regressions, but we strictly aggregate this data over 24 hours and strip all PII. We do not track what websites you visit or what passwords you use.
*   **Audits:** The cryptographic primitives and core architecture design are intended for independent, public security audits prior to the v1.0 general availability release.

**Conclusion:** Vautr is built on the premise that trust is verifiable, not assumed. By keeping the server dumb and the client smart, we ensure your digital identity remains exclusively yours.
