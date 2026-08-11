// Postgres/SQLite OCC behavior under concurrency (docs/architecture/roadmap.md
// §9 Phase 7, server-scaling.md §4).
//
// Every iteration does a read (GET /sync/pull) of the target item's current
// version, then pushes (POST /sync/push-batch) that same item with
// `target_version` equal to the version it read. Because many virtual users
// race on the same item, this produces genuine optimistic-concurrency write
// conflicts. The server resolves them atomically:
//
//   - exactly one writer whose target_version matched wins, and its response
//     reports `version == target_version + 1`;
//   - every losing writer gets a `conflict` result whose
//     `current_server_state.version == target_version + 1` (the winning
//     version).
//
// The check `resolved version == target + 1` asserts that conflict resolution
// always returns the correct, newest version — never a stale one.

import http from "k6/http";
import { check } from "k6";
import { sessionSetup, readTarget } from "./lib.js";

// A fixed opaque ciphertext blob (base64) — the server never parses payloads
// (no-plaintext rule), so any valid base64 is fine for load.
const BLOB = "bG9hZC10ZXN0LWJsb2ItY2lwaGVydGV4dA=="; // "load-test-blob-ciphertext"

export const options = {
  scenarios: {
    occ_contention: {
      // constant-arrival-rate sustains a steady concurrency while VUs are
      // recycled, exercising interleaved read-modify-write races.
      executor: "constant-arrival-rate",
      rate: __ENV.OCC_RATE ? Number(__ENV.OCC_RATE) : 50,
      timeUnit: "1s",
      duration: __ENV.OCC_DURATION ? __ENV.OCC_DURATION : "20s",
      preAllocatedVUs: __ENV.OCC_VUS ? Number(__ENV.OCC_VUS) : 25,
      maxVUs: __ENV.OCC_MAX_VUS ? Number(__ENV.OCC_MAX_VUS) : 100,
    },
  },
  thresholds: {
    http_req_failed: ["rate<0.01"],
    // The core invariant must never break: every resolved version is exactly
    // one past the version the writer targeted.
    checks: ["rate==1.00"],
  },
};

export function setup() {
  const data = sessionSetup();
  data.targetUuid = __ENV.VAUTR_ITEM_UUID || null;
  return data;
}

export default function (data) {
  const itemUuid =
    data.targetUuid || readTarget(data, null).uuid; // resolve once via pull

  const current = readTarget(data, itemUuid);
  const targetVersion = current.version;

  const res = http.post(
    `${data.url}/sync/push-batch`,
    JSON.stringify({
      items: [
        {
          uuid: current.uuid,
          target_version: targetVersion,
          enc_key_gen: current.enc_key_gen,
          payload: BLOB,
        },
      ],
    }),
    { headers: data.headers },
  );

  check(res, {
    "push-batch returns 200": (r) => r.status === 200,
  });

  let body;
  try {
    body = res.json();
  } catch (e) {
    check(res, { "response is valid JSON": () => false });
    return;
  }
  const r0 = body.results && body.results[0];

  check(res, {
    "outcome is success or conflict": () =>
      r0 && (r0.status === "success" || r0.status === "conflict"),
    "resolved version == target+1 (OCC correct)": () => {
      if (!r0) return false;
      const resolved =
        r0.status === "success" ? r0.version : r0.current_server_state?.version;
      return resolved === targetVersion + 1;
    },
  });
}
