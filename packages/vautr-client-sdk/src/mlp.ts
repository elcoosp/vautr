/**
 * Vautr client SDK — MLP (Projects / Secrets / Machine Accounts / Tokens / MFA).
 *
 * A typed HTTP client over the frozen `@vautr/api-contract` schema for the
 * org-model endpoints added in Wave A (mlp-wave-plan §4). It shares the bearer
 * session token with the password-manager client (`VautrWebClient`) so one login
 * drives both the vault sync surface and the Projects/Secrets/MFA surface.
 *
 * The server remains a zero-knowledge blob store: secret *values* are opaque
 * `value_ciphertext` blobs held by the server keyed to the caller's scopes. The
 * `secrets:reveal` scope gates `GET /secrets/{uuid}/value`; without it the
 * server denies the read.
 */

import { ApiClient } from './api';
import type {
  AccessToken,
  AccessTokenCreateRequest,
  AccessTokenCreateResponse,
  AccessTokenListResponse,
  MachineAccount,
  MachineAccountCreateRequest,
  MachineAccountListResponse,
  MachineAccountUpdateRequest,
  MfaPolicy,
  MfaPolicyUpdateRequest,
  MfaStatus,
  Project,
  ProjectAddMemberRequest,
  ProjectCreateRequest,
  ProjectListResponse,
  ProjectMember,
  ProjectMemberListResponse,
  ProjectUpdateMemberRequest,
  ProjectUpdateRequest,
  Secret,
  SecretCreateRequest,
  SecretListResponse,
  SecretUpdateRequest,
  SecretValue,
  StatusResponse,
  TotpIssueResponse,
  TotpVerifyRequest,
  TotpVerifyResponse,
  WebAuthnEnrollFinishRequest,
  WebAuthnEnrollStartResponse,
} from '@vautr/api-contract';

/**
 * Typed client for the MLP org-model endpoints. Constructed with an `ApiClient`
 * that already carries the session token (share the instance from
 * `VautrWebClient`).
 */
export class VautrMlpClient {
  constructor(private readonly api: ApiClient) {}

  // -------------------------------------------------------------------------
  // Projects
  // -------------------------------------------------------------------------

  listProjects(): Promise<ProjectListResponse> {
    return this.api.request<ProjectListResponse>('GET', '/projects');
  }

  createProject(request: ProjectCreateRequest): Promise<Project> {
    return this.api.request<Project>('POST', '/projects', request);
  }

  getProject(uuid: string): Promise<Project> {
    return this.api.request<Project>('GET', `/projects/${uuid}`);
  }

  updateProject(uuid: string, request: ProjectUpdateRequest): Promise<Project> {
    return this.api.request<Project>('PATCH', `/projects/${uuid}`, request);
  }

  deleteProject(uuid: string): Promise<StatusResponse> {
    return this.api.request<StatusResponse>('DELETE', `/projects/${uuid}`);
  }

  listProjectMembers(uuid: string): Promise<ProjectMemberListResponse> {
    return this.api.request<ProjectMemberListResponse>('GET', `/projects/${uuid}/members`);
  }

  addProjectMember(uuid: string, request: ProjectAddMemberRequest): Promise<ProjectMember> {
    return this.api.request<ProjectMember>('POST', `/projects/${uuid}/members`, request);
  }

  updateProjectMember(
    uuid: string,
    userUuid: string,
    request: ProjectUpdateMemberRequest,
  ): Promise<ProjectMember> {
    return this.api.request<ProjectMember>('PATCH', `/projects/${uuid}/members/${userUuid}`, request);
  }

  removeProjectMember(uuid: string, userUuid: string): Promise<StatusResponse> {
    return this.api.request<StatusResponse>('DELETE', `/projects/${uuid}/members/${userUuid}`);
  }

  // -------------------------------------------------------------------------
  // Secrets
  // -------------------------------------------------------------------------

  listSecrets(projectUuid: string): Promise<SecretListResponse> {
    return this.api.request<SecretListResponse>('GET', `/projects/${projectUuid}/secrets`);
  }

  createSecret(request: SecretCreateRequest): Promise<Secret> {
    return this.api.request<Secret>('POST', '/secrets', request);
  }

  getSecret(uuid: string): Promise<Secret> {
    return this.api.request<Secret>('GET', `/secrets/${uuid}`);
  }

  updateSecret(uuid: string, request: SecretUpdateRequest): Promise<Secret> {
    return this.api.request<Secret>('PATCH', `/secrets/${uuid}`, request);
  }

  deleteSecret(uuid: string): Promise<StatusResponse> {
    return this.api.request<StatusResponse>('DELETE', `/secrets/${uuid}`);
  }

  /** Read a secret's value. Requires the `secrets:reveal` scope on the caller. */
  getSecretValue(uuid: string): Promise<SecretValue> {
    return this.api.request<SecretValue>('GET', `/secrets/${uuid}/value`);
  }

  // -------------------------------------------------------------------------
  // Machine accounts + access tokens
  // -------------------------------------------------------------------------

  listMachineAccounts(): Promise<MachineAccountListResponse> {
    return this.api.request<MachineAccountListResponse>('GET', '/machine-accounts');
  }

  createMachineAccount(request: MachineAccountCreateRequest): Promise<MachineAccount> {
    return this.api.request<MachineAccount>('POST', '/machine-accounts', request);
  }

  updateMachineAccount(uuid: string, request: MachineAccountUpdateRequest): Promise<MachineAccount> {
    return this.api.request<MachineAccount>('PATCH', `/machine-accounts/${uuid}`, request);
  }

  deleteMachineAccount(uuid: string): Promise<StatusResponse> {
    return this.api.request<StatusResponse>('DELETE', `/machine-accounts/${uuid}`);
  }

  listTokens(): Promise<AccessTokenListResponse> {
    return this.api.request<AccessTokenListResponse>('GET', '/tokens');
  }

  createToken(request: AccessTokenCreateRequest): Promise<AccessTokenCreateResponse> {
    return this.api.request<AccessTokenCreateResponse>('POST', '/tokens', request);
  }

  revokeToken(uuid: string): Promise<StatusResponse> {
    return this.api.request<StatusResponse>('DELETE', `/tokens/${uuid}`);
  }

  // -------------------------------------------------------------------------
  // MFA
  // -------------------------------------------------------------------------

  mfaStatus(): Promise<MfaStatus> {
    return this.api.request<MfaStatus>('GET', '/mfa/status');
  }

  mfaPolicy(): Promise<MfaPolicy> {
    return this.api.request<MfaPolicy>('GET', '/mfa/policy');
  }

  updateMfaPolicy(request: MfaPolicyUpdateRequest): Promise<MfaPolicy> {
    return this.api.request<MfaPolicy>('PUT', '/mfa/policy', request);
  }

  totpIssue(): Promise<TotpIssueResponse> {
    return this.api.request<TotpIssueResponse>('POST', '/mfa/totp/issue');
  }

  totpVerify(request: TotpVerifyRequest): Promise<TotpVerifyResponse> {
    return this.api.request<TotpVerifyResponse>('POST', '/mfa/totp/verify', request);
  }

  webauthnEnrollStart(displayName: string): Promise<WebAuthnEnrollStartResponse> {
    return this.api.request<WebAuthnEnrollStartResponse>('POST', '/mfa/webauthn/enroll/start', {
      display_name: displayName,
    });
  }

  webauthnEnrollFinish(request: WebAuthnEnrollFinishRequest): Promise<StatusResponse> {
    return this.api.request<StatusResponse>('POST', '/mfa/webauthn/enroll/finish', request);
  }
}

/**
 * Open a **machine-account session** — a token-authenticated client for a
 * non-human identity (Wave C seam).
 *
 * A machine account does not log in with OPAQUE; it authenticates with a
 * long-lived access token issued under its granted scopes (Wave A2). This
 * builds a token-authenticated [`VautrMlpClient`] directly from that token, so
 * the caller can immediately `listSecrets` / `getSecretValue` (and any other
 * operation the token's scopes permit) without any human handshake.
 *
 * Only operations within the token's granted scopes are permitted; the server
 * rejects scope over-requests with `403 scope_not_allowed`.
 */
export function createMachineAccountSession(
  token: string,
  options: { baseUrl?: string } = {},
): VautrMlpClient {
  const api = new ApiClient({ baseUrl: options.baseUrl });
  api.setToken(token);
  return new VautrMlpClient(api);
}
