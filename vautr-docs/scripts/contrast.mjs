// Derive the Vautr accent palette from the brand gradient (#4F46E5 -> #06B6D4)
// and verify WCAG AA contrast for every accent role in light and dark themes.
function lum(hex) {
        const c = hex.replace('#', '');
        const [r, g, b] = [0, 2, 4].map((i) => {
                let v = parseInt(c.slice(i, i + 2), 16) / 255;
                return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
        });
        return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
function contrast(a, b) {
        const [l1, l2] = [lum(a), lum(b)].sort((x, y) => y - x);
        return (l1 + 0.05) / (l2 + 0.05);
}
// Starlight surfaces (approx. shipped defaults)
const LIGHT_BG = '#f6f6f7'; // page background light
const DARK_BG = '#1b1a1f'; // page background dark

const palette = {
        light: {
                '--sl-color-accent-low': '#eef2ff', // indigo-50, subtle backgrounds
                '--sl-color-accent': '#4f46e5', // brand indigo-600, links / active nav
                '--sl-color-accent-high': '#3730a3', // indigo-800, high-contrast text
        },
        dark: {
                '--sl-color-accent-low': '#312e81', // indigo-900, subtle backgrounds
                '--sl-color-accent': '#818cf8', // indigo-400, links / active nav on dark
                '--sl-color-accent-high': '#c7d2fe', // indigo-200, high-contrast text
        },
};

let fail = false;
for (const [mode, roles] of Object.entries(palette)) {
        const bg = mode === 'light' ? LIGHT_BG : DARK_BG;
        console.log(`\n[${mode}] page bg ${bg}:`);
        // --sl-color-accent is text/link color -> AA 4.5:1 against the page bg.
        const accent = contrast(roles['--sl-color-accent'], bg);
        console.log(
                `  accent ${roles['--sl-color-accent']} on ${bg} -> ${accent.toFixed(2)}:1 ${accent >= 4.5 ? 'AA-text' : 'FAIL'}`,
        );
        if (accent < 4.5) fail = true;
        // --sl-color-accent-high is text on --sl-color-accent-low backgrounds.
        const highOnLow = contrast(roles['--sl-color-accent-high'], roles['--sl-color-accent-low']);
        console.log(
                `  accent-high ${roles['--sl-color-accent-high']} on accent-low ${roles['--sl-color-accent-low']} -> ${highOnLow.toFixed(2)}:1 ${highOnLow >= 4.5 ? 'AA-text' : 'FAIL'}`,
        );
        if (highOnLow < 4.5) fail = true;
}
process.exit(fail ? 1 : 0);
