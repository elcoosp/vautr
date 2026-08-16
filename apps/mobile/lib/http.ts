/**
 * Thin HTTP transport for the mobile client (api.md §6).
 *
 * Sends `Authorization: Bearer <token>` and `X-Request-ID`, parses JSON, and
 * maps error statuses to the documented error envelope. This is a read-only
 * re-implementation of the `@vautr/client-sdk` `ApiClient` contract, kept local
 * so the mobile app owns its own transport (file-ownership: apps/mobile/**).
 */

/**
 * Default server base URL (matches the live dev server).
 *
 * On Android we default to `localhost:8080` and rely on `adb reverse
 * tcp:8080 tcp:8080` (set during dev) so the same default reaches the host
 * Mac from both a physical device and the emulator. iOS shares the host
 * network so `localhost` is correct there too. Override via `VAUTR_API_URL`
 * or an explicit arg.
 */
export const DEFAULT_API_BASE = 'http://localhost:8080';

/** Resolve the server base URL from the environment, else localhost. */
export function resolveApiBase(override?: string): string {
  if (override) return override.replace(/\/$/, '');
  const env = typeof process !== 'undefined' ? process.env.VAUTR_API_URL : undefined;
  return (env ?? DEFAULT_API_BASE).replace(/\/$/, '');
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

/** uuid generator that works in both Node and RN runtimes. */
function randomUUID(): string {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  // Minimal RFC4122 v4 fallback for environments without crypto.randomUUID.
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = Math.floor(Math.random() * 16);
    const v = c === 'x' ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}

export class HttpClient {
  private readonly baseUrl: string;
  private readonly fetchImpl: typeof fetch;
  private token: string | null = null;

  constructor(options: { baseUrl?: string; fetchImpl?: typeof fetch } = {}) {
    this.baseUrl = resolveApiBase(options.baseUrl);
    this.fetchImpl = options.fetchImpl ?? fetch.bind(globalThis);
  }

  setToken(token: string | null): void {
    this.token = token;
  }

  getToken(): string | null {
    return this.token;
  }

  /** Substitute `:param`/`{param}` placeholders in a path. */
  static interpolate(path: string, params: Record<string, string | number>): string {
    return path.replace(/\{([^}]+)\}/g, (_, key: string) => {
      const value = params[key];
      if (value === undefined) {
        throw new Error(`missing path param: ${key}`);
      }
      return encodeURIComponent(String(value));
    });
  }

  /** Perform a JSON request and return the parsed body on 2xx. */
  async request<T>(
    method: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE',
    path: string,
    body?: unknown,
  ): Promise<T> {
    const headers: Record<string, string> = {
      Accept: 'application/json',
      'Content-Type': 'application/json',
      'X-Request-ID': randomUUID(),
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
    const record = (
      data && typeof data === 'object' ? (data as Record<string, unknown>) : {}
    ) as Record<string, unknown>;
    const code = typeof record.error === 'string' ? record.error : 'http_error';
    const message = typeof record.message === 'string' ? record.message : `HTTP ${status}`;
    return new ApiError(status, code, message, record.context);
  }
}
