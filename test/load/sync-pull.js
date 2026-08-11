// Concurrent sync pulls (server-scaling.md §4: read-heavy pull under WAL).
// Exercises GET /sync/pull from many VUs simultaneously and asserts every
// response is a well-formed, consistent delta page. Under Postgres/SQLite this
// validates concurrent-read scaling while the OCC writers run (see
// occ-conflicts.js).

import http from "k6/http";
import { check } from "k6";
import { sessionSetup } from "./lib.js";

export const options = {
  scenarios: {
    concurrent_pull: {
      executor: "constant-arrival-rate",
      rate: __ENV.PULL_RATE ? Number(__ENV.PULL_RATE) : 100,
      timeUnit: "1s",
      duration: __ENV.PULL_DURATION ? __ENV.PULL_DURATION : "20s",
      preAllocatedVUs: __ENV.PULL_VUS ? Number(__ENV.PULL_VUS) : 40,
      maxVUs: __ENV.PULL_MAX_VUS ? Number(__ENV.PULL_MAX_VUS) : 150,
    },
  },
  thresholds: {
    http_req_failed: ["rate<0.01"],
    checks: ["rate==1.00"],
  },
};

export function setup() {
  return sessionSetup();
}

export default function (data) {
  const res = http.get(
    `${data.url}/sync/pull?cursor=0&limit=100`,
    { headers: data.headers },
  );

  check(res, {
    "pull returns 200": (r) => r.status === 200,
  });

  let body;
  try {
    body = res.json();
  } catch (e) {
    check(res, { "response is valid JSON": () => false });
    return;
  }

  check(res, {
    "items is an array": () => Array.isArray(body.items),
    "new_cursor is numeric and >= cursor": () =>
      typeof body.new_cursor === "number" && body.new_cursor >= 0,
    "has_more is boolean": () => typeof body.has_more === "boolean",
    "min_enc_key_gen is numeric": () => typeof body.min_enc_key_gen === "number",
  });
}
