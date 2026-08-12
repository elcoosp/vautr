/**
 * Typed MLP v1 API client for the mobile app, built on the frozen generated
 * contract (`@vautr/api-contract`). Read-only use of the contract types; the
 * transport lives in `./http`. All methods are typed to the server schemas.
 */

import type {
  AccessScope,
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
  accountStatusResponse,
} from '@vautr/api-contract';

import { HttpClient } from './http';

export type {
  AccessScope,
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
  accountStatusResponse,
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

  // ── Machine accounts & tokens (Wave A2) ───────────────────────────────
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
}
