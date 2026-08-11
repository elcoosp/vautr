const w = require('../wasm-pkg-nodejs/vautr_wasm.js');
const b64 = (b) => Buffer.from(b).toString('base64');
const unb64 = (s) => new Uint8Array(Buffer.from(s, 'base64'));
const BASE = 'http://localhost:8080';
async function req(method, path, body, token) {
  const h = { 'Content-Type': 'application/json' };
  if (token) h['Authorization'] = 'Bearer ' + token;
  const r = await fetch(BASE + path, { method, headers: h, body: body ? JSON.stringify(body) : undefined });
  const txt = await r.text();
  return { status: r.status, body: txt ? JSON.parse(txt) : null };
}
async function registerLogin(u, pw) {
  const salt = w.generate_kdf_salt_js();
  const mk = w.derive_master_key_js(pw, salt);
  const kek = w.derive_kek_js(mk);
  const svk = w.generate_svk_js();
  const svkWrapped = w.wrap_svk_js(svk, kek);
  const mnemonic = w.generate_recovery_mnemonic_js();
  const svkRkWrapped = w.wrap_svk_with_rk_js(svk, mnemonic);
  const start = w.opaque_register_start_js(pw);
  const sr = await req('POST', '/auth/register/start', { username: u, registration_start: b64(start.message) });
  const upload = w.opaque_register_finish_js(start.state, unb64(sr.body.registration_response), pw, u);
  await req('POST', '/auth/register/finish', { username: u, registration_finish: b64(upload), server_public_key: b64(new Uint8Array(32)), kdf_salt: b64(salt), svk_ciphertext_blob: b64(svkWrapped), svk_ciphertext_blob_rk: b64(svkRkWrapped) });
  const ls = w.opaque_login_start_js(pw);
  const lr = await req('POST', '/auth/login/start', { username: u, login_start: b64(ls.message) });
  const lf = w.opaque_login_finish_js(ls.state, unb64(lr.body.login_response), pw, u);
  const fr = await req('POST', '/auth/login/finish', { username: u, login_finish: b64(lf.upload) });
  return fr.body.session_token;
}
async function main() {
  const u = 'probe-' + Date.now() + '@vautr.test';
  const pw = 'correct-horse-battery-staple-e2e';
  const token = await registerLogin(u, pw);
  console.log('got token for', u);
  // create project
  const pr = await req('POST', '/projects', { name: 'Probe Proj', type: 'personal' }, token);
  const proj = pr.body;
  console.log('create project:', pr.status, proj.name);
  // create secret
  const se = await req('POST', '/secrets', { project_uuid: proj.uuid, key: 'DB_PASS', value_ciphertext: b64('supersecret') }, token);
  const secret = se.body;
  console.log('create secret:', se.status, secret.key, 'uuid', secret.uuid);
  // reveal with user token
  const val = await req('GET', '/secrets/' + secret.uuid + '/value', null, token);
  console.log('reveal with USER token:', val.status, JSON.stringify(val.body));
  // machine account without reveal scope
  const ma = await req('POST', '/machine-accounts', { name: 'no-reveal', scopes: ['secrets:read'] }, token);
  console.log('create machine-account:', ma.status, ma.body.uuid);
  const tok = await req('POST', '/tokens', { name: 'nr-tok', machine_account_uuid: ma.body.uuid, scopes: ['secrets:read'] }, token);
  console.log('create token:', tok.status, tok.body.token_id);
  const maTok = tok.body.token;
  const val2 = await req('GET', '/secrets/' + secret.uuid + '/value', null, maTok);
  console.log('reveal with MACHINE token (no reveal scope):', val2.status, JSON.stringify(val2.body));
  const val3 = await req('GET', '/secrets/' + secret.uuid, null, maTok);
  console.log('get secret meta with MACHINE token:', val3.status);
}
main().catch(e => { console.error('ERR', e); process.exit(1); });
