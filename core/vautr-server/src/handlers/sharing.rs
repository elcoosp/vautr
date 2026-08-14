//! Sharing PKI & group HTTP handlers (docs/architecture/sharing-pki.md §5-6).
//!
//! The server is an untrusted relay: it stores public keys and KEM envelopes
//! (`wrapped_sik` + `ephemeral_public_key`) but never the SIK or plaintext.
//! All share payloads are ciphertext.
//!
//! # Identity of `share_id`
//! The schema's `shares` table is keyed by the item's natural key
//! `(item_uuid, recipient_user_id)`; there is no dedicated `share_id` column
//! (migrations are frozen by the shared-tree contract). This module therefore
//! treats the path parameter `share_id` as the `item_uuid`.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{auth_user, b64, decode_b64, now_ms, ApiError, AppState, Bearer};

/// Build this feature's router. Merged into the main router in mod.rs.
pub fn routes() -> Router<AppState> {
    Router::new()
        // Directory (§2.1)
        .route("/users/{user_id}/public-key", get(public_key))
        .route("/users/{user_id}/public-key", put(put_public_key))
        // 1:1 shares (§5)
        .route("/shares/", post(create_share))
        .route("/shares/{share_id}/payload", post(upload_payload))
        .route("/shares/inbox", get(inbox))
        .route("/shares/{share_id}", delete(revoke_share))
        // Groups (§6)
        .route("/groups/", post(create_group))
        .route("/groups/{group_id}/members", post(add_group_member))
        .route(
            "/groups/{group_id}/members/{member_uuid}",
            delete(remove_group_member),
        )
        .route("/groups/{group_id}/rotate", post(rotate_group))
        .route("/groups/inbox", get(group_inbox))
        // Group items (§6)
        .route("/groups/{group_id}/items", post(add_group_item))
        .route("/groups/{group_id}/items", get(list_group_items))
        .route(
            "/groups/{group_id}/items/{item_uuid}",
            delete(delete_group_item),
        )
}

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub(crate) struct PublicKeyResp {
    user_id: String,
    public_key: String, // base64 (32-byte X25519 key)
}

#[derive(Deserialize)]
pub(crate) struct PutPublicKeyReq {
    public_key: String, // base64 (32-byte X25519 key)
}

#[derive(Deserialize)]
pub(crate) struct CreateShareReq {
    item_uuid: String,
    recipient_uuid: String,
    wrapped_sik: String,          // base64
    ephemeral_public_key: String, // base64
}

#[derive(Serialize)]
pub(crate) struct ShareInfoResp {
    share_id: String,
    sender_uuid: String,
    recipient_uuid: String,
    item_uuid: String,
}

#[derive(Deserialize)]
pub(crate) struct UploadPayloadReq {
    payload: String, // base64 (DEM ciphertext)
}

#[derive(Deserialize)]
pub(crate) struct CreateGroupReq {
    name: String,
}

#[derive(Serialize)]
pub(crate) struct GroupResp {
    group_id: String,
    name: String,
    admin_uuid: String,
}

#[derive(Deserialize)]
pub(crate) struct AddMemberReq {
    member_uuid: String,
    wrapped_sik: String,          // base64
    ephemeral_public_key: String, // base64
}

#[derive(Deserialize)]
pub(crate) struct WrappedKeyJson {
    recipient_user_id: String,
    wrapped_sik: String,          // base64
    ephemeral_public_key: String, // base64
}

#[derive(Deserialize)]
pub(crate) struct RotateGroupReq {
    wrapped_keys: Vec<WrappedKeyJson>,
}

#[derive(Serialize)]
pub(crate) struct InboxItem {
    share_id: String,
    sender_uuid: String,
    item_uuid: String,
    wrapped_sik: String,          // base64
    ephemeral_public_key: String, // base64
    payload: Option<String>,      // base64 (may be absent)
}

#[derive(Serialize)]
pub(crate) struct GroupInboxItem {
    group_id: String,
    name: String,
    admin_uuid: String,
    wrapped_sik: Option<String>,
    ephemeral_public_key: Option<String>,
}

// ---------------------------------------------------------------------------
// Directory
// ---------------------------------------------------------------------------

/// GET /users/{uuid}/public-key
async fn public_key(
    State(st): State<AppState>,
    Path(user_id): Path<String>,
    auth: Bearer,
) -> Result<Json<PublicKeyResp>, ApiError> {
    let _caller = auth_user(&st.repo, &auth.0).await?;
    let uid = user_id.clone();
    let Some(pk) = st
        .repo
        .get_sharing_public_key(&uid)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "user has no sharing public key",
        ));
    };
    Ok(Json(PublicKeyResp {
        user_id: uid,
        public_key: b64(&pk),
    }))
}

/// PUT /users/{uuid}/public-key
///
/// Publish/replace the caller's own sharing public key (the sender looks it up
/// later via `GET`). A user may only write their own key (path `{uuid}` must
/// match the authenticated identity).
async fn put_public_key(
    State(st): State<AppState>,
    Path(user_id): Path<String>,
    auth: Bearer,
    Json(req): Json<PutPublicKeyReq>,
) -> Result<Json<PublicKeyResp>, ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    if caller != user_id {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "you may only set your own sharing public key",
        ));
    }
    let pk = decode_b64(&req.public_key)?;
    if pk.len() != 32 {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "sharing public key must be 32 bytes",
        ));
    }
    st.repo
        .upsert_sharing_public_key(&caller, &pk, now_ms())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(PublicKeyResp {
        user_id: caller,
        public_key: b64(&pk),
    }))
}

// ---------------------------------------------------------------------------
// 1:1 shares
// ---------------------------------------------------------------------------

/// POST /shares/
async fn create_share(
    State(st): State<AppState>,
    auth: Bearer,
    Json(req): Json<CreateShareReq>,
) -> Result<Json<ShareInfoResp>, ApiError> {
    let sender = auth_user(&st.repo, &auth.0).await?;
    let wrapped_sik = decode_b64(&req.wrapped_sik)?;
    let ephemeral_pk = decode_b64(&req.ephemeral_public_key)?;
    st.repo
        .create_share(
            &req.item_uuid,
            &sender,
            &req.recipient_uuid,
            &wrapped_sik,
            &ephemeral_pk,
            now_ms(),
        )
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(ShareInfoResp {
        share_id: req.item_uuid.clone(),
        sender_uuid: sender,
        recipient_uuid: req.recipient_uuid,
        item_uuid: req.item_uuid,
    }))
}

/// POST /shares/{share_id}/payload
async fn upload_payload(
    State(st): State<AppState>,
    Path(share_id): Path<Uuid>,
    auth: Bearer,
    Json(req): Json<UploadPayloadReq>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let owner = auth_user(&st.repo, &auth.0).await?;
    let sid = share_id.to_string();
    let owned = st
        .repo
        .is_share_owner(&sid, &owner)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    if !owned {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "share not found or forbidden",
        ));
    }
    let payload = decode_b64(&req.payload)?;
    st.repo
        .store_share_payload(&sid, &payload, now_ms())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(serde_json::json!({ "share_id": sid, "status": "ok" })))
}

/// GET /shares/inbox
async fn inbox(State(st): State<AppState>, auth: Bearer) -> Result<Json<Vec<InboxItem>>, ApiError> {
    let recipient = auth_user(&st.repo, &auth.0).await?;
    let shares = st
        .repo
        .get_inbox(&recipient)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    let mut items = Vec::with_capacity(shares.len());
    for s in shares {
        let payload = st
            .repo
            .get_share_payload(&s.item_uuid)
            .await
            .map_err(|e| ApiError::internal(&e.to_string()))?;
        items.push(InboxItem {
            share_id: s.item_uuid.clone(),
            sender_uuid: s.owner_user_id,
            item_uuid: s.item_uuid,
            wrapped_sik: b64(&s.wrapped_sik),
            ephemeral_public_key: b64(&s.ephemeral_public_key),
            payload: payload.map(|p| b64(&p)),
        });
    }
    Ok(Json(items))
}

/// DELETE /shares/{share_id}
async fn revoke_share(
    State(st): State<AppState>,
    Path(share_id): Path<Uuid>,
    auth: Bearer,
) -> Result<Json<serde_json::Value>, ApiError> {
    let owner = auth_user(&st.repo, &auth.0).await?;
    let sid = share_id.to_string();
    let owned = st
        .repo
        .is_share_owner(&sid, &owner)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    if !owned {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "share not found or forbidden",
        ));
    }
    st.repo
        .delete_shares_by_owner_item(&sid, &owner)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    st.repo
        .delete_share_payload(&sid)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(
        serde_json::json!({ "share_id": sid, "status": "revoked" }),
    ))
}

// ---------------------------------------------------------------------------
// Groups
// ---------------------------------------------------------------------------

/// POST /groups/
async fn create_group(
    State(st): State<AppState>,
    auth: Bearer,
    Json(req): Json<CreateGroupReq>,
) -> Result<Json<GroupResp>, ApiError> {
    let admin = auth_user(&st.repo, &auth.0).await?;
    let group_id = Uuid::new_v4();
    let gid = group_id.to_string();
    st.repo
        .create_group(&gid, &req.name, &admin, now_ms())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    // The admin is implicitly the first member.
    st.repo
        .add_group_member(&gid, &admin, now_ms())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(GroupResp {
        group_id: gid,
        name: req.name,
        admin_uuid: admin,
    }))
}

/// POST /groups/{group_id}/members
async fn add_group_member(
    State(st): State<AppState>,
    Path(group_id): Path<Uuid>,
    auth: Bearer,
    Json(req): Json<AddMemberReq>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    let gid = group_id.to_string();
    require_admin(&st, &gid, &caller).await?;
    let wrapped_sik = decode_b64(&req.wrapped_sik)?;
    let ephemeral_pk = decode_b64(&req.ephemeral_public_key)?;
    st.repo
        .add_group_member(&gid, &req.member_uuid, now_ms())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    st.repo
        .store_group_wrapped_sik(&gid, &req.member_uuid, &wrapped_sik, &ephemeral_pk)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(
        serde_json::json!({ "group_id": gid, "status": "member_added" }),
    ))
}

/// DELETE /groups/{group_id}/members/{member_uuid}
async fn remove_group_member(
    State(st): State<AppState>,
    Path((group_id, member_uuid)): Path<(Uuid, String)>,
    auth: Bearer,
) -> Result<Json<serde_json::Value>, ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    let gid = group_id.to_string();
    require_admin(&st, &gid, &caller).await?;
    let muid = member_uuid;
    st.repo
        .remove_group_member(&gid, &muid)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    st.repo
        .delete_group_wrapped_sik(&gid, &muid)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(
        serde_json::json!({ "group_id": gid, "status": "member_removed" }),
    ))
}

/// POST /groups/{group_id}/rotate
///
/// Replaces every wrapped Group SIK after the admin rotated it (§6.3).
async fn rotate_group(
    State(st): State<AppState>,
    Path(group_id): Path<Uuid>,
    auth: Bearer,
    Json(req): Json<RotateGroupReq>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    let gid = group_id.to_string();
    require_admin(&st, &gid, &caller).await?;
    let mut wrapped = Vec::with_capacity(req.wrapped_keys.len());
    for k in req.wrapped_keys {
        let ws = decode_b64(&k.wrapped_sik)?;
        let epk = decode_b64(&k.ephemeral_public_key)?;
        wrapped.push((k.recipient_user_id, ws, epk));
    }
    st.repo
        .replace_group_wrapped_siks(&gid, &wrapped)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(
        serde_json::json!({ "group_id": gid, "status": "rotated" }),
    ))
}

/// GET /groups/inbox
async fn group_inbox(
    State(st): State<AppState>,
    auth: Bearer,
) -> Result<Json<Vec<GroupInboxItem>>, ApiError> {
    let user = auth_user(&st.repo, &auth.0).await?;
    let groups = st
        .repo
        .list_groups_for_member(&user)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    let mut items = Vec::with_capacity(groups.len());
    for g in groups {
        let wrapped = st
            .repo
            .get_group_wrapped_sik(&g.id, &user)
            .await
            .map_err(|e| ApiError::internal(&e.to_string()))?;
        items.push(GroupInboxItem {
            group_id: g.id.clone(),
            name: g.name,
            admin_uuid: g.admin_user_id,
            wrapped_sik: wrapped.as_ref().map(|w| b64(&w.wrapped_sik)),
            ephemeral_public_key: wrapped.as_ref().map(|w| b64(&w.ephemeral_public_key)),
        });
    }
    Ok(Json(items))
}

// ---------------------------------------------------------------------------
// Group items (§6) — Group-SIK-encrypted payload delivery
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub(crate) struct AddGroupItemReq {
    item_uuid: String,
    payload: String, // base64 (Group-SIK-encrypted ciphertext)
}

#[derive(Serialize)]
pub(crate) struct GroupItemResp {
    group_id: String,
    item_uuid: String,
    payload: String, // base64
}

#[derive(Serialize)]
pub(crate) struct GroupItemListResp {
    items: Vec<GroupItemResp>,
}

/// POST /groups/{group_id}/items
///
/// Admin uploads the item payload, encrypted once under the unified Group SIK.
/// Every member decrypts it with the Group SIK they hold via their wrapped key.
async fn add_group_item(
    State(st): State<AppState>,
    Path(group_id): Path<Uuid>,
    auth: Bearer,
    Json(req): Json<AddGroupItemReq>,
) -> Result<Json<GroupItemResp>, ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    let gid = group_id.to_string();
    require_admin(&st, &gid, &caller).await?;
    let payload = decode_b64(&req.payload)?;
    st.repo
        .add_group_item(&gid, &req.item_uuid, &payload, now_ms())
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(GroupItemResp {
        group_id: gid,
        item_uuid: req.item_uuid,
        payload: b64(&payload),
    }))
}

/// GET /groups/{group_id}/items
///
/// Any member lists the group's shared items (item_uuid + encrypted payload).
async fn list_group_items(
    State(st): State<AppState>,
    Path(group_id): Path<Uuid>,
    auth: Bearer,
) -> Result<Json<GroupItemListResp>, ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    let gid = group_id.to_string();
    // Membership is required (admin or ordinary member).
    let member = st
        .repo
        .is_group_member(&gid, &caller)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    if !member {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "only group members may read group items",
        ));
    }
    let rows = st
        .repo
        .list_group_items(&gid)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    let items = rows
        .into_iter()
        .map(|(item_uuid, payload)| GroupItemResp {
            group_id: gid.clone(),
            item_uuid,
            payload: b64(&payload),
        })
        .collect();
    Ok(Json(GroupItemListResp { items }))
}

/// DELETE /groups/{group_id}/items/{item_uuid}
async fn delete_group_item(
    State(st): State<AppState>,
    Path((group_id, item_uuid)): Path<(Uuid, String)>,
    auth: Bearer,
) -> Result<Json<serde_json::Value>, ApiError> {
    let caller = auth_user(&st.repo, &auth.0).await?;
    let gid = group_id.to_string();
    require_admin(&st, &gid, &caller).await?;
    st.repo
        .delete_group_item(&gid, &item_uuid)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?;
    Ok(Json(
        serde_json::json!({ "group_id": gid, "item_uuid": item_uuid, "status": "removed" }),
    ))
}

/// Enforce that `caller` is the admin of the group.
async fn require_admin(st: &AppState, group_id: &str, caller: &str) -> Result<(), ApiError> {
    let Some(group) = st
        .repo
        .get_group(group_id)
        .await
        .map_err(|e| ApiError::internal(&e.to_string()))?
    else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "group not found",
        ));
    };
    if group.admin_user_id != caller {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "only the group admin may manage membership",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::Repository;
    use axum::body::{to_bytes, Body};
    use axum::http::{header, Request, StatusCode};
    use serde_json::json;
    use std::sync::Arc;
    use tower::util::ServiceExt;

    /// Build an in-memory app with two users (u1: tok1, u2: tok2) and sessions.
    async fn test_state() -> AppState {
        let pool = crate::db::connect("sqlite::memory:")
            .await
            .expect("connect+migrate");
        let repo = Arc::new(Repository::new(pool));
        let now = now_ms();
        for (id, email, tok) in [
            ("u1", "a@example.com", "tok1"),
            ("u2", "b@example.com", "tok2"),
        ] {
            repo.create_user(
                id, email, &[0u8; 32], &[1u8; 16], &[2u8; 48], &[3u8; 48], now,
            )
            .await
            .expect("create user");
            // Insert the session directly: `sessions` has a NOT NULL
            // `created_at` that the `store_session` helper does not populate.
            sqlx::query(
                "INSERT INTO sessions (token, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)",
            )
            .bind(tok)
            .bind(id)
            .bind(now + 60_000)
            .bind(now)
            .execute(repo.pool())
            .await
            .expect("store session");
        }
        AppState::new(repo)
    }

    async fn call(
        router: &Router<()>,
        method: &str,
        path: &str,
        token: &str,
        body: Option<serde_json::Value>,
    ) -> (StatusCode, serde_json::Value) {
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header(header::AUTHORIZATION, format!("Bearer {token}"));
        let req = match body {
            Some(b) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(serde_json::to_vec(&b).unwrap()))
                .unwrap(),
            None => builder.body(Body::empty()).unwrap(),
        };
        let resp = router.clone().oneshot(req).await.expect("oneshot");
        let status = resp.status();
        let bytes = to_bytes(resp.into_body(), 1_048_576)
            .await
            .unwrap_or_default();
        let json = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
        };
        (status, json)
    }

    #[tokio::test]
    async fn public_key_directory() {
        let state = test_state().await;
        let key = [0x42u8; 32];
        // Upload u1's key before moving state into the router.
        state
            .repo
            .upsert_sharing_public_key("u1", &key, now_ms())
            .await
            .expect("upload key");
        let router = super::routes().with_state(state);

        // u1 fetches their own key back (round-trip).
        let (status, json) = call(&router, "GET", "/users/u1/public-key", "tok1", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["user_id"], "u1");
        assert_eq!(json["public_key"], b64(&key));

        // A user with no published key -> 404.
        let (status, _) = call(&router, "GET", "/users/u2/public-key", "tok1", None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // Unauthenticated -> 401.
        let (status, _) = call(&router, "GET", "/users/u1/public-key", "bogus", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn share_relay_and_revoke() {
        let state = test_state().await;
        let item_uuid = Uuid::new_v4().to_string();
        // The shares table FK-references items(uuid); create the item first.
        sqlx::query(
            "INSERT INTO items (uuid, user_id, version, enc_key_gen, deleted_date, payload, updated_at) \
             VALUES (?, ?, 1, 1, NULL, ?, ?)",
        )
        .bind(&item_uuid)
        .bind("u1")
        .bind(&[9u8; 8][..])
        .bind(now_ms())
        .execute(state.repo.pool())
        .await
        .expect("insert item");
        let router = super::routes().with_state(state);
        let ws = b64(&[1u8; 48]);
        let epk = b64(&[2u8; 32]);
        let payload = b64(b"ciphertext-blob");

        // u1 shares item to u2.
        let (status, json) = call(
            &router,
            "POST",
            "/shares/",
            "tok1",
            Some(json!({
                "item_uuid": item_uuid,
                "recipient_uuid": "u2",
                "wrapped_sik": ws,
                "ephemeral_public_key": epk,
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["sender_uuid"], "u1");

        // u1 uploads the payload.
        let (status, _) = call(
            &router,
            "POST",
            &format!("/shares/{item_uuid}/payload"),
            "tok1",
            Some(json!({ "payload": payload })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        // u2's inbox sees the KEM envelope + payload.
        let (status, json) = call(&router, "GET", "/shares/inbox", "tok2", None).await;
        assert_eq!(status, StatusCode::OK);
        let items = json.as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["item_uuid"], item_uuid);
        assert_eq!(items[0]["sender_uuid"], "u1");
        assert_eq!(items[0]["wrapped_sik"], ws);
        assert_eq!(items[0]["payload"], payload);

        // u2 cannot upload a payload for u1's share (not the owner).
        let (status, _) = call(
            &router,
            "POST",
            &format!("/shares/{item_uuid}/payload"),
            "tok2",
            Some(json!({ "payload": payload })),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);

        // u1 revokes; u2's inbox empties.
        let (status, _) = call(
            &router,
            "DELETE",
            &format!("/shares/{item_uuid}"),
            "tok1",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (_, json) = call(&router, "GET", "/shares/inbox", "tok2", None).await;
        assert_eq!(json.as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn group_add_and_member_inbox() {
        let state = test_state().await;
        let router = super::routes().with_state(state);

        // u1 creates a group.
        let (status, json) = call(
            &router,
            "POST",
            "/groups/",
            "tok1",
            Some(json!({ "name": "Family" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let group_id = json["group_id"].as_str().unwrap().to_string();
        assert_eq!(json["admin_uuid"], "u1");

        // u1 adds u2 as a member with a wrapped group SIK.
        let (status, _) = call(
            &router,
            "POST",
            &format!("/groups/{group_id}/members"),
            "tok1",
            Some(json!({
                "member_uuid": "u2",
                "wrapped_sik": b64(&[3u8; 48]),
                "ephemeral_public_key": b64(&[4u8; 32]),
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        // u2's group inbox shows the group + their wrapped key.
        let (status, json) = call(&router, "GET", "/groups/inbox", "tok2", None).await;
        assert_eq!(status, StatusCode::OK);
        let items = json.as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["group_id"], group_id);
        assert_eq!(items[0]["name"], "Family");
        assert!(items[0]["wrapped_sik"].is_string());

        // Non-admin (u2) cannot add another member (403).
        let (status, _) = call(
            &router,
            "POST",
            &format!("/groups/{group_id}/members"),
            "tok2",
            Some(json!({
                "member_uuid": "u1",
                "wrapped_sik": b64(&[5u8; 48]),
                "ephemeral_public_key": b64(&[6u8; 32]),
            })),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        // Admin removes u2; u2's group inbox empties.
        let (status, _) = call(
            &router,
            "DELETE",
            &format!("/groups/{group_id}/members/u2"),
            "tok1",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (_, json) = call(&router, "GET", "/groups/inbox", "tok2", None).await;
        assert_eq!(json.as_array().unwrap().len(), 0);
    }
}
