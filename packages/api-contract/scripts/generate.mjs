#!/usr/bin/env node
/**
 * Generate TypeScript types from the canonical OpenAPI document.
 *
 * Reads `openapi.json` and emits `src/generated.ts` with:
 *   - a type/interface for every `components.schemas` entry
 *   - a `<OperationId>Request` / `<OperationId>Response` type per operation
 *   - an `ApiPaths` typed map keyed by path + HTTP method for typed clients
 *
 * Self-contained (no runtime deps) so the contract package can be regenerated
 * anywhere. Run via `pnpm --filter @vautr/api-contract generate`.
 */
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
const spec = JSON.parse(readFileSync(join(root, "openapi.json"), "utf8"));

const SCHEMAS = spec.components?.schemas ?? {};
const PATHS = spec.paths ?? {};
const METHODS = ["get", "post", "put", "patch", "delete"];

/** Turn an OpenAPI schema object into a TypeScript type string. */
function typeRef(schema, inlineName) {
  if (!schema || typeof schema !== "object") return "unknown";

  if (schema.$ref) return refName(schema.$ref);

  if (schema.oneOf || schema.anyOf) {
    const list = schema.oneOf || schema.anyOf;
    const t = list.map((s) => typeRef(s)).join(" | ") || "unknown";
    return nullable(schema, `(${t})`);
  }

  if (schema.allOf) {
    const t = schema.allOf.map((s) => typeRef(s)).join(" & ") || "unknown";
    return nullable(schema, t);
  }

  const types = Array.isArray(schema.type) ? schema.type : [schema.type ?? "object"];
  const parts = types.map((t) => {
    if (t === "array") {
      const item = typeRef(schema.items ?? { type: "unknown" });
      return `Array<${item}>`;
    }
    if (t === "object") return objectType(schema, inlineName);
    if (t === "string") {
      if (Array.isArray(schema.enum) && schema.enum.length > 0) {
        return `(${schema.enum.map((v) => JSON.stringify(String(v))).join(" | ")})`;
      }
      return "string";
    }
    if (t === "integer" || t === "number") return "number";
    if (t === "boolean") return "boolean";
    if (t === "null") return "null";
    return "unknown";
  });

  return nullable(schema, parts.join(" | "));
}

function objectType(schema, inlineName) {
  const props = schema.properties ?? {};
  const required = new Set(schema.required ?? []);
  const propLines = Object.entries(props).map(([key, propSchema]) => {
    const name = /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(key) ? key : JSON.stringify(key);
    const optional = required.has(key) ? "" : "?";
    const valueType = typeRef(propSchema);
    return `    ${name}${optional}: ${valueType};`;
  });

  if (schema.additionalProperties) {
    const val =
      typeof schema.additionalProperties === "object"
        ? typeRef(schema.additionalProperties)
        : "unknown";
    propLines.push(`    [key: string]: ${val};`);
  }

  if (propLines.length === 0) {
    return inlineName ? "Record<string, unknown>" : "Record<string, unknown>";
  }
  return `{
${propLines.join("\n")}
  }`;
}

function nullable(schema, t) {
  return schema.nullable ? `${t} | null` : t;
}

function refName(ref) {
  return ref.replace(/^#\/components\/schemas\//, "");
}

function isIdentifier(k) {
  return /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(k);
}

/** Resolve an operation's application/json request body schema. */
function requestSchema(op) {
  const body = op.requestBody;
  if (!body) return undefined;
  const content = body.content ?? {};
  return content["application/json"]?.schema;
}

/** Resolve the primary (2xx) success response schema. */
function responseSchema(op) {
  const responses = op.responses ?? {};
  const status = Object.keys(responses).find((s) => /^2/.test(s));
  if (!status) return undefined;
  const content = responses[status]?.content ?? {};
  return content["application/json"]?.schema;
}

/** Collect path parameter schemas as { name: type } map. */
function pathParams(op) {
  const params = op.parameters ?? [];
  const out = {};
  for (const p of params) {
    if (p.in === "path" && p.schema) {
      const name = isIdentifier(p.name) ? p.name : JSON.stringify(p.name);
      out[name] = typeRef(p.schema);
    }
  }
  return out;
}

const out = [];
out.push("// Auto-generated from openapi.json. Do not edit.");
out.push('// Source: packages/api-contract/openapi.json (docs/architecture/api.md).');
out.push("// Regenerate with: `pnpm --filter @vautr/api-contract generate`.");
out.push("");
out.push("export type HttpMethod = \"get\" | \"post\" | \"put\" | \"patch\" | \"delete\";");
out.push("");

// ---- Schema types ----
out.push("// ---------------------------------------------------------------------------");
out.push("// Component schemas");
out.push("// ---------------------------------------------------------------------------");
for (const [name, schema] of Object.entries(SCHEMAS)) {
  if (schema.type === "object" && schema.properties) {
    out.push(`export interface ${name} ${objectType(schema, name)}`);
  } else if (schema.oneOf || schema.anyOf || schema.allOf) {
    out.push(`export type ${name} = ${typeRef(schema)};`);
  } else {
    out.push(`export type ${name} = ${typeRef(schema)};`);
  }
  out.push("");
}

// ---- Operation types ----
out.push("// ---------------------------------------------------------------------------");
out.push("// Operation request / response types");
out.push("// ---------------------------------------------------------------------------");
const opInfo = [];
for (const [path, pathItem] of Object.entries(PATHS)) {
  for (const method of METHODS) {
    const op = pathItem?.[method];
    if (!op) continue;
    const opId = op.operationId;
    if (!opId) continue;

    const req = requestSchema(op);
    const res = responseSchema(op);
    const reqType = req ? typeRef(req) : "undefined";
    const resType = res ? typeRef(res) : "undefined";

    out.push(`export type ${opId}Request = ${reqType};`);
    out.push(`export type ${opId}Response = ${resType};`);
    out.push("");

    opInfo.push({ path, method, opId, req, res });
  }
}

// ---- ApiPaths typed map ----
out.push("// ---------------------------------------------------------------------------");
out.push("// Typed path/method map for building HTTP clients");
out.push("// ---------------------------------------------------------------------------");
const grouped = new Map();
for (const info of opInfo) {
  if (!grouped.has(info.path)) grouped.set(info.path, []);
  grouped.get(info.path).push(info);
}

for (const [path, ops] of grouped) {
  const methodLines = ops.map((info) => {
    const pp = pathParams(info);
    const paramsKey = Object.keys(pp).length
      ? `pathParams: ${JSON.stringify(pp).replace(/"([^"]+)":/g, "$1:")};`
      : "pathParams?: undefined;";
    const reqKey = info.req ? `request: ${info.opId}Request;` : "request?: undefined;";
    const resKey = `response: ${info.opId}Response;`;
    return `    ${info.method}: {\n      ${paramsKey}\n      ${reqKey}\n      ${resKey}\n    };`;
  });

  const pathKey = isIdentifier(path) ? path : JSON.stringify(path);
  out.push(`export type ApiPath_${hashName(path)} = {`);
  out.push(methodLines.join("\n"));
  out.push(`};`);
  out.push("");
}

out.push("export type ApiPaths = {");
for (const [path, ops] of grouped) {
  const pathKey = isIdentifier(path) ? path : JSON.stringify(path);
  out.push(`  ${pathKey}: ApiPath_${hashName(path)};`);
}
out.push("};");
out.push("");

function hashName(path) {
  return path.replace(/[^A-Za-z0-9]/g, "_");
}

const outDir = join(root, "src");
mkdirSync(outDir, { recursive: true });
writeFileSync(join(outDir, "generated.ts"), out.join("\n"), "utf8");
console.log(
  `Generated src/generated.ts: ${Object.keys(SCHEMAS).length} schemas, ${opInfo.length} operations, ${grouped.size} paths.`
);
