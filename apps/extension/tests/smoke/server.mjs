// Tiny static server serving tests/smoke/ for the Playwright smoke test. The
// extension content script matches `<all_urls>`, and this page provides a real
// URL (http://localhost:4174) for the content script to run against.

import { readFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { dirname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = dirname(fileURLToPath(import.meta.url));
const port = Number(process.env.PORT ?? 4174);

createServer(async (req, res) => {
  try {
    const pathname = decodeURIComponent((req.url ?? '/').split('?')[0] ?? '/');
    const rel = pathname === '/' ? 'index.html' : pathname.replace(/^\/+/, '');
    const file = normalize(join(root, rel));
    if (!file.startsWith(root)) {
      res.writeHead(403);
      res.end('forbidden');
      return;
    }
    const data = await readFile(file);
    res.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    res.end(data);
  } catch {
    res.writeHead(404);
    res.end('not found');
  }
}).listen(port, () => {
  console.log(`smoke server listening on http://localhost:${port}`);
});
