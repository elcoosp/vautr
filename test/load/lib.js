// Shared helpers for the Vautr server load suite (test/load/).
//
// The suite drives the OCC + sync endpoints over HTTP, which is database
// agnostic: the same tests validate the Postgres OCC contract (the operational
// target described in docs/architecture/server-scaling.md / build-env-deploy.md
// §4.2) and the current SQLite-backed build alike, because both enforce the
// same atomic `UPDATE ... WHERE version = ?` semantics behind the API.

import http from 'k6/http';

/** Base URL of the server under test, from VAUTR_BASE_URL (default localhost). */
export function baseUrl() {
  return (__ENV.VAUTR_BASE_URL || 'http://localhost:8080').replace(/\/+$/, '');
}

/** Bearer session token from VAUTR_TOKEN (required). */
export function token() {
  return __ENV.VAUTR_TOKEN || '';
}

/** JSON + Authorization headers for the seeded session. */
export function authHeaders() {
  return {
    Authorization: `Bearer ${token()}`,
    'Content-Type': 'application/json',
  };
}

/**
 * Discover a target item and its current (version, enc_key_gen) from the live
 * server via GET /sync/pull. Returns `{ uuid, version, enc_key_gen }`.
 * Prefers VAUTR_ITEM_UUID; otherwise falls back to the first item found.
 * Throws if no seed item is available (see test/load/README.md for seeding).
 */
export function readTarget(data, uuid) {
  const res = http.get(`${data.url}/sync/pull?cursor=0&limit=1000`, { headers: data.headers });
  if (res.status !== 200) {
    throw new Error(`setup sync/pull failed: HTTP ${res.status} ${res.body}`);
  }
  const items = res.json().items || [];
  const it = items.find((i) => i.uuid === uuid) || items[0];
  if (!it) {
    throw new Error(
      'no seed item found. Seed at least one item for this user (see test/load/README.md) or set VAUTR_ITEM_UUID.',
    );
  }
  return { uuid: it.uuid, version: it.version, enc_key_gen: it.enc_key_gen };
}

/** Shared setup for scenarios that require an authenticated session. */
export function sessionSetup() {
  if (!token()) {
    throw new Error('VAUTR_TOKEN is required (Bearer session for the seeded user).');
  }
  return { url: baseUrl(), headers: authHeaders() };
}
