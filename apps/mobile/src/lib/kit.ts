import { Share } from 'react-native';

/**
 * Emergency Kit download helper (VTR-076). Renders a self-contained, printable
 * HTML document holding the 24-word Recovery Key. ZK: the mnemonic is passed in
 * from local storage (KEK-sealed at rest) and never sent to the server.
 */
export function renderKitHtml(mnemonic: string, email: string): string {
  const words = mnemonic.split(/\s+/).filter(Boolean);
  const wordRows = words
    .map((w, i) => `<li><span class="n">${i + 1}.</span> ${escapeHtml(w)}</li>`)
    .join('\n');
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8" />
<title>Vautr Emergency Kit</title>
<style>
  body { font-family: ui-sans-serif, system-ui, sans-serif; max-width: 640px; margin: 40px auto; padding: 0 20px; color: #111; }
  h1 { font-size: 22px; }
  .sub { color: #555; font-size: 14px; }
  .words { display: grid; grid-template-columns: 1fr 1fr; gap: 4px 24px; margin: 24px 0; padding: 16px; border: 1px solid #ddd; border-radius: 8px; }
  .words li { font-size: 15px; list-style: none; font-family: ui-monospace, monospace; }
  .words .n { color: #888; margin-right: 8px; }
  .warn { background: #fff7ed; border: 1px solid #fdba74; color: #9a3412; padding: 12px 14px; border-radius: 8px; font-size: 13px; }
  footer { margin-top: 32px; color: #888; font-size: 12px; }
</style>
</head>
<body>
  <h1>Vautr Emergency Kit</h1>
  <p class="sub">Account: ${escapeHtml(email)}</p>
  <div class="warn">
    Store this Recovery Key somewhere safe and private. Anyone with these 24 words can recover
    this account. Vautr cannot reset it for you.
  </div>
  <ol class="words">
${wordRows}
  </ol>
  <footer>Generated locally by the Vautr client. No server received these words.</footer>
</body>
</html>`;
}

/** Share the Emergency Kit via the platform share sheet (RN). */
export async function shareKit(mnemonic: string, email: string): Promise<void> {
  const html = renderKitHtml(mnemonic, email);
  await Share.share({
    title: 'Vautr Emergency Kit',
    message: `Vautr Emergency Kit for ${email}\n\nRecovery Key (24 words):\n${mnemonic}`,
  });
  // The HTML is the canonical artifact; on platforms that accept files we could
  // write it to disk, but the share-sheet text above carries the words reliably.
  void html;
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
