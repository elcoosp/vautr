# Impeccable UI/UX Upgrade — All Clients

> **For agentic workers:** REQUIRED: Use executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform Vautr's UI from stock-shadcn-defaults into an out-of-distribution "Vault Ledger" security instrument — precise, confident, and alive — then port the visual world to extension, mobile, and desktop.

**Architecture:** Upgrade shared design tokens first (the single source of truth), then apply improvements web-first (the largest surface), and port CSS/component changes to extension, mobile (NativeWind adaptation), and desktop (GPUI token alignment). Every change preserves the existing information architecture, zero-knowledge semantics, and functional behavior.

**Tech Stack:** Tailwind CSS v4 (web/extension), NativeWind 4 + Tailwind 3 (mobile), CSS custom properties (shared tokens), `tw-animate-css` (already installed), Lucide icons, shadcn v4 component layer, Geist Variable font.

---

## Chunk 1: Shared Design Tokens & CSS Motion Foundation

Enhance the shared token system and add motion utilities that every client inherits.

### Task 1: Enhance design tokens with hover, focus, and motion values

**Files:**
- Modify: `packages/design-tokens/tokens.css`
- Modify: `packages/design-tokens/scripts/build.mjs` (if tokens are generated from JSON)
- Modify: `packages/design-tokens/tokens.json` (source of truth)

- [ ] **Step 1: Read the token source**

Read `packages/design-tokens/tokens.json` to understand the token generation pipeline.

- [ ] **Step 2: Add new semantic tokens to tokens.json**

Add these tokens under both `:root` (dark) and `.light`:

```jsonc
// Hover/focus state overlays (multiply against existing bg tokens)
"surface-hover": "#141c25",       // dark: slight lift from #0f151d
"surface-active": "#1a2430",      // dark: stronger lift
"surface-focus-ring": "#42b59a33", // dark: accent at 20% opacity

// Transitions
"transition-fast": "120ms",
"transition-base": "180ms",
"transition-slow": "280ms",
"ease-out-expo": "cubic-bezier(0.16, 1, 0.3, 1)",
"ease-out-quart": "cubic-bezier(0.25, 1, 0.5, 1)",

// Sidebar specific
"sidebar-item-hover": "#161d26",
"sidebar-item-active": "#1a2430",
"sidebar-indicator": "#42b59a",

// Skeleton / loading shimmer
"skeleton-base": "#151c24",
"skeleton-shimmer": "#1e2830"
```

For `.light`:

```jsonc
"surface-hover": "#f0f2f5",
"surface-active": "#e8eaee",
"surface-focus-ring": "#1f8f7433",
"sidebar-item-hover": "#e6e9ed",
"sidebar-item-active": "#dfe3e8",
"skeleton-base": "#eef0f3",
"skeleton-shimmer": "#f5f6f8"
```

- [ ] **Step 3: Regenerate tokens.css**

Run the token build script:
```bash
node packages/design-tokens/scripts/build.mjs
```

- [ ] **Step 4: Verify tokens.css updated**

Read `packages/design-tokens/tokens.css` and confirm the new tokens appear in `:root`, `.dark`, and `.light` blocks.

- [ ] **Step 5: Commit**

```bash
git add packages/design-tokens/
git commit -s "feat(tokens): add hover, focus, motion, and skeleton tokens"
```

---

### Task 2: Add motion CSS utilities to web client

**Files:**
- Modify: `apps/web/src/index.css`

- [ ] **Step 1: Add animation keyframes and utility classes**

Append to `apps/web/src/index.css` after the existing `@layer base` block:

```css
/* ── Vault Ledger motion layer ─────────────────────────────── */

@layer utilities {
  /* Page entrance: gentle rise + fade */
  .animate-vault-enter {
    animation: vault-enter 400ms cubic-bezier(0.16, 1, 0.3, 1) both;
  }
  @keyframes vault-enter {
    from { opacity: 0; transform: translateY(8px); }
    to   { opacity: 1; transform: translateY(0); }
  }

  /* Sidebar item hover transition (set on parent, children inherit) */
  .vault-nav-item {
    transition: background-color 150ms cubic-bezier(0.16, 1, 0.3, 1),
                color 150ms cubic-bezier(0.16, 1, 0.3, 1);
  }

  /* Active indicator: accent bar slides in from left */
  .vault-active-indicator {
    position: relative;
  }
  .vault-active-indicator::before {
    content: '';
    position: absolute;
    left: 0;
    top: 25%;
    bottom: 25%;
    width: 2px;
    border-radius: 1px;
    background: var(--primary);
    transform: scaleY(0);
    transition: transform 200ms cubic-bezier(0.16, 1, 0.3, 1);
  }
  .vault-active-indicator[data-active="true"]::before {
    transform: scaleY(1);
  }

  /* Button press feedback */
  .vault-btn-press {
    transition: transform 80ms ease, opacity 150ms ease;
  }
  .vault-btn-press:active:not(:disabled) {
    transform: scale(0.97);
  }

  /* Card hover lift (subtle, 1px only) */
  .vault-card-hover {
    transition: box-shadow 200ms cubic-bezier(0.16, 1, 0.3, 1),
                border-color 200ms cubic-bezier(0.16, 1, 0.3, 1);
  }
  .vault-card-hover:hover {
    border-color: color-mix(in oklch, var(--border), var(--primary) 15%);
  }

  /* Skeleton shimmer animation */
  .animate-shimmer {
    animation: shimmer 1.8s ease-in-out infinite;
    background: linear-gradient(
      90deg,
      var(--skeleton-base) 25%,
      var(--skeleton-shimmer) 50%,
      var(--skeleton-base) 75%
    );
    background-size: 200% 100%;
  }
  @keyframes shimmer {
    0%   { background-position: 200% 0; }
    100% { background-position: -200% 0; }
  }

  /* Copy success flash */
  .animate-copy-flash {
    animation: copy-flash 600ms cubic-bezier(0.16, 1, 0.3, 1);
  }
  @keyframes copy-flash {
    0%   { background-color: color-mix(in oklch, var(--primary), transparent 80%); }
    100% { background-color: transparent; }
  }

  /* Fade in for conditional content */
  .animate-fade-in {
    animation: fade-in 200ms cubic-bezier(0.16, 1, 0.3, 1) both;
  }
  @keyframes fade-in {
    from { opacity: 0; }
    to   { opacity: 1; }
  }
}

/* ── Focus ring refinement ─────────────────────────────────── */
:focus-visible {
  outline: 2px solid var(--ring);
  outline-offset: 2px;
  border-radius: 4px;
}

/* Ensure smooth scrolling for page transitions */
html {
  scroll-behavior: smooth;
}
```

- [ ] **Step 2: Build and verify no CSS errors**

```bash
cd apps/web && pnpm build
```

Expected: Build succeeds, no Tailwind or CSS errors.

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/index.css
git commit -s "feat(web): add Vault Ledger motion utilities and skeleton animation"
```

---

### Task 3: Add motion CSS utilities to extension client

**Files:**
- Modify: `apps/extension/src/styles/globals.css`

- [ ] **Step 1: Add matching motion layer**

Append the same animation keyframes and utilities to `apps/extension/src/styles/globals.css` (after the `@layer base` block), identical to the web version. The extension inherits the same tokens.

- [ ] **Step 2: Build and verify**

```bash
cd apps/extension && pnpm build
```

- [ ] **Step 3: Commit**

```bash
git add apps/extension/src/styles/globals.css
git commit -s "feat(extension): add Vault Ledger motion utilities"
```

---

## Chunk 2: Web Client — Sidebar & Navigation

Transform the flat 12-item sidebar into a grouped, hierarchical security instrument navigation with active indicators and motion.

### Task 4: Redesign the sidebar layout

**Files:**
- Modify: `apps/web/src/routes/_authed.tsx`

- [ ] **Step 1: Read current sidebar code**

Read `apps/web/src/routes/_authed.tsx` (already known: lines 27-98).

- [ ] **Step 2: Rewrite sidebar with grouped nav and active indicator**

Replace the NAV constant and sidebar JSX with:

```tsx
const NAV_SECTIONS = [
  {
    label: 'Overview',
    items: [
      { to: '/dashboard', label: 'Dashboard', icon: LayoutDashboard },
    ],
  },
  {
    label: 'Vault',
    items: [
      { to: '/projects', label: 'Projects', icon: Folder },
      { to: '/vault', label: 'Vault', icon: Eye },
      { to: '/secrets', label: 'Secrets', icon: HardDrive },
      { to: '/shares', label: 'Shares', icon: Share2 },
    ],
  },
  {
    label: 'Tools',
    items: [
      { to: '/generator', label: 'Generator', icon: Settings2 },
      { to: '/mfa', label: 'MFA & security', icon: CircleCheck },
    ],
  },
  {
    label: 'Administration',
    items: [
      { to: '/machine-accounts', label: 'Machine accounts', icon: Bot },
      { to: '/tokens', label: 'Tokens', icon: Globe },
      { to: '/audit', label: 'Security log', icon: ScrollText },
      { to: '/import-export', label: 'Import / export', icon: Replace },
      { to: '/settings', label: 'Settings', icon: Settings },
    ],
  },
] as const;
```

Replace sidebar JSX:

```tsx
<aside className="flex w-60 shrink-0 flex-col border-r border-border bg-surface">
  {/* Brand */}
  <div className="flex items-center gap-2.5 border-b border-border px-4 py-4">
    <span className="grid size-8 place-items-center rounded-lg bg-accent/15 text-accent transition-colors duration-200">
      <Lock className="size-4" aria-hidden="true" />
    </span>
    <span className="text-base font-semibold tracking-tight text-text">Vautr</span>
  </div>

  {/* Grouped navigation */}
  <nav className="flex-1 overflow-y-auto px-2 pt-3" aria-label="Main navigation">
    {NAV_SECTIONS.map((section, si) => (
      <div key={section.label} className={si > 0 ? 'mt-4' : ''}>
        <span className="mb-1 block px-3 text-[10px] font-semibold uppercase tracking-widest text-text-muted/60">
          {section.label}
        </span>
        <div className="space-y-0.5">
          {section.items.map(({ to, label, icon: Icon }) => (
            <Link
              key={to}
              to={to}
              data-tour={TOUR_ANCHORS[to]}
              activeOptions={{ exact: to === '/dashboard' }}
              className="vault-nav-item vault-active-indicator flex items-center gap-2.5 rounded-md px-3 py-2 text-sm text-text-muted hover:bg-[var(--sidebar-item-hover)] hover:text-text data-[status=active]:bg-[var(--sidebar-item-active)] data-[status=active]:text-text"
            >
              <Icon className="size-4 shrink-0" aria-hidden="true" />
              {label}
            </Link>
          ))}
        </div>
      </div>
    ))}
  </nav>

  {/* Footer */}
  <div className="border-t border-border p-2">
    <button
      type="button"
      onClick={() => void logout()}
      className="vault-nav-item flex w-full items-center gap-2.5 rounded-md px-3 py-2 text-sm text-text-muted hover:bg-[var(--sidebar-item-hover)] hover:text-text"
    >
      <LogOut className="size-4" aria-hidden="true" />
      Log out
    </button>
  </div>
</aside>
```

- [ ] **Step 3: Remove the now-unused flat NAV constant**

- [ ] **Step 4: Build and verify**

```bash
cd apps/web && pnpm typecheck
```

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/routes/_authed.tsx
git commit -s "feat(web): group sidebar nav into sections with active indicator"
```

---

### Task 5: Improve page content transitions

**Files:**
- Modify: `apps/web/src/routes/_authed/dashboard.tsx`
- Modify: `apps/web/src/routes/_authed/generator.tsx`
- Modify: `apps/web/src/routes/_authed/secrets.tsx`
- Modify: `apps/web/src/routes/_authed/mfa.tsx`
- Modify: `apps/web/src/routes/_authed/audit.tsx`
- Modify: `apps/web/src/routes/_authed/settings.tsx`
- Modify: `apps/web/src/routes/_authed/shares.tsx`
- Modify: `apps/web/src/routes/_authed/tokens.tsx`
- Modify: `apps/web/src/routes/_authed/machine-accounts.tsx`
- Modify: `apps/web/src/routes/_authed/import-export.tsx`

- [ ] **Step 1: Add `animate-vault-enter` to every page root**

Each page's root `<div>` gets `className="animate-vault-enter ..."`. For example, in dashboard.tsx:

```tsx
// Before:
<div className="space-y-6 p-6">

// After:
<div className="animate-vault-enter space-y-6 p-6">
```

Apply the same class to every page root `<div>`.

- [ ] **Step 2: Build and verify typecheck**

```bash
cd apps/web && pnpm typecheck
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/routes/_authed/
git commit -s "feat(web): add page entrance animations to all views"
```

---

## Chunk 3: Web Client — Login & Registration

Transform the login/registration from generic form to a vault entry experience with visual identity.

### Task 6: Redesign login page

**Files:**
- Modify: `apps/web/src/routes/login.tsx`

- [ ] **Step 1: Rewrite login page with vault-entry experience**

Replace the login page JSX:

```tsx
function LoginPage() {
  const isLocked = useIsLocked();
  const navigate = useNavigate();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!isLocked) void navigate({ to: '/dashboard' });
  }, [isLocked, navigate]);

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!username.trim() || !password) {
      setError('Enter your username and master password.');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await login(username.trim(), password);
      void navigate({ to: '/dashboard' });
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Login failed.');
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className="flex min-h-screen items-center justify-center bg-bg p-6">
      <div className="animate-vault-enter w-full max-w-sm space-y-8">
        {/* Brand header */}
        <div className="space-y-2 text-center">
          <div className="mx-auto grid size-16 place-items-center rounded-2xl bg-surface ring-1 ring-border">
            <img src="/logo.svg" alt="" className="size-10" aria-hidden="true" />
          </div>
          <h1 className="text-2xl font-semibold tracking-tight text-text">Vautr</h1>
          <p className="text-sm text-text-muted">Zero-knowledge vault</p>
        </div>

        {/* Form card */}
        <div className="rounded-xl border border-border bg-surface p-6">
          <p className="mb-5 text-sm text-text-muted">Unlock your vault to continue.</p>
          <form onSubmit={onSubmit} className="space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="username">Username</Label>
              <Input
                id="username"
                type="text"
                autoComplete="username"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder="you@example.com"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="master-password">Master password</Label>
              <Input
                id="master-password"
                type="password"
                autoComplete="current-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="Enter your master password"
              />
            </div>
            {error ? (
              <p role="alert" className="text-sm text-danger">{error}</p>
            ) : null}
            <Button
              type="submit"
              className="vault-btn-press w-full"
              disabled={busy}
            >
              {busy ? 'Unlocking…' : 'Unlock vault'}
            </Button>
          </form>
        </div>

        <p className="text-center text-xs text-text-muted">
          New here?{' '}
          <Link to="/register" className="text-accent underline-offset-2 hover:underline">
            Register an account
          </Link>
        </p>
      </div>
    </main>
  );
}
```

- [ ] **Step 2: Build and verify typecheck**

```bash
cd apps/web && pnpm typecheck
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/routes/login.tsx
git commit -s "feat(web): redesign login with vault-entry brand experience"
```

---

### Task 7: Redesign register page

**Files:**
- Modify: `apps/web/src/routes/register.tsx`

- [ ] **Step 1: Apply matching vault-entry treatment**

Same brand header pattern as login: centered logo + "Vautr" title + "Zero-knowledge vault" subtitle. Card with form. Add `animate-vault-enter` and `vault-btn-press` classes.

- [ ] **Step 2: Build and verify typecheck**

```bash
cd apps/web && pnpm typecheck
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/routes/register.tsx
git commit -s "feat(web): redesign register with vault-entry brand experience"
```

---

## Chunk 4: Web Client — Dashboard, Vault, Generator

Improve the core operational views with better visual hierarchy, hover states, and interaction feedback.

### Task 8: Enhance dashboard stat cards and project list

**Files:**
- Modify: `apps/web/src/routes/_authed/dashboard.tsx`

- [ ] **Step 1: Redesign StatCard with icon background and better hierarchy**

```tsx
function StatCard({ icon: Icon, label, value, hint }: { ... }) {
  return (
    <Card className="vault-card-hover">
      <CardHeader className="flex-row items-center justify-between space-y-0 pb-2">
        <CardDescription>{label}</CardDescription>
        <span className="grid size-8 place-items-center rounded-lg bg-accent/15">
          <Icon className="size-4 text-accent" aria-hidden="true" />
        </span>
      </CardHeader>
      <CardContent>
        <div className="text-3xl font-semibold tracking-tight text-text">{value}</div>
        {hint ? (
          <Badge className="mt-1.5" variant="secondary">{hint}</Badge>
        ) : null}
      </CardContent>
    </Card>
  );
}
```

- [ ] **Step 2: Add hover transition to project list rows**

Update the project list `<Link>` with `vault-card-hover` class and `vault-btn-press`:

```tsx
<Link
  key={p.uuid}
  to="/projects/$uuid"
  params={{ uuid: p.uuid }}
  className="vault-card-hover vault-btn-press flex items-center justify-between rounded-md border border-border bg-surface-raised px-4 py-3 transition-colors hover:bg-border/40"
>
```

- [ ] **Step 3: Add loading skeleton state**

Replace the simple `Loading…` text with skeleton cards:

```tsx
{!data && !error && (
  <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
    {[1, 2, 3, 4].map((i) => (
      <Card key={i}>
        <CardHeader className="flex-row items-center justify-between space-y-0 pb-2">
          <div className="h-3 w-20 animate-shimmer rounded" />
          <div className="size-8 rounded-lg animate-shimmer" />
        </CardHeader>
        <CardContent>
          <div className="h-8 w-12 animate-shimmer rounded" />
        </CardContent>
      </Card>
    ))}
  </div>
)}
```

- [ ] **Step 4: Build and verify**

```bash
cd apps/web && pnpm typecheck
```

- [ ] **Step 5: Commit**

```bash
git add apps/web/src/routes/_authed/dashboard.tsx
git commit -s "feat(web): enhance dashboard with card hover, loading skeletons"
```

---

### Task 9: Improve vault list items with better selection states

**Files:**
- Modify: `apps/web/src/components/VaultList.tsx`

- [ ] **Step 1: Add vault-nav-item transition and improved selection indicator**

Update each vault item button:

```tsx
<button
  type="button"
  role="option"
  aria-selected={isSelected}
  data-uuid={item.uuid}
  onClick={() => onSelect(item.uuid)}
  className={`vault-nav-item vault-active-indicator flex w-full items-center gap-3 border-b border-border px-4 text-left ${
    isSelected
      ? 'bg-[var(--sidebar-item-active)] text-text'
      : 'bg-surface text-text-muted hover:bg-[var(--sidebar-item-hover)] hover:text-text'
  }`}
  style={{ height: virtualRow.size - 1 }}
  data-active={isSelected ? 'true' : undefined}
>
```

- [ ] **Step 2: Build and verify**

```bash
cd apps/web && pnpm typecheck
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/components/VaultList.tsx
git commit -s "feat(web): improve vault list selection states and transitions"
```

---

### Task 10: Improve vault view header and empty state

**Files:**
- Modify: `apps/web/src/components/VaultView.tsx`

- [ ] **Step 1: Add vault-btn-press to header buttons**

Update the "Add item" and "Lock" buttons:

```tsx
<button
  type="button"
  onClick={() => { setAdding(true); setSelectedUuid(null); }}
  data-tour="add-secret"
  aria-label="Add item"
  className="vault-btn-press inline-flex items-center gap-1.5 rounded-md border border-border px-3 py-1.5 text-sm text-text-muted hover:bg-surface-raised hover:text-text"
>
```

Same for Lock button.

- [ ] **Step 2: Build and verify**

```bash
cd apps/web && pnpm typecheck
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/components/VaultView.tsx
git commit -s "feat(web): add press feedback to vault header buttons"
```

---

### Task 11: Improve generator page with better visual feedback

**Files:**
- Modify: `apps/web/src/routes/_authed/generator.tsx`

- [ ] **Step 1: Add vault-btn-press to copy/regenerate buttons**

```tsx
<Button variant="outline" size="icon" onClick={copy} aria-label="Copy password" className="vault-btn-press">
  <Copy className="size-4" aria-hidden="true" />
</Button>
<Button variant="outline" size="icon" onClick={regenerate} aria-label="Regenerate" className="vault-btn-press">
  <RefreshCw className="size-4" aria-hidden="true" />
</Button>
```

- [ ] **Step 2: Improve strength indicator bar animation**

Replace the static bar with an animated transition:

```tsx
<div className="h-2 overflow-hidden rounded-full bg-border">
  <div
    className={`h-full transition-all duration-500 ease-out ${scoreColors[analysis.score] ?? 'bg-border'}`}
    style={{ width: `${Math.min(100, analysis.entropyBits)}%` }}
  />
</div>
```

- [ ] **Step 3: Build and verify**

```bash
cd apps/web && pnpm typecheck
```

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/_authed/generator.tsx
git commit -s "feat(web): add press feedback and animated strength bar to generator"
```

---

## Chunk 5: Web Client — Secrets, Error States, Empty States

Improve operational consistency across secrets table and shared components.

### Task 12: Improve secrets table with better row states

**Files:**
- Modify: `apps/web/src/routes/_authed/secrets.tsx`

- [ ] **Step 1: Add vault-btn-press to reveal buttons**

```tsx
<Button
  size="sm"
  variant="outline"
  onClick={() => void onReveal(secret.uuid)}
  className="vault-btn-press"
>
```

- [ ] **Step 2: Add animate-copy-flash to copied state**

```tsx
{copied[secret.uuid] ? (
  <span className="animate-copy-flash inline-flex items-center">
    <EyeOff className="mr-1 size-4" aria-hidden="true" />
    Copied
  </span>
) : (
  <>
    <Eye className="mr-1 size-4" aria-hidden="true" />
    Reveal
  </>
)}
```

- [ ] **Step 3: Build and verify**

```bash
cd apps/web && pnpm typecheck
```

- [ ] **Step 4: Commit**

```bash
git add apps/web/src/routes/_authed/secrets.tsx
git commit -s "feat(web): add press feedback and copy flash to secrets table"
```

---

### Task 13: Improve EmptyState component

**Files:**
- Modify: `apps/web/src/components/EmptyState.tsx`

- [ ] **Step 1: Refine empty state icon container**

Update the icon container for a more distinctive look:

```tsx
<div className="flex h-12 w-12 items-center justify-center rounded-xl bg-accent/10 ring-1 ring-accent/20">
  <Icon className="h-5 w-5 text-accent" />
</div>
```

- [ ] **Step 2: Build and verify**

```bash
cd apps/web && pnpm typecheck
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/components/EmptyState.tsx
git commit -s "feat(web): refine empty state icon with accent ring"
```

---

### Task 14: Improve SyncIndicator with better visual feedback

**Files:**
- Modify: `apps/web/src/components/SyncIndicator.tsx`

- [ ] **Step 1: Refine sync indicator with pulse animation**

```tsx
export function SyncIndicator() {
  const sync = useSyncState();

  if (!sync.isSyncing) {
    return (
      <span role="status" aria-label="Vault is up to date" className="inline-flex items-center gap-1.5 text-sm text-text-muted">
        <span className="size-1.5 rounded-full bg-success" aria-hidden="true" />
        Synced
      </span>
    );
  }

  return (
    <span role="status" aria-live="polite" className="inline-flex items-center gap-1.5 text-sm text-text">
      <span className="relative flex size-2">
        <span className="absolute inline-flex size-full animate-ping rounded-full bg-accent opacity-50" />
        <span className="relative inline-flex size-2 rounded-full bg-accent" />
      </span>
      Syncing {sync.progress}%
    </span>
  );
}
```

- [ ] **Step 2: Build and verify**

```bash
cd apps/web && pnpm typecheck
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/src/components/SyncIndicator.tsx
git commit -s "feat(web): improve sync indicator with pulse animation"
```

---

## Chunk 6: Extension Client Porting

Port CSS motion utilities and component improvements to the browser extension.

### Task 15: Port sidebar tab improvements to extension

**Files:**
- Modify: `apps/extension/src/popup/App.tsx`

- [ ] **Step 1: Add vault-btn-press to lock button and tab triggers**

```tsx
<Button size="sm" variant="outline" onClick={() => void handleLock()} className="vault-btn-press">
  Lock
</Button>
```

Add transition class to tab triggers:

```tsx
<TabsTrigger
  key={t.id}
  value={t.id}
  data-tour={TOUR_ANCHORS[t.id]}
  className="vault-nav-item flex flex-col items-center gap-0.5 px-2 py-1.5 text-[10px]"
>
```

- [ ] **Step 2: Build and verify**

```bash
cd apps/extension && pnpm build
```

- [ ] **Step 3: Commit**

```bash
git add apps/extension/src/popup/App.tsx
git commit -s "feat(extension): add motion classes to popup navigation"
```

---

### Task 16: Port vault tab improvements to extension

**Files:**
- Modify: `apps/extension/src/popup/components/VaultTab.tsx`

- [ ] **Step 1: Add vault-list-item transitions and hover states**

Read the VaultTab component, identify the vault item list, and add the same `vault-nav-item` class and hover states as the web VaultList.

- [ ] **Step 2: Build and verify**

```bash
cd apps/extension && pnpm build
```

- [ ] **Step 3: Commit**

```bash
git add apps/extension/src/popup/components/VaultTab.tsx
git commit -s "feat(extension): add vault list item transitions"
```

---

### Task 17: Port generator and dashboard improvements to extension

**Files:**
- Modify: `apps/extension/src/popup/components/GeneratorTab.tsx`
- Modify: `apps/extension/src/popup/components/DashboardTab.tsx`

- [ ] **Step 1: Add vault-btn-press to generator buttons**

- [ ] **Step 2: Add vault-card-hover to dashboard stat cards**

- [ ] **Step 3: Build and verify**

```bash
cd apps/extension && pnpm build
```

- [ ] **Step 4: Commit**

```bash
git add apps/extension/src/popup/components/GeneratorTab.tsx apps/extension/src/popup/components/DashboardTab.tsx
git commit -s "feat(extension): add motion classes to generator and dashboard"
```

---

## Chunk 7: Mobile Client Porting

Adapt visual improvements to NativeWind/React Native. Motion uses `react-native-reanimated`.

### Task 18: Add animation tokens to mobile NativeWind config

**Files:**
- Modify: `apps/mobile/tailwind.config.js`
- Modify: `apps/mobile/global.css`

- [ ] **Step 1: Add the new CSS custom properties to global.css**

Add the new tokens (`--surface-hover`, `--sidebar-item-hover`, `--skeleton-base`, `--skeleton-shimmer`) to both `:root` and `.light` blocks in `apps/mobile/global.css`.

- [ ] **Step 2: Extend tailwind.config.js colors**

Add the new semantic tokens to the Tailwind config's `theme.extend.colors`:

```js
colors: {
  // ...existing...
  'surface-hover': 'hsl(var(--surface-hover))',
  'surface-active': 'hsl(var(--surface-active))',
}
```

- [ ] **Step 3: Build and verify**

```bash
cd apps/mobile && pnpm typecheck
```

- [ ] **Step 4: Commit**

```bash
git add apps/mobile/tailwind.config.js apps/mobile/global.css
git commit -s "feat(mobile): add surface hover/active tokens to NativeWind config"
```

---

### Task 19: Add entrance animations to mobile routes

**Files:**
- Modify: `apps/mobile/src/routes/_app.dashboard.tsx`
- Modify: `apps/mobile/src/routes/_app.secrets.tsx`
- Modify: `apps/mobile/src/routes/_app.generator.tsx`
- Modify: `apps/mobile/src/routes/_app.settings.tsx`
- Modify: `apps/mobile/src/routes/_app.mfa.tsx`
- Modify: `apps/mobile/src/routes/_app.audit.tsx`

- [ ] **Step 1: Add FadeInDown from reanimated to page root views**

In each route file, wrap the root view with an animated entrance:

```tsx
import Animated, { FadeInDown } from 'react-native-reanimated';

// In the component return:
<Animated.View entering={FadeInDown.duration(300).springify()} className="flex-1 p-4">
  {/* existing content */}
</Animated.View>
```

- [ ] **Step 2: Verify typecheck**

```bash
cd apps/mobile && pnpm typecheck
```

- [ ] **Step 3: Commit**

```bash
git add apps/mobile/src/routes/
git commit -s "feat(mobile): add page entrance animations via reanimated"
```

---

### Task 20: Improve mobile empty states and loading

**Files:**
- Modify: `apps/mobile/components/ui/empty-state.tsx`

- [ ] **Step 1: Update empty state icon styling**

Match the web EmptyState treatment: accent ring around icon container.

- [ ] **Step 2: Add skeleton component for mobile**

Create or update `apps/mobile/components/ui/skeleton.tsx` with a shimmer animation using reanimated.

- [ ] **Step 3: Verify typecheck**

```bash
cd apps/mobile && pnpm typecheck
```

- [ ] **Step 4: Commit**

```bash
git add apps/mobile/components/ui/
git commit -s "feat(mobile): refine empty states and add skeleton component"
```

---

## Chunk 8: Desktop Client — GPUI Token Alignment

Align the desktop GPUI client's visual tokens with the shared Vault Ledger world.

### Task 21: Update desktop GPUI theme tokens

**Files:**
- Identify desktop theme/color files in `apps/desktop/src/`

- [ ] **Step 1: Read the desktop theme system**

```bash
find apps/desktop/src -name "*theme*" -o -name "*color*" -o -name "*token*" | head -20
```

- [ ] **Step 2: Update color values to match shared tokens**

Map the new token values (`surface-hover`, `surface-active`, `sidebar-item-hover`, etc.) into the GPUI theme system.

- [ ] **Step 3: Build and verify**

```bash
cargo check -p vautr-desktop
```

- [ ] **Step 4: Commit**

```bash
git add apps/desktop/src/
git commit -s "feat(desktop): align GPUI theme tokens with Vault Ledger world"
```

---

## Chunk 9: Full Build Gate & Verification

### Task 22: Run full build and typecheck gate

- [ ] **Step 1: Run full workspace typecheck**

```bash
pnpm typecheck
```

Expected: No errors across all packages.

- [ ] **Step 2: Run full workspace build**

```bash
pnpm build
```

Expected: All builds succeed.

- [ ] **Step 3: Run Rust workspace check**

```bash
cargo check --workspace
```

Expected: No errors.

- [ ] **Step 4: Run lint**

```bash
pnpm lint
```

Expected: Clean.

- [ ] **Step 5: Run format**

```bash
pnpm format
```

Expected: No changes needed.

- [ ] **Step 6: Commit any formatting fixes**

```bash
git add -A
git commit -s "chore: format after UI/UX upgrade"
```

---

## Summary of Changes

| Area | What Changes | Files Touched |
|------|-------------|---------------|
| **Tokens** | New hover, focus, motion, skeleton tokens | `packages/design-tokens/` |
| **Web CSS** | Motion utility classes, skeleton shimmer | `apps/web/src/index.css` |
| **Sidebar** | Grouped nav sections, active indicator | `apps/web/src/routes/_authed.tsx` |
| **Login** | Vault-entry brand experience | `apps/web/src/routes/login.tsx`, `register.tsx` |
| **Dashboard** | Card hover, loading skeletons | `apps/web/src/routes/_authed/dashboard.tsx` |
| **Vault** | Better selection states, press feedback | `apps/web/src/components/VaultList.tsx`, `VaultView.tsx` |
| **Generator** | Press feedback, animated strength bar | `apps/web/src/routes/_authed/generator.tsx` |
| **Secrets** | Copy flash, press feedback | `apps/web/src/routes/_authed/secrets.tsx` |
| **Empty states** | Accent ring icon treatment | `apps/web/src/components/EmptyState.tsx` |
| **Sync indicator** | Pulse animation | `apps/web/src/components/SyncIndicator.tsx` |
| **All pages** | Page entrance animation | All `_authed/*.tsx` routes |
| **Extension** | Motion classes, vault transitions | `apps/extension/` CSS + components |
| **Mobile** | NativeWind tokens, reanimated entrance | `apps/mobile/` config + routes |
| **Desktop** | GPUI token alignment | `apps/desktop/src/` |

**Principles preserved:**
- Zero-knowledge semantics unchanged
- Information architecture unchanged
- Functional behavior unchanged
- One coherent visual world across all clients
- Dark is default; light is first-class
- Monospace reserved for real data
- Hairline structure, not shadows/glow
