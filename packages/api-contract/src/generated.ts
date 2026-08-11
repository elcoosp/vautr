// Auto-generated from openapi.json. Do not edit.
// Source: packages/api-contract/openapi.json (docs/architecture/api.md).
// Regenerate with: `pnpm --filter @vautr/api-contract generate`.

export type HttpMethod = "get" | "post" | "put" | "patch" | "delete";

// ---------------------------------------------------------------------------
// Component schemas
// ---------------------------------------------------------------------------
export interface ErrorEnvelope {
    error: ("payload_limit_exceeded" | "unauthorized" | "not_found" | "cursor_expired" | "precondition_failed" | "key_generation_too_old" | "rate_limited" | "internal_server_error");
    message?: string;
    context?: {
    [key: string]: unknown;
  };
  }

export interface ServerState {
    version: number;
    enc_key_gen: number;
  }

export interface ItemMetadata {
    uuid: string;
    version: number;
    enc_key_gen: number;
    deleted_date?: number | null;
  }

export interface PayloadResult {
    uuid: string;
    status: ("payload_delivered" | "version_mismatch");
    version: number;
    enc_key_gen: number;
    deleted_date?: number | null;
    payload?: string | null;
  }

export interface BatchItem {
    uuid: string;
    target_version: number;
    enc_key_gen: number;
    payload?: string | null;
    deleted_date?: number | null;
  }

export interface BatchResult {
    uuid: string;
    status?: ("success" | "conflict");
    version?: number;
    enc_key_gen?: number;
    updated_at?: number;
    current_server_state?: ServerState;
  }

export interface RecoveryChallenge {
    nonce: string;
  }

export interface RecoveryVerify {
    nonce: string;
    signature: string;
  }

export interface RecoveryVerifyResponse {
    session_token: string;
    expires_at: number;
  }

export interface RecoveryComplete {
    registration_finish: string;
    server_public_key: string;
    wrapped_svk_mp: string;
    wrapped_svk_rk: string;
    rk_public_key: string;
  }

export interface SharingPublicKey {
    uuid: string;
    public_key: string;
    created_at: number;
  }

export interface WrappedSIK {
    wrapped_sik: string;
    ephemeral_public_key: string;
  }

export interface ShareCreateRequest {
    sender_uuid: string;
    recipient_uuid: string;
    item_uuid: string;
    wrapped_sik: string;
    ephemeral_public_key: string;
  }

export interface ShareResult {
    share_id: string;
    status: string;
  }

export interface ShareInboxItem {
    share_id: string;
    sender_uuid: string;
    item_uuid: string;
    wrapped_sik: string;
    ephemeral_public_key: string;
    created_at: number;
  }

export interface GroupCreateRequest {
    name: string;
    description?: string;
  }

export interface GroupMember {
    user_uuid: string;
    role: ("admin" | "member");
  }

export interface Group {
    group_id: string;
    name: string;
    description?: string;
    members: Array<GroupMember>;
  }

export interface GroupResult {
    group_id: string;
    status: string;
  }

export interface GroupAddMemberRequest {
    user_uuid: string;
    wrapped_group_sik: string;
  }

export interface FileManifest {
    file_uuid: string;
    total_size: number;
    chunk_size: number;
    total_chunks: number;
    enc_key_gen: number;
    content_type: string;
    last_modified: number;
    status: ("pending_upload" | "available");
  }

export interface PresignedUrl {
    chunk_index: number;
    url: string;
  }

export interface UploadInitiateResponse {
    upload_id: string;
    presigned_urls: Array<PresignedUrl>;
  }

export interface UploadStatus {
    upload_id: string;
    received_chunks: Array<number>;
    total_chunks: number;
  }

export interface UploadCompleteRequest {
    upload_id: string;
    parts?: Array<string>;
  }

export interface DownloadResponse {
    presigned_urls: Array<PresignedUrl>;
  }

export interface StatusResponse {
    status: string;
  }

export type OrgRole = ("owner" | "admin" | "manager" | "member");

export type ProjectPermission = ("can_view" | "can_edit" | "can_manage");

export type ProjectType = ("personal" | "shared");

export interface ProjectCreateRequest {
    name: string;
    description?: string;
    type?: ProjectType;
  }

export interface ProjectUpdateRequest {
    name?: string;
    description?: string;
  }

export interface Project {
    uuid: string;
    name: string;
    description?: string | null;
    type: ProjectType;
    role: OrgRole;
    permission?: ProjectPermission;
    created_at: number;
    updated_at: number;
  }

export interface ProjectListResponse {
    projects: Array<Project>;
  }

export interface ProjectMember {
    user_uuid: string;
    display_name?: string | null;
    role: OrgRole;
    permission: ProjectPermission;
    added_at: number;
  }

export interface ProjectMemberListResponse {
    members: Array<ProjectMember>;
  }

export interface ProjectAddMemberRequest {
    user_uuid: string;
    role?: OrgRole;
    permission?: ProjectPermission;
  }

export interface ProjectUpdateMemberRequest {
    role?: OrgRole;
    permission?: ProjectPermission;
  }

export type UserGroupRole = ("admin" | "member");

export interface UserGroup {
    id: string;
    name: string;
    description?: string | null;
    members: Array<UserGroupMember>;
    created_at: number;
    updated_at: number;
  }

export interface UserGroupMember {
    user_uuid: string;
    role: UserGroupRole;
  }

export interface UserGroupCreateRequest {
    name: string;
    description?: string;
  }

export interface UserGroupUpdateRequest {
    name?: string;
    description?: string;
  }

export interface UserGroupListResponse {
    groups: Array<UserGroup>;
  }

export interface UserGroupAddMemberRequest {
    user_uuid: string;
    role?: UserGroupRole;
  }

export interface OffboardRequest {
    user_uuid: string;
    reason?: string;
  }

export interface OffboardResponse {
    status: string;
    user_uuid: string;
    revoked_projects: number;
    revoked_memberships: number;
    revoked_tokens: number;
    revoked_at: number;
  }

export type AccessScope = ("secrets:read" | "secrets:write" | "secrets:reveal" | "projects:read" | "projects:write" | "tokens:manage" | "machine_accounts:manage");

export type MachineAccountStatus = ("active" | "disabled" | "revoked");

export interface MachineAccount {
    uuid: string;
    name: string;
    description?: string | null;
    project_uuid?: string | null;
    status: MachineAccountStatus;
    scopes: Array<AccessScope>;
    expires_at?: number | null;
    last_used_at?: number | null;
    created_at: number;
  }

export interface MachineAccountCreateRequest {
    name: string;
    description?: string;
    project_uuid?: string;
    scopes: Array<AccessScope>;
    expires_at?: number;
  }

export interface MachineAccountUpdateRequest {
    name?: string;
    description?: string;
    status?: MachineAccountStatus;
    scopes?: Array<AccessScope>;
    expires_at?: number | null;
  }

export interface MachineAccountListResponse {
    machine_accounts: Array<MachineAccount>;
  }

export interface AccessToken {
    uuid: string;
    name: string;
    machine_account_uuid?: string | null;
    project_uuid?: string | null;
    scopes: Array<AccessScope>;
    prefix?: string;
    expires_at?: number | null;
    revoked_at?: number | null;
    last_used_at?: number | null;
    created_at: number;
  }

export interface AccessTokenCreateRequest {
    name: string;
    machine_account_uuid?: string;
    project_uuid?: string;
    scopes: Array<AccessScope>;
    expires_at?: number;
  }

export interface AccessTokenCreateResponse {
    token: string;
    token_id: string;
    expires_at?: number | null;
  }

export interface AccessTokenListResponse {
    tokens: Array<AccessToken>;
  }

export interface Secret {
    uuid: string;
    project_uuid: string;
    key: string;
    version: number;
    created_by: string;
    last_accessed_at?: number | null;
    created_at: number;
    updated_at: number;
  }

export interface SecretListResponse {
    secrets: Array<Secret>;
  }

export interface SecretCreateRequest {
    project_uuid: string;
    key: string;
    value_ciphertext: string;
  }

export interface SecretUpdateRequest {
    key?: string;
    value_ciphertext?: string;
  }

export interface SecretValue {
    uuid: string;
    key: string;
    value_ciphertext: string;
  }

export type MfaMethod = ("totp" | "webauthn" | "email");

export interface MfaStatus {
    required: boolean;
    configured_methods: Array<MfaMethod>;
  }

export interface TotpIssueResponse {
    enrollment_id: string;
    otpauth_url: string;
    secret: string;
    qr_code_data_url?: string;
  }

export interface TotpVerifyRequest {
    enrollment_id?: string;
    code: string;
  }

export interface TotpVerifyResponse {
    status: string;
    recovery_codes?: Array<string>;
  }

export interface WebAuthnEnrollStartRequest {
    display_name: string;
  }

export interface WebAuthnEnrollStartResponse {
    enrollment_id: string;
    options: Record<string, unknown>;
  }

export interface WebAuthnEnrollFinishRequest {
    enrollment_id: string;
    attestation: Record<string, unknown>;
  }

export interface MasterPasswordPolicy {
    min_length?: number;
    require_upper?: boolean;
    require_lower?: boolean;
    require_digit?: boolean;
    require_special?: boolean;
    min_entropy_bits?: number;
  }

export interface MfaPolicy {
    required: boolean;
    allowed_methods?: Array<MfaMethod>;
    master_password_policy?: MasterPasswordPolicy;
  }

export interface MfaPolicyUpdateRequest {
    required?: boolean;
    allowed_methods?: Array<MfaMethod>;
    master_password_policy?: MasterPasswordPolicy;
  }

export interface BackupStatus {
    enabled: boolean;
    location?: string;
    schedule?: string;
    last_backup_at?: number | null;
    last_backup_size_bytes?: number | null;
    last_restore_test_at?: number | null;
    last_restore_test_status?: ("passed" | "failed" | "pending" | "none") | null;
  }

export interface BackupExportRequest {
    include_secrets?: boolean;
  }

export interface BackupExportResponse {
    backup_id: string;
    download_url?: string | null;
    size_bytes?: number;
    checksum?: string;
    created_at: number;
  }

export interface BackupRestoreRequest {
    backup_id?: string;
    archive_base64?: string;
  }

export interface BackupRestoreResponse {
    status: string;
    test_id: string;
    restored_records?: number;
    restored_at: number;
  }

// ---------------------------------------------------------------------------
// Operation request / response types
// ---------------------------------------------------------------------------
export type authRegisterStartRequest = {
    username: string;
    registration_start: string;
  };
export type authRegisterStartResponse = {
    registration_response: string;
  };

export type authRegisterFinishRequest = {
    username: string;
    registration_finish: string;
    server_public_key: string;
  };
export type authRegisterFinishResponse = {
    status?: string;
  };

export type authLoginStartRequest = {
    username: string;
    login_start: string;
  };
export type authLoginStartResponse = {
    login_response: string;
  };

export type authLoginFinishRequest = {
    username: string;
    login_finish: string;
  };
export type authLoginFinishResponse = {
    session_token: string;
    expires_at: number;
  };

export type syncPullRequest = undefined;
export type syncPullResponse = {
    new_cursor: number;
    has_more: boolean;
    min_enc_key_gen: number;
    items: Array<ItemMetadata>;
  };

export type syncPullPayloadsRequest = {
    items: Array<{
    uuid: string;
    version: number;
  }>;
  };
export type syncPullPayloadsResponse = {
    results: Array<PayloadResult>;
  };

export type syncPushBatchRequest = {
    items: Array<BatchItem>;
  };
export type syncPushBatchResponse = {
    results: Array<BatchResult>;
  };

export type itemUpdateRequest = {
    enc_key_gen: number;
    payload: string;
  };
export type itemUpdateResponse = {
    uuid: string;
    version: number;
    enc_key_gen: number;
    updated_at: number;
  };

export type itemDeleteRequest = undefined;
export type itemDeleteResponse = {
    uuid: string;
    version: number;
    deleted_date: number;
  };

export type accountStatusRequest = undefined;
export type accountStatusResponse = {
    min_enc_key_gen: number;
    svk_ciphertext_blob: string;
  };

export type accountRotateKeyRequest = {
    new_min_enc_key_gen: number;
    new_svK_ciphertext_blob: string;
  };
export type accountRotateKeyResponse = {
    status: string;
    min_enc_key_gen: number;
  };

export type recoverChallengeRequest = undefined;
export type recoverChallengeResponse = RecoveryChallenge;

export type recoverVerifyRequest = RecoveryVerify;
export type recoverVerifyResponse = RecoveryVerifyResponse;

export type recoverCompleteRequest = RecoveryComplete;
export type recoverCompleteResponse = StatusResponse;

export type accountReclaimRequest = undefined;
export type accountReclaimResponse = StatusResponse;

export type accountDeleteRequest = undefined;
export type accountDeleteResponse = StatusResponse;

export type usersPublicKeyRequest = undefined;
export type usersPublicKeyResponse = SharingPublicKey;

export type shareCreateRequest = ShareCreateRequest;
export type shareCreateResponse = ShareResult;

export type shareUploadPayloadRequest = {
    payload: string;
  };
export type shareUploadPayloadResponse = StatusResponse;

export type shareInboxRequest = undefined;
export type shareInboxResponse = {
    shares: Array<ShareInboxItem>;
  };

export type shareRevokeRequest = undefined;
export type shareRevokeResponse = StatusResponse;

export type groupListRequest = undefined;
export type groupListResponse = {
    groups: Array<Group>;
  };

export type groupCreateRequest = GroupCreateRequest;
export type groupCreateResponse = GroupResult;

export type groupGetRequest = undefined;
export type groupGetResponse = Group;

export type groupDeleteRequest = undefined;
export type groupDeleteResponse = StatusResponse;

export type groupAddMemberRequest = GroupAddMemberRequest;
export type groupAddMemberResponse = StatusResponse;

export type groupAddItemRequest = {
    item_uuid: string;
    payload: string;
  };
export type groupAddItemResponse = StatusResponse;

export type fileUploadInitiateRequest = FileManifest;
export type fileUploadInitiateResponse = UploadInitiateResponse;

export type fileUploadStatusRequest = undefined;
export type fileUploadStatusResponse = UploadStatus;

export type fileUploadCompleteRequest = UploadCompleteRequest;
export type fileUploadCompleteResponse = StatusResponse;

export type fileDownloadRequest = undefined;
export type fileDownloadResponse = DownloadResponse;

export type projectListRequest = undefined;
export type projectListResponse = ProjectListResponse;

export type projectCreateRequest = ProjectCreateRequest;
export type projectCreateResponse = Project;

export type projectGetRequest = undefined;
export type projectGetResponse = Project;

export type projectUpdateRequest = ProjectUpdateRequest;
export type projectUpdateResponse = Project;

export type projectDeleteRequest = undefined;
export type projectDeleteResponse = StatusResponse;

export type projectMemberListRequest = undefined;
export type projectMemberListResponse = ProjectMemberListResponse;

export type projectMemberAddRequest = ProjectAddMemberRequest;
export type projectMemberAddResponse = ProjectMember;

export type projectMemberUpdateRequest = ProjectUpdateMemberRequest;
export type projectMemberUpdateResponse = ProjectMember;

export type projectMemberRemoveRequest = undefined;
export type projectMemberRemoveResponse = StatusResponse;

export type projectGroupListRequest = undefined;
export type projectGroupListResponse = UserGroupListResponse;

export type projectGroupCreateRequest = UserGroupCreateRequest;
export type projectGroupCreateResponse = UserGroup;

export type projectGroupUpdateRequest = UserGroupUpdateRequest;
export type projectGroupUpdateResponse = UserGroup;

export type projectGroupDeleteRequest = undefined;
export type projectGroupDeleteResponse = StatusResponse;

export type projectGroupAddMemberRequest = UserGroupAddMemberRequest;
export type projectGroupAddMemberResponse = StatusResponse;

export type projectGroupRemoveMemberRequest = undefined;
export type projectGroupRemoveMemberResponse = StatusResponse;

export type offboardUserRequest = OffboardRequest;
export type offboardUserResponse = OffboardResponse;

export type machineAccountListRequest = undefined;
export type machineAccountListResponse = MachineAccountListResponse;

export type machineAccountCreateRequest = MachineAccountCreateRequest;
export type machineAccountCreateResponse = MachineAccount;

export type machineAccountGetRequest = undefined;
export type machineAccountGetResponse = MachineAccount;

export type machineAccountUpdateRequest = MachineAccountUpdateRequest;
export type machineAccountUpdateResponse = MachineAccount;

export type machineAccountDeleteRequest = undefined;
export type machineAccountDeleteResponse = StatusResponse;

export type tokenListRequest = undefined;
export type tokenListResponse = AccessTokenListResponse;

export type tokenCreateRequest = AccessTokenCreateRequest;
export type tokenCreateResponse = AccessTokenCreateResponse;

export type tokenGetRequest = undefined;
export type tokenGetResponse = AccessToken;

export type tokenRevokeRequest = undefined;
export type tokenRevokeResponse = StatusResponse;

export type secretListRequest = undefined;
export type secretListResponse = SecretListResponse;

export type secretCreateRequest = SecretCreateRequest;
export type secretCreateResponse = Secret;

export type secretGetRequest = undefined;
export type secretGetResponse = Secret;

export type secretUpdateRequest = SecretUpdateRequest;
export type secretUpdateResponse = Secret;

export type secretDeleteRequest = undefined;
export type secretDeleteResponse = StatusResponse;

export type secretGetValueRequest = undefined;
export type secretGetValueResponse = SecretValue;

export type mfaStatusRequest = undefined;
export type mfaStatusResponse = MfaStatus;

export type mfaTotpIssueRequest = undefined;
export type mfaTotpIssueResponse = TotpIssueResponse;

export type mfaTotpVerifyRequest = TotpVerifyRequest;
export type mfaTotpVerifyResponse = TotpVerifyResponse;

export type mfaWebAuthnEnrollStartRequest = WebAuthnEnrollStartRequest;
export type mfaWebAuthnEnrollStartResponse = WebAuthnEnrollStartResponse;

export type mfaWebAuthnEnrollFinishRequest = WebAuthnEnrollFinishRequest;
export type mfaWebAuthnEnrollFinishResponse = StatusResponse;

export type mfaPolicyGetRequest = undefined;
export type mfaPolicyGetResponse = MfaPolicy;

export type mfaPolicyUpdateRequest = MfaPolicyUpdateRequest;
export type mfaPolicyUpdateResponse = MfaPolicy;

export type backupStatusRequest = undefined;
export type backupStatusResponse = BackupStatus;

export type backupExportRequest = BackupExportRequest;
export type backupExportResponse = BackupExportResponse;

export type backupRestoreRequest = BackupRestoreRequest;
export type backupRestoreResponse = BackupRestoreResponse;

// ---------------------------------------------------------------------------
// Typed path/method map for building HTTP clients
// ---------------------------------------------------------------------------
export type ApiPath__auth_register_start = {
    post: {
      pathParams?: undefined;
      request: authRegisterStartRequest;
      response: authRegisterStartResponse;
    };
};

export type ApiPath__auth_register_finish = {
    post: {
      pathParams?: undefined;
      request: authRegisterFinishRequest;
      response: authRegisterFinishResponse;
    };
};

export type ApiPath__auth_login_start = {
    post: {
      pathParams?: undefined;
      request: authLoginStartRequest;
      response: authLoginStartResponse;
    };
};

export type ApiPath__auth_login_finish = {
    post: {
      pathParams?: undefined;
      request: authLoginFinishRequest;
      response: authLoginFinishResponse;
    };
};

export type ApiPath__sync_pull = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: syncPullResponse;
    };
};

export type ApiPath__sync_pull_payloads = {
    post: {
      pathParams?: undefined;
      request: syncPullPayloadsRequest;
      response: syncPullPayloadsResponse;
    };
};

export type ApiPath__sync_push_batch = {
    post: {
      pathParams?: undefined;
      request: syncPushBatchRequest;
      response: syncPushBatchResponse;
    };
};

export type ApiPath__items__uuid_ = {
    put: {
      pathParams?: undefined;
      request: itemUpdateRequest;
      response: itemUpdateResponse;
    };
    delete: {
      pathParams?: undefined;
      request?: undefined;
      response: itemDeleteResponse;
    };
};

export type ApiPath__account_status = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: accountStatusResponse;
    };
};

export type ApiPath__account_rotate_key = {
    post: {
      pathParams?: undefined;
      request: accountRotateKeyRequest;
      response: accountRotateKeyResponse;
    };
};

export type ApiPath__account_recover_challenge = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: recoverChallengeResponse;
    };
};

export type ApiPath__account_recover_verify = {
    post: {
      pathParams?: undefined;
      request: recoverVerifyRequest;
      response: recoverVerifyResponse;
    };
};

export type ApiPath__account_recover_complete = {
    post: {
      pathParams?: undefined;
      request: recoverCompleteRequest;
      response: recoverCompleteResponse;
    };
};

export type ApiPath__account_reclaim = {
    post: {
      pathParams?: undefined;
      request?: undefined;
      response: accountReclaimResponse;
    };
};

export type ApiPath__account = {
    delete: {
      pathParams?: undefined;
      request?: undefined;
      response: accountDeleteResponse;
    };
};

export type ApiPath__users__uuid__public_key = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: usersPublicKeyResponse;
    };
};

export type ApiPath__shares_ = {
    post: {
      pathParams?: undefined;
      request: shareCreateRequest;
      response: shareCreateResponse;
    };
};

export type ApiPath__shares__share_id__payload = {
    post: {
      pathParams?: undefined;
      request: shareUploadPayloadRequest;
      response: shareUploadPayloadResponse;
    };
};

export type ApiPath__shares_inbox = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: shareInboxResponse;
    };
};

export type ApiPath__shares__share_id_ = {
    delete: {
      pathParams?: undefined;
      request?: undefined;
      response: shareRevokeResponse;
    };
};

export type ApiPath__groups = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: groupListResponse;
    };
    post: {
      pathParams?: undefined;
      request: groupCreateRequest;
      response: groupCreateResponse;
    };
};

export type ApiPath__groups__id_ = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: groupGetResponse;
    };
    delete: {
      pathParams?: undefined;
      request?: undefined;
      response: groupDeleteResponse;
    };
};

export type ApiPath__groups__id__members = {
    post: {
      pathParams?: undefined;
      request: groupAddMemberRequest;
      response: groupAddMemberResponse;
    };
};

export type ApiPath__groups__id__items = {
    post: {
      pathParams?: undefined;
      request: groupAddItemRequest;
      response: groupAddItemResponse;
    };
};

export type ApiPath__files__file_uuid__upload_initiate = {
    post: {
      pathParams?: undefined;
      request: fileUploadInitiateRequest;
      response: fileUploadInitiateResponse;
    };
};

export type ApiPath__files__file_uuid__upload_status = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: fileUploadStatusResponse;
    };
};

export type ApiPath__files__file_uuid__upload_complete = {
    post: {
      pathParams?: undefined;
      request: fileUploadCompleteRequest;
      response: fileUploadCompleteResponse;
    };
};

export type ApiPath__files__file_uuid__download = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: fileDownloadResponse;
    };
};

export type ApiPath__projects = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: projectListResponse;
    };
    post: {
      pathParams?: undefined;
      request: projectCreateRequest;
      response: projectCreateResponse;
    };
};

export type ApiPath__projects__uuid_ = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: projectGetResponse;
    };
    patch: {
      pathParams?: undefined;
      request: projectUpdateRequest;
      response: projectUpdateResponse;
    };
    delete: {
      pathParams?: undefined;
      request?: undefined;
      response: projectDeleteResponse;
    };
};

export type ApiPath__projects__uuid__members = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: projectMemberListResponse;
    };
    post: {
      pathParams?: undefined;
      request: projectMemberAddRequest;
      response: projectMemberAddResponse;
    };
};

export type ApiPath__projects__uuid__members__user_uuid_ = {
    patch: {
      pathParams?: undefined;
      request: projectMemberUpdateRequest;
      response: projectMemberUpdateResponse;
    };
    delete: {
      pathParams?: undefined;
      request?: undefined;
      response: projectMemberRemoveResponse;
    };
};

export type ApiPath__projects__uuid__groups = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: projectGroupListResponse;
    };
    post: {
      pathParams?: undefined;
      request: projectGroupCreateRequest;
      response: projectGroupCreateResponse;
    };
};

export type ApiPath__projects__uuid__groups__group_id_ = {
    patch: {
      pathParams?: undefined;
      request: projectGroupUpdateRequest;
      response: projectGroupUpdateResponse;
    };
    delete: {
      pathParams?: undefined;
      request?: undefined;
      response: projectGroupDeleteResponse;
    };
};

export type ApiPath__projects__uuid__groups__group_id__members = {
    post: {
      pathParams?: undefined;
      request: projectGroupAddMemberRequest;
      response: projectGroupAddMemberResponse;
    };
};

export type ApiPath__projects__uuid__groups__group_id__members__user_uuid_ = {
    delete: {
      pathParams?: undefined;
      request?: undefined;
      response: projectGroupRemoveMemberResponse;
    };
};

export type ApiPath__offboard = {
    post: {
      pathParams?: undefined;
      request: offboardUserRequest;
      response: offboardUserResponse;
    };
};

export type ApiPath__machine_accounts = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: machineAccountListResponse;
    };
    post: {
      pathParams?: undefined;
      request: machineAccountCreateRequest;
      response: machineAccountCreateResponse;
    };
};

export type ApiPath__machine_accounts__uuid_ = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: machineAccountGetResponse;
    };
    patch: {
      pathParams?: undefined;
      request: machineAccountUpdateRequest;
      response: machineAccountUpdateResponse;
    };
    delete: {
      pathParams?: undefined;
      request?: undefined;
      response: machineAccountDeleteResponse;
    };
};

export type ApiPath__tokens = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: tokenListResponse;
    };
    post: {
      pathParams?: undefined;
      request: tokenCreateRequest;
      response: tokenCreateResponse;
    };
};

export type ApiPath__tokens__uuid_ = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: tokenGetResponse;
    };
    delete: {
      pathParams?: undefined;
      request?: undefined;
      response: tokenRevokeResponse;
    };
};

export type ApiPath__projects__uuid__secrets = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: secretListResponse;
    };
};

export type ApiPath__secrets = {
    post: {
      pathParams?: undefined;
      request: secretCreateRequest;
      response: secretCreateResponse;
    };
};

export type ApiPath__secrets__uuid_ = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: secretGetResponse;
    };
    patch: {
      pathParams?: undefined;
      request: secretUpdateRequest;
      response: secretUpdateResponse;
    };
    delete: {
      pathParams?: undefined;
      request?: undefined;
      response: secretDeleteResponse;
    };
};

export type ApiPath__secrets__uuid__value = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: secretGetValueResponse;
    };
};

export type ApiPath__mfa_status = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: mfaStatusResponse;
    };
};

export type ApiPath__mfa_totp_issue = {
    post: {
      pathParams?: undefined;
      request?: undefined;
      response: mfaTotpIssueResponse;
    };
};

export type ApiPath__mfa_totp_verify = {
    post: {
      pathParams?: undefined;
      request: mfaTotpVerifyRequest;
      response: mfaTotpVerifyResponse;
    };
};

export type ApiPath__mfa_webauthn_enroll_start = {
    post: {
      pathParams?: undefined;
      request: mfaWebAuthnEnrollStartRequest;
      response: mfaWebAuthnEnrollStartResponse;
    };
};

export type ApiPath__mfa_webauthn_enroll_finish = {
    post: {
      pathParams?: undefined;
      request: mfaWebAuthnEnrollFinishRequest;
      response: mfaWebAuthnEnrollFinishResponse;
    };
};

export type ApiPath__mfa_policy = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: mfaPolicyGetResponse;
    };
    put: {
      pathParams?: undefined;
      request: mfaPolicyUpdateRequest;
      response: mfaPolicyUpdateResponse;
    };
};

export type ApiPath__backup = {
    get: {
      pathParams?: undefined;
      request?: undefined;
      response: backupStatusResponse;
    };
};

export type ApiPath__backup_export = {
    post: {
      pathParams?: undefined;
      request: backupExportRequest;
      response: backupExportResponse;
    };
};

export type ApiPath__backup_restore = {
    post: {
      pathParams?: undefined;
      request: backupRestoreRequest;
      response: backupRestoreResponse;
    };
};

export type ApiPaths = {
  "/auth/register/start": ApiPath__auth_register_start;
  "/auth/register/finish": ApiPath__auth_register_finish;
  "/auth/login/start": ApiPath__auth_login_start;
  "/auth/login/finish": ApiPath__auth_login_finish;
  "/sync/pull": ApiPath__sync_pull;
  "/sync/pull-payloads": ApiPath__sync_pull_payloads;
  "/sync/push-batch": ApiPath__sync_push_batch;
  "/items/{uuid}": ApiPath__items__uuid_;
  "/account/status": ApiPath__account_status;
  "/account/rotate-key": ApiPath__account_rotate_key;
  "/account/recover/challenge": ApiPath__account_recover_challenge;
  "/account/recover/verify": ApiPath__account_recover_verify;
  "/account/recover/complete": ApiPath__account_recover_complete;
  "/account/reclaim": ApiPath__account_reclaim;
  "/account": ApiPath__account;
  "/users/{uuid}/public-key": ApiPath__users__uuid__public_key;
  "/shares/": ApiPath__shares_;
  "/shares/{share_id}/payload": ApiPath__shares__share_id__payload;
  "/shares/inbox": ApiPath__shares_inbox;
  "/shares/{share_id}": ApiPath__shares__share_id_;
  "/groups": ApiPath__groups;
  "/groups/{id}": ApiPath__groups__id_;
  "/groups/{id}/members": ApiPath__groups__id__members;
  "/groups/{id}/items": ApiPath__groups__id__items;
  "/files/{file_uuid}/upload/initiate": ApiPath__files__file_uuid__upload_initiate;
  "/files/{file_uuid}/upload/status": ApiPath__files__file_uuid__upload_status;
  "/files/{file_uuid}/upload/complete": ApiPath__files__file_uuid__upload_complete;
  "/files/{file_uuid}/download": ApiPath__files__file_uuid__download;
  "/projects": ApiPath__projects;
  "/projects/{uuid}": ApiPath__projects__uuid_;
  "/projects/{uuid}/members": ApiPath__projects__uuid__members;
  "/projects/{uuid}/members/{user_uuid}": ApiPath__projects__uuid__members__user_uuid_;
  "/projects/{uuid}/groups": ApiPath__projects__uuid__groups;
  "/projects/{uuid}/groups/{group_id}": ApiPath__projects__uuid__groups__group_id_;
  "/projects/{uuid}/groups/{group_id}/members": ApiPath__projects__uuid__groups__group_id__members;
  "/projects/{uuid}/groups/{group_id}/members/{user_uuid}": ApiPath__projects__uuid__groups__group_id__members__user_uuid_;
  "/offboard": ApiPath__offboard;
  "/machine-accounts": ApiPath__machine_accounts;
  "/machine-accounts/{uuid}": ApiPath__machine_accounts__uuid_;
  "/tokens": ApiPath__tokens;
  "/tokens/{uuid}": ApiPath__tokens__uuid_;
  "/projects/{uuid}/secrets": ApiPath__projects__uuid__secrets;
  "/secrets": ApiPath__secrets;
  "/secrets/{uuid}": ApiPath__secrets__uuid_;
  "/secrets/{uuid}/value": ApiPath__secrets__uuid__value;
  "/mfa/status": ApiPath__mfa_status;
  "/mfa/totp/issue": ApiPath__mfa_totp_issue;
  "/mfa/totp/verify": ApiPath__mfa_totp_verify;
  "/mfa/webauthn/enroll/start": ApiPath__mfa_webauthn_enroll_start;
  "/mfa/webauthn/enroll/finish": ApiPath__mfa_webauthn_enroll_finish;
  "/mfa/policy": ApiPath__mfa_policy;
  "/backup": ApiPath__backup;
  "/backup/export": ApiPath__backup_export;
  "/backup/restore": ApiPath__backup_restore;
};
