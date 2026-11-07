// Interaction QA for the Vautr documentation site.
//
// Complements scripts/qa-screenshots.mjs (which proves every route renders)
// by exercising the interactive behaviours the skill's eyeball checklist
// calls out: the Pagefind search modal, the theme <select>, the mobile
// hamburger navigation (popover sidebar), and MDX <Tabs> switching.
//
// Selectors are verified against the built HTML (Starlight 0.42.10 markup):
//   - search button:  button[data-open-modal][aria-label="Search"]
//   - theme picker:   starlight-theme-select select   (native <select>)
//   - mobile menu:    button.sl-menu-button[popovertarget="starlight__sidebar"]
//   - tabs:           ul[role="tablist"] a[role="tab"]  + div[role="tabpanel"]
//
// Run against a production preview:
//   pnpm build && pnpm preview --port 4321 &
//   pnpm qa:interactions
//
// Exits non-zero on any failure and writes .qa/interactions.json.
import { chromium } from 'playwright';
import { writeFile, mkdir } from 'node:fs/promises';

const BASE = process.env.QA_BASE_URL ?? 'http://localhost:4321';
const OUT = '.qa';
const SHOTS = '.qa/interactions';

const results = [];
function record(name, ok, detail = '') {
	results.push({ name, ok, detail });
	console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${detail ? ` — ${detail}` : ''}`);
}

const browser = await chromium.launch();
await mkdir(SHOTS, { recursive: true });

try {
	// ------------------------------------------------------------------
	// 1. Search modal (Pagefind): open, type a domain term, expect hits.
	// ------------------------------------------------------------------
	{
		const ctx = await browser.newContext({
			viewport: { width: 1440, height: 900 },
			reducedMotion: 'reduce',
		});
		const page = await ctx.newPage();
		await page.goto(BASE + '/', { waitUntil: 'networkidle' });

		await page.locator('button[data-open-modal][aria-label="Search"]').click();
		const dialog = page.locator('dialog[aria-label="Search"]').first();
		await dialog.waitFor({ state: 'visible', timeout: 10_000 });
		record('search-modal-opens', true, 'dialog visible after clicking header search');

		// Type a term straight from the site's domain.
		const input = dialog.locator('input[type="text"], input[type="search"]').first();
		await input.click();
		await input.type('vault', { delay: 40 });
		await dialog
			.locator('.pagefind-ui__result')
			.first()
			.waitFor({ state: 'visible', timeout: 15_000 })
			.catch(() => {});
		const hits = await dialog.locator('.pagefind-ui__result').count();
		const excerpt = hits
			? (await dialog.locator('.pagefind-ui__result').first().innerText()).slice(0, 90)
			: '';
		await page.screenshot({ path: `${SHOTS}/search-modal.png` });

		record('search-returns-results', hits > 0, `${hits} result(s) for query "vault"`);
		record('search-excerpt-present', /vault/i.test(excerpt), `first hit: "${excerpt}"`);
		await ctx.close();
	}

	// ------------------------------------------------------------------
	// 2. Theme picker: switching to dark must flip data-theme.
	// ------------------------------------------------------------------
	{
		const ctx = await browser.newContext({
			viewport: { width: 1440, height: 900 },
			reducedMotion: 'reduce',
		});
		const page = await ctx.newPage();
		await page.goto(BASE + '/guides/self-host-docker/', { waitUntil: 'networkidle' });
		const before = await page.evaluate(() => document.documentElement.dataset.theme);

		await page
			.locator('starlight-theme-select select')
			.first()
			.selectOption('dark');
		await page.waitForTimeout(300);
		const after = await page.evaluate(() => document.documentElement.dataset.theme);

		record(
			'theme-toggle-dark',
			before === 'light' && after === 'dark',
			`before=${before} after=${after}`,
		);
		await page.screenshot({ path: `${SHOTS}/theme-toggled-dark.png` });
		await ctx.close();
	}

	// ------------------------------------------------------------------
	// 3. Mobile hamburger: opens the popover sidebar with sidebar links.
	// ------------------------------------------------------------------
	{
		const ctx = await browser.newContext({
			viewport: { width: 390, height: 844 },
			reducedMotion: 'reduce',
		});
		const page = await ctx.newPage();
		await page.goto(BASE + '/tutorials/first-vault/', { waitUntil: 'networkidle' });

		const menuButton = page.locator('button.sl-menu-button').first();
		await menuButton.click();
		await page.waitForTimeout(400);
		// The sidebar is a popover: :popover-open matches only while open.
		const sidebarOpen = await page.evaluate(() => {
			const el = document.getElementById('starlight__sidebar');
			return !!el && el.matches(':popover-open');
		});
		const linkCount = await page
			.locator('#starlight__sidebar a')
			.count();
		await page.screenshot({ path: `${SHOTS}/mobile-menu-open.png` });

		record(
			'mobile-menu-opens',
			sidebarOpen && linkCount > 0,
			`popover-open=${sidebarOpen}, ${linkCount} sidebar link(s) visible`,
		);
		await ctx.close();
	}

	// ------------------------------------------------------------------
	// 4. MDX <Tabs>: clicking the npm tab swaps the visible panel.
	// ------------------------------------------------------------------
	{
		const ctx = await browser.newContext({
			viewport: { width: 1440, height: 900 },
			reducedMotion: 'reduce',
		});
		const page = await ctx.newPage();
		await page.goto(BASE + '/reference/client-sdk/', { waitUntil: 'networkidle' });

		const tabs = page.locator('ul[role="tablist"] a[role="tab"]');
		const tabCount = await tabs.count();
		if (tabCount >= 2) {
			const pnpmPanel = page.locator('div[role="tabpanel"]').first();
			const pnpmText = (await pnpmPanel.innerText().catch(() => '')).slice(0, 80);

			await tabs.nth(1).click(); // "npm" tab
			await page.waitForTimeout(200);
			const npmPanel = page.locator('div[role="tabpanel"]:visible').first();
			const npmText = (await npmPanel.innerText().catch(() => '')).slice(0, 80);
			await page.screenshot({ path: `${SHOTS}/tab-switched.png` });

			record(
				'tabs-switch-panel',
				npmText.length > 0 && npmText !== pnpmText,
				`pnpm="${pnpmText}" npm="${npmText}"`,
			);
		} else {
			record('tabs-switch-panel', false, `expected >=2 tabs, found ${tabCount}`);
		}
		await ctx.close();
	}
} finally {
	await browser.close();
}

await writeFile(`${OUT}/interactions.json`, JSON.stringify(results, null, 2));

const failed = results.filter((r) => !r.ok);
if (failed.length) {
	console.error(`INTERACTIONS FAILED — ${failed.length} issue(s)`);
	process.exit(1);
}
console.log('INTERACTIONS PASSED — search, theme, mobile nav, tabs');
