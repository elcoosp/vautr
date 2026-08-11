# 🛡️ Security Policy

Vautr is a zero-knowledge password manager. The security of our users' vaults is our absolute highest priority. We take all security vulnerabilities seriously and appreciate the efforts of the security research community in keeping Vautr safe.

This policy outlines how to report vulnerabilities and what you can expect in return.

## 🚨 Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

If you believe you have found a security vulnerability in Vautr (including the core Rust server, cryptographic implementations, or any official client), please report it to us privately via one of the following methods:

1. **GitHub Private Vulnerability Reporting (Preferred):** 
   Use the [GitHub Security Advisories](https://github.com/vautrorg/vautr/security/advisories/new) feature on our repository.
2. **Encrypted Email:**
   Send an email to `security@vautr.org`. If the vulnerability is sensitive, please encrypt your email using our PGP public key:
   *(PGP Key fingerprint and link will be added here upon project launch)*

### What to Include

To help us triage and resolve the issue quickly, please include:
- A clear description of the vulnerability.
- The affected component (e.g., `core/vautr-crypto`, `clients/desktop`, `clients/web`).
- Step-by-step instructions to reproduce the issue.
- The potential impact of the vulnerability (e.g., unauthorized vault access, bypass of zero-knowledge architecture).
- Any proof-of-concept code or screenshots.

## 🔄 Response Process & Timeline

We are committed to working with the security community to verify and resolve potential vulnerabilities. Here is our commitment to you:

1. **Acknowledgment:** We will acknowledge receipt of your report within **24 hours**.
2. **Triage:** We will assess the impact and severity of the vulnerability within **3 business days**.
3. **Updates:** We will keep you informed of our progress at least every **7 days** until the issue is resolved.
4. **Resolution:** We aim to release a patch for critical vulnerabilities within **14 days** of confirmation. More complex issues may require a longer timeline, which we will communicate clearly.

## 🧠 AI & Security Pipeline

Vautr utilizes an AI-assisted security review pipeline (PR-Agent) on all pull requests to catch common vulnerabilities (e.g., unsafe Rust, XSS, injection flaws) before they are merged. However, automated tools are not perfect. We rely on the manual review and responsible disclosure of the security community to find the deep, architectural flaws that AI misses.

## 🏅 Recognition & Rewards

While Vautr does not currently have a paid bug bounty program, we deeply value the work of security researchers. If you responsibly disclose a vulnerability, we offer:

- **Public Acknowledgment:** Credit in our Hall of Fame and the security advisory release notes (unless you prefer to remain anonymous).
- **Early Access:** Advance notice of security patches before they are publicly released, allowing you to verify the fix.
- **Swag:** As the project grows and acquires funding, we intend to launch a formal, paid bug bounty program.

## 🔢 Supported Versions

Security updates are applied only to the most recent release cycle. As a project focused on rapid, continuous delivery and self-hosting, we encourage all users and self-hosters to always run the latest version of Vautr.

| Version | Supported          |
| ------- | ------------------ |
| Latest  | ✅ |
| Older   | ❌ |

## 🧭 Safe Harbor

If you act in good faith and respect the privacy and data of Vautr users, we will not pursue legal action against you. We ask that you:

- Do not access, modify, or delete other users' data.
- Do not degrade the availability of our services (e.g., DDoS).
- Report vulnerabilities immediately without public disclosure until the patch is released.
- Do not use automated vulnerability scanners to spam our logs.

Thank you for helping us build the most secure and trustworthy password manager in the world.
