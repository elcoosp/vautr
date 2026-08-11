/**
 * WebAuthn (FIDO2) optional second factor for MP unlock (VTR-052).
 *
 * Browser-facing bridge between the Vautr server's `/webauthn/*` endpoints and
 * the Web Authentication API (`navigator.credentials`). This module is gated by
 * the server's `webauthn` feature: when it is compiled out the `/webauthn/*`
 * routes do not exist, so callers should treat these as best-effort helpers and
 * degrade gracefully (VTR-052 acceptance: "falls back gracefully if no security
 * key is available").
 *
 * The WebAuthn ceremony is ONLINE-ONLY: the server must verify the assertion
 * signature + counter, so offline unlock is not supported (VTR-052 #5).
 */

/** A registered second-factor credential summary returned by the server. */
export interface WebauthnCredentialSummary {
  cred_id: string;
  label: string;
}

/** Response shape of `GET /webauthn/status`. */
export interface WebauthnStatus {
  second_factor_required: boolean;
  credentials: WebauthnCredentialSummary[];
}

/** Response shape of `GET /webauthn/credentials`. */
export interface WebauthnCredentialList {
  credentials: WebauthnCredentialSummary[];
}

/**
 * Browser `PublicKeyCredential` with the raw response buffers we must re-encode
 * to the base64url form the server's `RegisterPublicKeyCredential` /
 * `PublicKeyCredential` handlers deserialize.
 */
interface BrowserCredential {
  id: string;
  rawId: ArrayBuffer;
  response: Record<string, unknown>;
  type: string;
}

function b64url(buf: ArrayBuffer): string {
  const bytes = new Uint8Array(buf);
  let binary = '';
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
}

/** Encode a browser `PublicKeyCredential` into the JSON the server parses. */
function credentialToJson(cred: BrowserCredential): Record<string, unknown> {
  const raw = cred.response as Record<string, ArrayBuffer | null>;
  const response: Record<string, string | null> = {};
  for (const [key, value] of Object.entries(raw)) {
    if (value instanceof ArrayBuffer) {
      response[key] = b64url(value);
    } else if (value === null) {
      response[key] = null;
    }
  }
  return {
    id: cred.id,
    rawId: b64url(cred.rawId),
    type: cred.type,
    response,
  };
}

/**
 * Thin HTTP client for the `/webauthn/*` endpoints. Construct with the server
 * base URL (e.g. `http://localhost:3000`).
 */
export class WebauthnApi {
  constructor(private readonly baseUrl: string) {}

  private async request<T>(
    path: string,
    sessionToken: string,
    init: RequestInit = {},
  ): Promise<T> {
    const headers: Record<string, string> = {
      Authorization: `Bearer ${sessionToken}`,
      ...(init.body ? { 'Content-Type': 'application/json' } : {}),
      ...((init.headers as Record<string, string>) ?? {}),
    };
    const res = await fetch(`${this.baseUrl}${path}`, { ...init, headers });
    if (!res.ok) {
      let detail = `HTTP ${res.status}`;
      try {
        const body = await res.json();
        detail = body?.message ?? detail;
      } catch {
        /* non-JSON error body */
      }
      throw new Error(detail);
    }
    return (await res.json()) as T;
  }

  /**
   * Register a new security key / passkey as a second factor for the session's
   * user. Runs the full ceremony: start -> `navigator.credentials.create` ->
   * verify. Throws if no authenticator is available.
   */
  async register(sessionToken: string, label: string): Promise<void> {
    const start = await this.request<{
      request_id: string;
      challenge: CredentialCreationOptions;
    }>('/webauthn/register/start', sessionToken, {
      method: 'POST',
      body: JSON.stringify({ label }),
    });

    const credential = (await navigator.credentials.create(
      start.challenge,
    )) as unknown as BrowserCredential | null;
    if (!credential) {
      throw new Error('registration cancelled');
    }

    await this.request('/webauthn/register/verify', sessionToken, {
      method: 'POST',
      body: JSON.stringify({
        request_id: start.request_id,
        credential: credentialToJson(credential),
      }),
    });
  }

  /**
   * Complete a WebAuthn assertion for the session, satisfying the second factor
   * and un-gating `/account/status` (the wrapped-SVK fetch).
   */
  async assertSecondFactor(sessionToken: string): Promise<void> {
    const start = await this.request<{
      request_id: string;
      challenge: CredentialRequestOptions;
    }>('/webauthn/assert/start', sessionToken, { method: 'POST' });

    const credential = (await navigator.credentials.get(
      start.challenge,
    )) as unknown as BrowserCredential | null;
    if (!credential) {
      throw new Error('assertion cancelled');
    }

    await this.request('/webauthn/assert/verify', sessionToken, {
      method: 'POST',
      body: JSON.stringify({
        request_id: start.request_id,
        credential: credentialToJson(credential),
      }),
    });
  }

  /** Whether the account requires a second factor (and which keys are enrolled). */
  status(sessionToken: string): Promise<WebauthnStatus> {
    return this.request<WebauthnStatus>('/webauthn/status', sessionToken);
  }

  /** List the session's registered credentials. */
  listCredentials(sessionToken: string): Promise<WebauthnCredentialList> {
    return this.request<WebauthnCredentialList>(
      '/webauthn/credentials',
      sessionToken,
    );
  }

  /** Remove a credential (disables the second factor for that key). */
  async removeCredential(sessionToken: string, credId: string): Promise<void> {
    await this.request(`/webauthn/credentials/${encodeURIComponent(credId)}`, sessionToken, {
      method: 'DELETE',
    });
  }
}
