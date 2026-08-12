import 'fake-indexeddb/auto';
import * as nodeCrypto from 'node:crypto';
if (!globalThis.crypto) globalThis.crypto = nodeCrypto;
if (!globalThis.crypto.randomUUID) globalThis.crypto.randomUUID = () => nodeCrypto.randomUUID();
const { VautrWebClient } = await import('../../../packages/vautr-client-sdk/src/realClient');
const BASE = 'http://localhost:8080';
const u = `verify-${Date.now()}@vautr.test`;
const c = new VautrWebClient({ baseUrl: BASE });
await c.register(u, 'correct-horse-battery-staple-e2e');
await c.login(u, 'correct-horse-battery-staple-e2e');
const token = (c as any).api.getToken();
const hdr = { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' };
const pr = await fetch(`${BASE}/projects`, {
  method: 'POST',
  headers: hdr,
  body: JSON.stringify({ name: 'Proj A', type: 'personal' }),
});
const pbody = await pr.text();
console.log('create project', pr.status, pbody);
if (pr.status !== 200) process.exit(1);
const proj = JSON.parse(pbody);
const sr = await fetch(`${BASE}/projects/${proj.uuid}/secrets`, {
  method: 'POST',
  headers: hdr,
  body: JSON.stringify({ key: 'DB_PASS', value_ciphertext: 'enc:abc' }),
});
console.log('create secret', sr.status, await sr.text());
const ms = await fetch(`${BASE}/mfa/status`, { headers: hdr });
console.log('mfa status', ms.status, await ms.text());
