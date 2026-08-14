/**
 * Typed MLP v1 API client for the mobile app, built on the frozen generated
 * contract (`@vautr/api-contract`). Read-only use of the contract types; the
 * transport lives in `./http`. All methods are typed to the server schemas.
 */

import type {
  AccessScope,
  AccessToken,
  accountStatusResponse,
  BackupExportRequest,
  BackupExportResponse,
  BackupStatus,
  MachineAccount,
  MfaMethod,
  MfaStatus,
  OrgRole,
  Project,
  ProjectMember,
  ProjectPermission,
  ProjectType,
  Secret,
  SecretValue,
  TotpIssueResponse,
  TotpVerifyResponse,
} from '@vautr/api-contract';

import { HttpClient } from './http';

export type {
  AccessScope,
  AccessToken,
  accountStatusResponse,
  BackupExportRequest,
  BackupExportResponse,
  BackupStatus,
  MachineAccount,
  MfaMethod,
  MfaStatus,
  OrgRole,
  Project,
  ProjectMember,
  ProjectPermission,
  ProjectType,
  Secret,
  SecretValue,
  TotpIssueResponse,
  TotpVerifyResponse,
};

/** OPAQUE handshake wire types (api.md §3). */
export interface AuthStartResponse {
  registration_response?: string;
  login_response?: string;
}
export interface LoginFinishResponse {
  session_token: string;
  expires_at: number;
}
export interface RegisterFinishResponse {
  status?: string;
}

/** Options for creating a new secret. */
export interface SecretCreateInput {
  project_uuid: string;
  key: string;
  value_ciphertext: string;
}

export class MobileApiClient {
  readonly http: HttpClient;

  constructor(options: { baseUrl?: string; token?: string } = {}) {
    this.http = new HttpClient({ baseUrl: options.baseUrl });
    if (options.token) this.http.setToken(options.token);
  }

  setToken(token: string | null): void {
    this.http.setToken(token);
  }

  // ── Auth (OPAQUE handshake, api.md §3) ────────────────────────────────
  async authRegisterStart(username: string, registrationStart: string): Promise<AuthStartResponse> {
    return this.http.request<AuthStartResponse>('POST', '/auth/register/start', {
      username,
      registration_start: registrationStart,
    });
  }
  async authRegisterFinish(
    username: string,
    registrationFinish: string,
    serverPublicKey: string,
    kdfSalt: string,
    svkCiphertextBlob: string,
    svkCiphertextBlobRk: string,
  ): Promise<RegisterFinishResponse> {
    return this.http.request<RegisterFinishResponse>('POST', '/auth/register/finish', {
      username,
      registration_finish: registrationFinish,
      server_public_key: serverPublicKey,
      kdf_salt: kdfSalt,
      svk_ciphertext_blob: svkCiphertextBlob,
      svk_ciphertext_blob_rk: svkCiphertextBlobRk,
    });
  }
  async authLoginStart(username: string, loginStart: string): Promise<AuthStartResponse> {
    return this.http.request<AuthStartResponse>('POST', '/auth/login/start', {
      username,
      login_start: loginStart,
    });
  }
  async authLoginFinish(username: string, loginFinish: string): Promise<LoginFinishResponse> {
    return this.http.request<LoginFinishResponse>('POST', '/auth/login/finish', {
      username,
      login_finish: loginFinish,
    });
  }

  // ── Account ───────────────────────────────────────────────────────────
  async accountStatus(): Promise<accountStatusResponse> {
    return this.http.request<accountStatusResponse>('GET', '/account/status');
  }

  // ── Projects (Wave A1) ────────────────────────────────────────────────
  async listProjects(): Promise<Project[]> {
    const res = await this.http.request<{ projects: Project[] }>('GET', '/projects');
    return res.projects;
  }
  async createProject(input: {
    name: string;
    description?: string;
    type?: ProjectType;
  }): Promise<Project> {
    return this.http.request<Project>('POST', '/projects', input);
  }
  async getProject(uuid: string): Promise<Project> {
    return this.http.request<Project>('GET', HttpClient.interpolate('/projects/{uuid}', { uuid }));
  }
  async updateProject(
    uuid: string,
    input: { name?: string; description?: string },
  ): Promise<Project> {
    return this.http.request<Project>(
      'PATCH',
      HttpClient.interpolate('/projects/{uuid}', { uuid }),
      input,
    );
  }

  // ── Project members & roles/permissions (Wave A1) ─────────────────────
  async listProjectMembers(uuid: string): Promise<ProjectMember[]> {
    const res = await this.http.request<{ members: ProjectMember[] }>(
      'GET',
      HttpClient.interpolate('/projects/{uuid}/members', { uuid }),
    );
    return res.members;
  }
  async addProjectMember(
    uuid: string,
    input: {
      user_uuid: string;
      role?: OrgRole;
      permission?: ProjectPermission;
    },
  ): Promise<ProjectMember> {
    return this.http.request<ProjectMember>(
      'POST',
      HttpClient.interpolate('/projects/{uuid}/members', { uuid }),
      input,
    );
  }
  async updateProjectMember(
    uuid: string,
    userUuid: string,
    input: { role?: OrgRole; permission?: ProjectPermission },
  ): Promise<ProjectMember> {
    return this.http.request<ProjectMember>(
      'PATCH',
      HttpClient.interpolate('/projects/{uuid}/members/{user_uuid}', { uuid, user_uuid: userUuid }),
      input,
    );
  }
  async removeProjectMember(uuid: string, userUuid: string): Promise<void> {
    await this.http.request(
      'DELETE',
      HttpClient.interpolate('/projects/{uuid}/members/{user_uuid}', { uuid, user_uuid: userUuid }),
    );
  }

  // ── Secrets (Wave A3) ─────────────────────────────────────────────────
  async listProjectSecrets(projectUuid: string): Promise<Secret[]> {
    const res = await this.http.request<{ secrets: Secret[] }>(
      'GET',
      HttpClient.interpolate('/projects/{uuid}/secrets', { uuid: projectUuid }),
    );
    return res.secrets;
  }
  async createSecret(input: SecretCreateInput): Promise<Secret> {
    return this.http.request<Secret>('POST', '/secrets', {
      project_uuid: input.project_uuid,
      key: input.key,
      value_ciphertext: input.value_ciphertext,
    });
  }
  async updateSecret(
    uuid: string,
    input: { key?: string; value_ciphertext?: string },
  ): Promise<Secret> {
    return this.http.request<Secret>(
      'PATCH',
      HttpClient.interpolate('/secrets/{uuid}', { uuid }),
      input,
    );
  }
  async deleteSecret(uuid: string): Promise<void> {
    await this.http.request('DELETE', HttpClient.interpolate('/secrets/{uuid}', { uuid }));
  }
  /** Reveal a secret value (gated by `secrets:reveal` scope on the server). */
  async revealSecret(uuid: string): Promise<SecretValue> {
    return this.http.request<SecretValue>(
      'GET',
      HttpClient.interpolate('/secrets/{uuid}/value', { uuid }),
    );
  }

  // ── MFA (Wave A4) ─────────────────────────────────────────────────────
  async mfaStatus(): Promise<MfaStatus> {
    return this.http.request<MfaStatus>('GET', '/mfa/status');
  }
  async mfaTotpIssue(): Promise<TotpIssueResponse> {
    return this.http.request<TotpIssueResponse>('POST', '/mfa/totp/issue');
  }
  async mfaTotpVerify(input: {
    enrollment_id?: string;
    code: string;
  }): Promise<TotpVerifyResponse> {
    return this.http.request<TotpVerifyResponse>('POST', '/mfa/totp/verify', input);
  }

  // ── Backup / import-export (Wave A5) ───────────────────────────────────
  async getBackupStatus(): Promise<BackupStatus> {
    return this.http.request<BackupStatus>('GET', '/backup');
  }
  async exportBackup(input?: BackupExportRequest): Promise<BackupExportResponse> {
    return this.http.request<BackupExportResponse>('POST', '/backup/export', input ?? {});
  }

  async listMachineAccounts(): Promise<MachineAccount[]> {
    const res = await this.http.request<{ machine_accounts: MachineAccount[] }>(
      'GET',
      '/machine-accounts',
    );
    return res.machine_accounts;
  }
  async createMachineAccount(input: {
    name: string;
    description?: string;
    project_uuid?: string;
    scopes: AccessScope[];
    expires_at?: number;
  }): Promise<MachineAccount> {
    return this.http.request<MachineAccount>('POST', '/machine-accounts', input);
  }
  async listTokens(): Promise<import('@vautr/api-contract').AccessToken[]> {
    const res = await this.http.request<{ tokens: import('@vautr/api-contract').AccessToken[] }>(
      'GET',
      '/tokens',
    );
    return res.tokens;
  }
  async createToken(input: {
    name: string;
    machine_account_uuid?: string;
    project_uuid?: string;
    scopes: AccessScope[];
    expires_at?: number;
  }): Promise<import('@vautr/api-contract').AccessTokenCreateResponse> {
    return this.http.request<import('@vautr/api-contract').AccessTokenCreateResponse>(
      'POST',
      '/tokens',
      input,
    );
  }
  async revokeToken(uuid: string): Promise<void> {
    await this.http.request('DELETE', HttpClient.interpolate('/tokens/{uuid}', { uuid }));
  }

  // ── Sharing PKI relay (ADR-007 / sharing-pki.md) ──────────────────────
  /** Publish this user's sharing public key (X25519, base64). */
  async publishSharingPublicKey(userId: string, publicKeyB64: string): Promise<void> {
    await this.http.request(
      'PUT',
      HttpClient.interpolate('/users/{user_id}/public-key', { user_id: userId }),
      { sharing_public_key: publicKeyB64 },
    );
  }
  /** Fetch a recipient's sharing public key (base64), or null if unset. */
  async getSharingPublicKey(userId: string): Promise<string | null> {
    const res = await this.http.request<{ sharing_public_key: string | null }>(
      'GET',
      HttpClient.interpolate('/users/{user_id}/public-key', { user_id: userId }),
    );
    return res.sharing_public_key;
  }
  /** Create a 1:1 share relay record (server stores wrapped_sik + payload). */
  async createShare(input: {
    item_uuid: string;
    recipient_uuid: string;
    wrapped_sik: string;
    ephemeral_public_key: string;
  }): Promise<void> {
    await this.http.request('POST', '/shares', input);
  }
  /** Upload the share's SIK-encrypted payload blob. */
  async uploadSharePayload(itemUuid: string, payloadB64: string): Promise<void> {
    await this.http.request(
      'POST',
      HttpClient.interpolate('/shares/{item_uuid}/payload', { item_uuid: itemUuid }),
      { encrypted_payload: payloadB64 },
    );
  }
  /** List shares waiting in our inbox. */
  async listShareInbox(): Promise<
    Array<{
      share_id: string;
      sender_uuid: string;
      item_uuid: string;
      wrapped_sik: string;
      ephemeral_public_key: string;
      encrypted_payload: string;
    }>
  > {
    const res = await this.http.request<{
      shares: Array<{
        share_id: string;
        sender_uuid: string;
        item_uuid: string;
        wrapped_sik: string;
        ephemeral_public_key: string;
        encrypted_payload: string;
      }>;
    }>('GET', '/shares/inbox');
    return res.shares;
  }
  // ── Group sharing relay (sharing-pki.md §6) ───────────────────────────
  async createGroup(input: {
    name: string;
  }): Promise<{ group_id: string; name: string; admin_uuid: string }> {
    return this.http.request('POST', '/groups', input);
  }
  async listGroups(): Promise<Array<{ group_id: string; name: string; admin_uuid: string }>> {
    const res = await this.http.request<{
      groups: Array<{ group_id: string; name: string; admin_uuid: string }>;
    }>('GET', '/groups');
    return res.groups;
  }
  /** Fetch a group's member list with each member's wrapped Group SIK. */
  async groupInbox(): Promise<
    Array<{
      group_id: string;
      name: string;
      admin_uuid: string;
      member_uuid: string;
      wrapped_sik: string;
      ephemeral_public_key: string;
    }>
  > {
    const res = await this.http.request<{
      groups: Array<{
        group_id: string;
        name: string;
        admin_uuid: string;
        member_uuid: string;
        wrapped_sik: string;
        ephemeral_public_key: string;
      }>;
    }>('GET', '/groups/inbox');
    return res.groups;
  }
  async addGroupMember(
    groupId: string,
    input: { member_uuid: string; wrapped_sik: string; ephemeral_public_key: string },
  ): Promise<void> {
    await this.http.request(
      'POST',
      HttpClient.interpolate('/groups/{group_id}/members', { group_id: groupId }),
      input,
    );
  }
  async listGroupItems(groupId: string): Promise<Array<{ item_uuid: string; payload: string }>> {
    const res = await this.http.request<{ items: Array<{ item_uuid: string; payload: string }> }>(
      'GET',
      HttpClient.interpolate('/groups/{group_id}/items', { group_id: groupId }),
    );
    return res.items;
  }
  async addGroupItem(
    groupId: string,
    input: { item_uuid: string; payload: string },
  ): Promise<void> {
    await this.http.request(
      'POST',
      HttpClient.interpolate('/groups/{group_id}/items', { group_id: groupId }),
      input,
    );
  }
  async deleteGroupItem(groupId: string, itemUuid: string): Promise<void> {
    await this.http.request(
      'DELETE',
      HttpClient.interpolate('/groups/{group_id}/items/{item_uuid}', {
        group_id: groupId,
        item_uuid: itemUuid,
      }),
    );
  }
}
