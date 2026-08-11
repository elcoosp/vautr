/**
 * @vautr/api-contract
 *
 * Canonical Vautr server API contract. Re-exports the TypeScript types
 * generated from `openapi.json` so web/extension/CLI/desktop clients can
 * consume a single source of truth for the HTTP API.
 */
export * from "./generated.js";

/** API version reported in the OpenAPI document info block. */
export const API_VERSION = "0.1.0";

/** OpenAPI document version (3.0.x). */
export const OPENAPI_VERSION = "3.0.3";
