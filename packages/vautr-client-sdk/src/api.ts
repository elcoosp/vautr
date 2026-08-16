/**
 * HTTP transport to the Vautr server (api.md).
 *
 * A thin fetch wrapper that sends `Authorization: Bearer <token>`, `X-Request-ID`,
 * parses JSON, and maps error statuses to the documented error enum (api.md §6).
 * The server is an untrusted zero-knowledge blob store; all payloads here are
 * opaque base64 ciphertext (api.md §1).
 */

import type { CoreError } from './types';

/** Default server base URL (Vite dev proxies `/api`). */
export const DEFAULT_API_BASE = '/api';

/** btoa that works for arbitrary bytes (base64 wire encoding). */
export function toBase64(bytes: Uint8Array): string {
  let binary = '';
  for (let i = 0; i < bytes.length; i += 1) {
    binary += String.fromCharCode(bytes[i] ?? 0);
  }
  return btoa(binary);
}

/** Decode base64 into bytes (reverse of `toBase64`). */
export function fromBase64(value: string): Uint8Array {
  const binary = atob(value);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    out[i] = binary.charCodeAt(i);
  }
  return out;
}

/** HTTP error carrying the api.md §6 error enum + status. */
export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly context?: unknown;

  constructor(status: number, code: string, message: string, context?: unknown) {
    super(message);
    this.status = status;
    this.code = code;
    this.context = context;
  }
}

export interface ApiClientOptions {
  baseUrl?: string;
  fetchImpl?: typeof fetch;
  onRequestId?: () => string;
}

/** JSON-capable HTTP client for the Vautr server. */
export class ApiClient {
  private readonly baseUrl: string;
  private readonly fetchImpl: typeof fetch;
  private readonly onRequestId: () => string;
  private token: string | null = null;

  constructor(options: ApiClientOptions = {}) {
    this.baseUrl = (options.baseUrl ?? DEFAULT_API_BASE).replace(/\/$/, '');
    this.fetchImpl = options.fetchImpl ?? fetch.bind(globalThis);
    this.onRequestId = options.onRequestId ?? (() => crypto.randomUUID());
  }

  /** Set/clear the bearer token (after login / on lock). */
  setToken(token: string | null): void {
    this.token = token;
  }

  getToken(): string | null {
    return this.token;
  }

  /** Base URL of the server (used to build SSE / non-JSON endpoints). */
  getBaseUrl(): string {
    return this.baseUrl;
  }

  /** Perform a JSON request and return the parsed body (200/2xx). */
  async request<T>(method: string, path: string, body?: unknown): Promise<T> {
    const requestId = this.onRequestId();
    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      'X-Request-ID': requestId,
    };
    if (this.token) {
      headers.Authorization = `Bearer ${this.token}`;
    }
    const init: RequestInit = { method, headers };
    if (body !== undefined) {
      init.body = JSON.stringify(body);
    }
    const res = await this.fetchImpl(`${this.baseUrl}${path}`, init);
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
      throw this.toApiError(res.status, data);
    }
    return data as T;
  }

  private toApiError(status: number, data: unknown): ApiError {
    const code =
      (data && typeof data === 'object' && 'error' in data
        ? String((data as { error: unknown }).error)
        : 'http_error') ?? 'http_error';
    const message =
      (data && typeof data === 'object' && 'message' in data
        ? String((data as { message: unknown }).message)
        : `HTTP ${status}`) ?? `HTTP ${status}`;
    const context =
      data && typeof data === 'object' && 'context' in data
        ? (data as { context: unknown }).context
        : undefined;
    return new ApiError(status, code, message, context);
  }

  /** Map an `ApiError` to a `CoreError` per api.md §6. */
  static toCoreError(error: unknown): CoreError {
    if (error instanceof ApiError) {
      switch (error.status) {
        case 401:
          return { type: 'NetworkError', message: 'unauthorized' };
        case 410:
          return { type: 'NetworkError', message: 'cursor_expired' };
        case 412:
          return { type: 'NetworkError', message: 'precondition_failed' };
        case 422:
          return { type: 'EpochMismatch' };
        case 429:
          return { type: 'NetworkError', message: 'rate_limited' };
        default:
          return { type: 'NetworkError', message: error.message };
      }
    }
    return {
      type: 'NetworkError',
      message: error instanceof Error ? error.message : String(error),
    };
  }
}
