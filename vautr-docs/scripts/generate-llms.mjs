// Generates public/llms.txt and dist/llms-full.txt.
//
// - public/llms.txt is a curated index (title + description + URL per page),
//   copied into dist by Astro's public/ passthrough.
// - dist/llms-full.txt is generated post-build from the rendered HTML of every
//   page, so AI agents can fetch the whole site as plain text.
import { readdir, readFile, writeFile, mkdir } from 'node:fs/promises';
import path from 'node:path';

const DIST = 'dist';

async function walk(dir) {
	const entries = await readdir(dir, { withFileTypes: true });
	const files = [];
	for (const e of entries) {
		const p = path.join(dir, e.name);
		if (e.isDirectory()) files.push(...(await walk(p)));
		else if (e.name === 'index.html') files.push(p);
	}
	return files;
}

function htmlToText(html) {
	return html
		.replace(/<script[\s\S]*?<\/script>/g, '')
		.replace(/<style[\s\S]*?<\/style>/g, '')
		.replace(/<nav[\s\S]*?<\/nav>/g, '')
		.replace(/<footer[\s\S]*?<\/footer>/g, '')
		.replace(/<!--[\s\S]*?-->/g, '')
		.replace(/<[^>]+>/g, ' ')
		.replace(/&lt;/g, '<')
		.replace(/&gt;/g, '>')
		.replace(/&amp;/g, '&')
		.replace(/&quot;/g, '"')
		.replace(/&#39;/g, "'")
		.replace(/[ \t]+/g, ' ')
		.replace(/\n{3,}/g, '\n\n')
		.trim();
}

const [stage] = process.argv.slice(2);

if (stage === 'index') {
	// Pre-build: emit the curated index into public/ so Astro copies it to dist/.
	const docsDir = 'src/content/docs';
	const pages = [];
	async function scan(dir, base) {
		for (const e of await readdir(dir, { withFileTypes: true })) {
			const p = path.join(dir, e.name);
			if (e.isDirectory()) await scan(p, `${base}/${e.name}`);
			else if (/\.mdx?$/.test(e.name)) {
				const raw = await readFile(p, 'utf8');
				const fm = raw.match(/^---\n([\s\S]*?)\n---/);
				const meta = fm?.[1] ?? '';
				const title = meta.match(/^title:\s*"?(.+?)"?\s*$/m)?.[1] ?? e.name;
				const desc = meta.match(/^description:\s*"?(.+?)"?\s*$/m)?.[1] ?? '';
				const url = base === '' && e.name.startsWith('index') ? '/' : `${base}/${e.name.replace(/\.mdx?$/, '')}/`;
				pages.push({ url, title, desc });
			}
		}
	}
	await scan(docsDir, '');
	pages.sort((a, b) => a.url.localeCompare(b.url));
	const lines = [
		'# Vautr Documentation',
		'',
		'> Vautr is a constitutionally open-source, zero-knowledge password manager built in Rust.',
		'> The server stores only ciphertext; authentication uses OPAQUE so the master password never leaves the client.',
		'',
		'## Pages',
		'',
		...pages.map((p) => `- [${p.title}](${p.url}): ${p.desc}`),
		'',
		'## Optional',
		'',
		'- [Full documentation as plain text](/llms-full.txt): every page rendered to text for AI consumption.',
		'- [OpenAPI specification](/vautr-openapi.json): machine-readable REST API contract (55 paths).',
		'',
	];
	await mkdir('public', { recursive: true });
	await writeFile('public/llms.txt', lines.join('\n'));
	console.log(`llms.txt: indexed ${pages.length} pages`);
} else {
	// Post-build: render every page to text and write dist/llms-full.txt.
	const files = await walk(DIST);
	const parts = ['# Vautr Documentation — Full Text', ''];
	for (const f of files) {
		const html = await readFile(f, 'utf8');
		const rel = path.relative(DIST, path.dirname(f));
		const url = rel === '.' ? '/' : `/${rel.split(path.sep).join('/')}/`;
		const title = html.match(/<title>(.*?)<\/title>/)?.[1] ?? url;
		parts.push(`---`, ``, `# ${title}`, `URL: ${url}`, ``, htmlToText(html), ``);
	}
	await writeFile(path.join(DIST, 'llms-full.txt'), parts.join('\n'));
	console.log(`llms-full.txt: rendered ${files.length} pages`);
}
