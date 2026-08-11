import { createServer } from 'node:http';

/**
 * Minimal static server hosting the Relying-Party origin the browser
 * authenticates against (http://localhost:5173). The page itself is inert; the
 * spec drives WebAuthn via `page.evaluate` after establishing this origin.
 */
const HTML = `<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>vautr webauthn e2e</title></head>
<body>vautr webauthn e2e origin</body>
</html>`;

createServer((_req, res) => {
  res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
  res.end(HTML);
}).listen(5173, '0.0.0.0', () => {
  console.log('e2e origin server listening on http://localhost:5173');
});
