/**
 * Typed HTTP client for the Vautr MLP endpoints (projects, members, groups,
 * secrets, machine accounts, tokens, MFA, backup). Driven against the live
 * server using the frozen OpenAPI contract types in `@vautr/api-contract`.
 *
 * Auth: every request carries the bearer session token minted by the OPAQUE
 * login (api.md §3). The token is persisted to IndexedDB by the shared client
 * SDK (`IndexedDbStore.setState`), so we read it from the same DB to avoid
 * reaching into the SDK's private ApiClient.
 */
import type {
  AccessToken,
  AccessTokenCreateRequest,
  AccessTokenCreateResponse,
  AccessTokenListResponse,
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
  OffboardResponse,
  Project,
  ProjectCreateRequest,
  ProjectListResponse,
  ProjectMember,
  ProjectMemberListResponse,
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
  UserGroup,
  UserGroupListResponse,
} from '@vautr/api-contract';
import { IndexedDbStore } from '@vautr/client-sdk/storage';

export const API_BASE = 'http://localhost:8080';

/** HTTP error with the server's error code + status. */
export class MlpApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly context?: unknown;

  constructor(status: number, code: string, message: string, context?: unknown) {
    super(message);
    this.name = 'MlpApiError';
    this.status = status;
    this.code = code;
    this.context = context;
  }
}

interface ErrorEnvelope {
  error?: string;
  message?: string;
  context?: unknown;
}

const stateStore = new IndexedDbStore();

/** Current bearer token, read from the shared IndexedDB session state. */
async function sessionToken(): Promise<string | null> {
  const state = await stateStore.getState();
  return state.sessionToken;
}

async function request<T>(
  method: string,
  path: string,
  body?: unknown,
): Promise<T> {
  const token = await sessionToken();
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    'X-Request-ID': crypto.randomUUID(),
  };
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }
  const res = await fetch(`${API_BASE}${path}`, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await res.text();
  let data: unknown = {};
  if (text) {
    try {
      data = JSON.parse(text);
    } catch {
      data = { message: text };
    }
  }
  if (!res.ok) {
    const err = data as ErrorEnvelope;
    throw new MlpApiError(res.status, err.error ?? 'http_error', err.message ?? `HTTP ${res.status}`, err.context);
  }
  return data as T;
}

function encode(value: string): string {
  return encodeURIComponent(value);
}

// ---------------------------------------------------------------------------
// Projects
// ---------------------------------------------------------------------------

export const mlp = {
  listProjects: () => request<ProjectListResponse>('GET', '/projects'),
  createProject: (req: ProjectCreateRequest) => request<Project>('POST', '/projects', req),
  getProject: (uuid: string) => request<Project>('GET', `/projects/${encode(uuid)}`),
  updateProject: (uuid: string, req: ProjectUpdateRequest) =>
    request<Project>('PATCH', `/projects/${encode(uuid)}`, req),
  deleteProject: (uuid: string) => request<StatusResponse>('DELETE', `/projects/${encode(uuid)}`),

  listMembers: (uuid: string) =>
    request<ProjectMemberListResponse>('GET', `/projects/${encode(uuid)}/members`),
  addMember: (
    uuid: string,
    req: { user_uuid: string; role?: string; permission?: string; hide_password?: boolean },
  ) => request<ProjectMember>('POST', `/projects/${encode(uuid)}/members`, req),
  updateMember: (
    uuid: string,
    userUuid: string,
    req: { role?: string; permission?: string; hide_password?: boolean },
  ) => request<ProjectMember>('PATCH', `/projects/${encode(uuid)}/members/${encode(userUuid)}`, req),
  removeMember: (uuid: string, userUuid: string) =>
    request<StatusResponse>('DELETE', `/projects/${encode(uuid)}/members/${encode(userUuid)}`),

  listGroups: (uuid: string) =>
    request<UserGroupListResponse>('GET', `/projects/${encode(uuid)}/groups`),
  createGroup: (uuid: string, req: { name: string; description?: string }) =>
    request<UserGroup>('POST', `/projects/${encode(uuid)}/groups`, req),
  updateGroup: (uuid: string, groupId: string, req: { name?: string; description?: string }) =>
    request<UserGroup>('PATCH', `/projects/${encode(uuid)}/groups/${encode(groupId)}`, req),
  deleteGroup: (uuid: string, groupId: string) =>
    request<StatusResponse>('DELETE', `/projects/${encode(uuid)}/groups/${encode(groupId)}`),
  addGroupMember: (uuid: string, groupId: string, req: { user_uuid: string; role?: string }) =>
    request<StatusResponse>('POST', `/projects/${encode(uuid)}/groups/${encode(groupId)}/members`, req),
  removeGroupMember: (uuid: string, groupId: string, userUuid: string) =>
    request<StatusResponse>(
      'DELETE',
      `/projects/${encode(uuid)}/groups/${encode(groupId)}/members/${encode(userUuid)}`,
    ),

  offboard: (req: { user_uuid: string; reason?: string }) =>
    request<OffboardResponse>('POST', '/offboard', req),

  // -------------------------------------------------------------------------
  // Secrets
  // -------------------------------------------------------------------------
  listSecrets: (projectUuid: string) =>
    request<SecretListResponse>('GET', `/projects/${encode(projectUuid)}/secrets`),
  createSecret: (req: SecretCreateRequest) => request<Secret>('POST', '/secrets', req),
  getSecret: (uuid: string) => request<Secret>('GET', `/secrets/${encode(uuid)}`),
  updateSecret: (uuid: string, req: SecretUpdateRequest) =>
    request<Secret>('PATCH', `/secrets/${encode(uuid)}`, req),
  deleteSecret: (uuid: string) => request<StatusResponse>('DELETE', `/secrets/${encode(uuid)}`),
  revealSecret: (uuid: string) => request<SecretValue>('GET', `/secrets/${encode(uuid)}/value`),

  // -------------------------------------------------------------------------
  // MFA
  // -------------------------------------------------------------------------
  mfaStatus: () => request<MfaStatus>('GET', '/mfa/status'),
  mfaTotpIssue: () => request<TotpIssueResponse>('POST', '/mfa/totp/issue'),
  mfaTotpVerify: (req: TotpVerifyRequest) => request<TotpVerifyResponse>('POST', '/mfa/totp/verify', req),
  mfaPolicyGet: () => request<MfaPolicy>('GET', '/mfa/policy'),
  mfaPolicyUpdate: (req: MfaPolicyUpdateRequest) => request<MfaPolicy>('PUT', '/mfa/policy', req),

  // -------------------------------------------------------------------------
  // Machine accounts
  // -------------------------------------------------------------------------
  listMachineAccounts: () => request<MachineAccountListResponse>('GET', '/machine-accounts'),
  createMachineAccount: (req: MachineAccountCreateRequest) =>
    request<MachineAccount>('POST', '/machine-accounts', req),
  getMachineAccount: (uuid: string) =>
    request<MachineAccount>('GET', `/machine-accounts/${encode(uuid)}`),
  updateMachineAccount: (uuid: string, req: MachineAccountUpdateRequest) =>
    request<MachineAccount>('PATCH', `/machine-accounts/${encode(uuid)}`, req),
  deleteMachineAccount: (uuid: string) =>
    request<StatusResponse>('DELETE', `/machine-accounts/${encode(uuid)}`),

  // -------------------------------------------------------------------------
  // Access tokens
  // -------------------------------------------------------------------------
  listTokens: () => request<AccessTokenListResponse>('GET', '/tokens'),
  createToken: (req: AccessTokenCreateRequest) => request<AccessTokenCreateResponse>('POST', '/tokens', req),
  getToken: (uuid: string) => request<AccessToken>('GET', `/tokens/${encode(uuid)}`),
  revokeToken: (uuid: string) => request<StatusResponse>('DELETE', `/tokens/${encode(uuid)}`),

  // -------------------------------------------------------------------------
  // Backup / export / import
  // -------------------------------------------------------------------------
  backupStatus: () => request<BackupStatus>('GET', '/backup'),
  backupExport: (req: { include_secrets?: boolean }) =>
    request<BackupExportResponse>('POST', '/backup/export', req),
  backupRestore: (req: { backup_id?: string; archive_base64?: string }) =>
    request<BackupRestoreResponse>('POST', '/backup/restore', req),
};

/** Low-level escape hatch for the E2E test harness. */
export { request as mlpRawRequest };
