#!/usr/bin/env node
/**
 * Contract sanity check. Verifies the OpenAPI document is structurally valid:
 * every $ref resolves, every operation has an operationId and a 2xx response,
 * and the document parses as JSON. Fails (non-zero exit) on any violation.
 */
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
const spec = JSON.parse(readFileSync(join(root, "openapi.json"), "utf8"));

const METHODS = ["get", "post", "put", "patch", "delete"];
const errors = [];

// ---- $ref resolution across the whole document ----
const refs = new Set();
(function collect(node) {
  if (!node || typeof node !== "object") return;
  if (Array.isArray(node)) {
    node.forEach(collect);
    return;
  }
  if (typeof node.$ref === "string") refs.add(node.$ref);
  for (const v of Object.values(node)) collect(v);
})(spec);

for (const ref of refs) {
  if (!ref.startsWith("#/")) {
    errors.push(`external ref not supported: ${ref}`);
    continue;
  }
  const parts = ref.replace(/^#\//, "").split("/");
  let node = spec;
  for (const part of parts) {
    node = node?.[decodeURIComponent(part)];
  }
  if (node === undefined) errors.push(`unresolved $ref: ${ref}`);
}

// ---- Operation invariants ----
let ops = 0;
for (const [path, item] of Object.entries(spec.paths ?? {})) {
  for (const method of METHODS) {
    const op = item?.[method];
    if (!op) continue;
    ops += 1;
    if (!op.operationId) errors.push(`operation missing operationId: ${method.toUpperCase()} ${path}`);
    const has2xx = Object.keys(op.responses ?? {}).some((s) => /^2/.test(s));
    if (!has2xx) errors.push(`operation missing 2xx response: ${method.toUpperCase()} ${path}`);
  }
}

if (errors.length) {
  console.error("CONTRACT INVALID:");
  for (const e of errors) console.error(`  - ${e}`);
  process.exit(1);
}

console.log(
  `Contract OK: ${Object.keys(spec.paths).length} paths, ${ops} operations, ` +
    `${Object.keys(spec.components.schemas).length} schemas, ${refs.size} \$refs resolved.`
);
