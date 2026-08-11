import { MobileApiClient } from './api';
import { VautrAuth, secureTokenStore } from './auth';

/** App-wide shared API client + auth (lazy singleton). */
class AppServices {
  private _api: MobileApiClient | null = null;
  private _auth: VautrAuth | null = null;

  get api(): MobileApiClient {
    if (!this._api) {
      this._api = new MobileApiClient();
    }
    return this._api;
  }

  get auth(): VautrAuth {
    if (!this._auth) {
      this._auth = new VautrAuth({ api: this.api, store: secureTokenStore });
    }
    return this._auth;
  }

  /** Recreate the auth singleton (used after logout). */
  reset(): void {
    this._auth = null;
    this._api = null;
  }
}

export const services = new AppServices();
