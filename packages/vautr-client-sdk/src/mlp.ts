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

import type {
  AccessTokenCreateRequest,
  AccessTokenCreateResponse,
  AccessTokenListResponse,
  AuditEntry,
  AuditListQuery,
  BackupExportResponse,
  BackupRestoreResponse,
  BackupStatus,
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
import { ApiClient } from './api';

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
    return this.api.request<ProjectMember>(
      'PATCH',
      `/projects/${uuid}/members/${userUuid}`,
      request,
    );
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

  // ---------------------------------------------------------------------------
  // Backup / export / import (VTR-039)
  // ---------------------------------------------------------------------------

  backupStatus(): Promise<BackupStatus> {
    return this.api.request<BackupStatus>('GET', '/backup');
  }

  backupExport(request: { include_secrets?: boolean }): Promise<BackupExportResponse> {
    return this.api.request<BackupExportResponse>('POST', '/backup/export', request);
  }

  /**
   * List audit-log entries (VTR-071). The server endpoint is metadata-only
   * (actor / action / timestamp), never secret plaintext.
   */
  auditList(query?: AuditListQuery): Promise<AuditEntry[]> {
    const qs = auditQueryString(query);
    return this.api.request<AuditEntry[]>('GET', `/audit${qs}`);
  }

  backupRestore(request: {
    backup_id?: string;
    archive_base64?: string;
  }): Promise<BackupRestoreResponse> {
    return this.api.request<BackupRestoreResponse>('POST', '/backup/restore', request);
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

  updateMachineAccount(
    uuid: string,
    request: MachineAccountUpdateRequest,
  ): Promise<MachineAccount> {
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

  // -------------------------------------------------------------------------
  // Sharing (ADR-007) — zero-knowledge 1:1 item share relay
  // -------------------------------------------------------------------------

  /** Look up a user's published X25519 sharing public key. */
  getSharingPublicKey(userId: string): Promise<{ user_id: string; public_key: string }> {
    return this.api.request('GET', `/users/${userId}/public-key`);
  }

  /** Publish/replace our own sharing public key (caller must equal `userId`). */
  publishSharingPublicKey(
    userId: string,
    publicKeyB64: string,
  ): Promise<{ user_id: string; public_key: string }> {
    return this.api.request('PUT', `/users/${userId}/public-key`, { public_key: publicKeyB64 });
  }

  /** Create a 1:1 share: the KEM envelope (`wrapped_sik` + `ephemeral_public_key`). */
  createShare(request: {
    item_uuid: string;
    recipient_uuid: string;
    wrapped_sik: string;
    ephemeral_public_key: string;
  }): Promise<{
    share_id: string;
    sender_uuid: string;
    recipient_uuid: string;
    item_uuid: string;
  }> {
    return this.api.request('POST', '/shares/', request);
  }

  /** Deliver the DEM-encrypted payload for a share (keyed by `item_uuid`). */
  uploadSharePayload(
    itemUuid: string,
    payloadB64: string,
  ): Promise<{ share_id: string; status: string }> {
    return this.api.request('POST', `/shares/${itemUuid}/payload`, { payload: payloadB64 });
  }

  /** List shares waiting in our inbox. */
  listShareInbox(): Promise<
    Array<{
      share_id: string;
      sender_uuid: string;
      item_uuid: string;
      wrapped_sik: string;
      ephemeral_public_key: string;
      encrypted_payload: string | null;
    }>
  > {
    return this.api.request('GET', '/shares/inbox');
  }

  /** Revoke a share we own. */
  revokeShare(itemUuid: string): Promise<{ share_id: string; status: string }> {
    return this.api.request('DELETE', `/shares/${itemUuid}`);
  }

  // -------------------------------------------------------------------------
  // Group sharing (sharing-pki.md §6) — zero-knowledge 1:N item share relay
  // -------------------------------------------------------------------------

  /** Create a sharing group; the caller becomes the admin. */
  createGroup(request: {
    name: string;
  }): Promise<{ group_id: string; name: string; admin_uuid: string }> {
    return this.api.request('POST', '/groups/', request);
  }

  /** Add a member: deliver their Group SIK wrap (KEM envelope). Admin-only. */
  addGroupMember(
    groupId: string,
    request: { member_uuid: string; wrapped_sik: string; ephemeral_public_key: string },
  ): Promise<{ group_id: string; status: string }> {
    return this.api.request('POST', `/groups/${groupId}/members`, request);
  }

  /** List groups the caller belongs to, with the member's wrapped Group SIK. */
  groupInbox(): Promise<
    Array<{
      group_id: string;
      name: string;
      admin_uuid: string;
      wrapped_sik: string | null;
      ephemeral_public_key: string | null;
    }>
  > {
    return this.api.request('GET', '/groups/inbox');
  }

  /** Rotate the Group SIK: deliver re-wrapped keys for remaining members. Admin-only. */
  rotateGroup(
    groupId: string,
    request: {
      wrapped_keys: Array<{
        recipient_user_id: string;
        wrapped_sik: string;
        ephemeral_public_key: string;
      }>;
    },
  ): Promise<{ group_id: string; status: string }> {
    return this.api.request('POST', `/groups/${groupId}/rotate`, request);
  }

  /** Remove a member from a group. Admin-only. */
  removeGroupMember(
    groupId: string,
    memberUuid: string,
  ): Promise<{ group_id: string; status: string }> {
    return this.api.request('DELETE', `/groups/${groupId}/members/${memberUuid}`);
  }

  /** Upload a Group-SIK-encrypted item payload. Admin-only. */
  addGroupItem(
    groupId: string,
    request: { item_uuid: string; payload: string },
  ): Promise<{ group_id: string; item_uuid: string; payload: string }> {
    return this.api.request('POST', `/groups/${groupId}/items`, request);
  }

  /** List a group's shared items (item_uuid + Group-SIK-encrypted payload). */
  listGroupItems(
    groupId: string,
  ): Promise<Array<{ group_id: string; item_uuid: string; payload: string }>> {
    return this.api.request('GET', `/groups/${groupId}/items`);
  }

  /** Remove an item from a group. Admin-only. */
  deleteGroupItem(
    groupId: string,
    itemUuid: string,
  ): Promise<{ group_id: string; item_uuid: string; status: string }> {
    return this.api.request('DELETE', `/groups/${groupId}/items/${itemUuid}`);
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

/** Serialize an audit query into a URL query string (omitting empty fields). */
function auditQueryString(query?: AuditListQuery): string {
  if (!query) return '';
  const params = new URLSearchParams();
  const set = (k: string, v: string | number | null | undefined) => {
    if (v !== undefined && v !== null && v !== '') params.set(k, String(v));
  };
  set('user_id', query.user_id);
  set('actor', query.actor);
  set('event_type', query.event_type);
  set('resource_type', query.resource_type);
  set('resource_id', query.resource_id);
  set('from', query.from);
  set('to', query.to);
  set('limit', query.limit);
  set('offset', query.offset);
  const s = params.toString();
  return s ? `?${s}` : '';
}
