# Component Primitives Canon

Canonical primitive set and variant contracts shared by every Vautr client.
Companion to `design-system.md` (tokens) and `navigation.md` (IA). This is the
single source of truth for *which* primitives exist and *how* their variants
behave. Removing a primitive or a variant requires a token-level justification,
not a one-off.

## Primitive list (canonical)

`Button`, `Input`, `Textarea`, `Label`, `Card` (with `Header / Title /
Description / Content / Footer`), `Badge`, `Switch`, `Select`, `Tabs`, `Dialog`,
`DropdownMenu`, `Tooltip`, `Separator`, `Table`, `Toast` (sonner).

### Per-client status (2026-08-12)

| Primitive      | extension | web | mobile | desktop        |
| -------------- | --------- | --- | ------ | -------------- |
| Button         | ✓         | ✓   | ✓      | gpui_component ✓ |
| Input          | ✓         | ✓   | ✓      | gpui_component ✓ |
| Textarea       | ✓         | ✓   | —      | gpui_component ✓ |
| Label          | ✓         | ✓   | ✓      | gpui_component ✓ |
| Card (+parts)  | ✓         | ✓   | ✓      | gpui_component ✓ |
| Badge          | ✓         | ✓   | ✓      | gpui_component ✓ |
| Switch         | ✓         | ✓   | ✓      | gpui_component ✓ |
| Select         | ✓         | ✓   | ✓      | gpui_component ✓ |
| Tabs           | ✓         | ✓   | ✓      | gpui_component ✓ |
| Dialog         | ✓         | ✓   | Sheet¹ | gpui_component ✓ |
| DropdownMenu   | ✓         | ✓   | —      | gpui_component ✓ |
| Tooltip        | ✓         | ✓   | —      | gpui_component ✓ |
| Separator      | ✓         | ✓   | —      | gpui_component ✓ |
| Table          | ✓         | ✓   | ✓      | gpui_component ✓ |
| Toast (sonner) | ✓ (sonner)| ✓   | ✓ (toast) | inline status² |

¹ Mobile ships `Sheet` (bottom sheet) as its modal surface; a centered `Dialog`
  primitive is a follow-up. ² Desktop renders form/status inline rather than a
  global toast; a single toast system is a Phase 4 item.

## Button variant contract (canonical)

Adopted from the extension's `components/ui/button.tsx`; mobile's
`buttonVariants` already mirrors it (incl. `link`). Desktop's gpui_component
`Button` maps as: `.primary()` → `default`, `.destructive()` → `destructive`,
`.outline()` → `outline`, `.ghost()` → `ghost`.

- **variants**: `default` (primary action), `destructive`, `outline`,
  `secondary`, `ghost`, `link`.
- **sizes**: `default`, `sm`, `lg`, `icon`, `icon-sm`, `icon-lg`.

Every form's submit action uses `variant="default"` (primary); secondary/
cancel actions use `variant="ghost"` or `variant="outline"`.

## Card composition contract (canonical)

A create/edit form MUST compose as:

```
Card
 ├─ CardHeader
 │   ├─ CardTitle
 │   └─ CardDescription   (optional)
 ├─ CardContent
 │   └─ Label + Input (× N)
 └─ CardFooter
     ├─ Button variant="default"   (primary / submit)
     └─ Button variant="ghost"     (cancel)
```

Implemented on mobile (`_app.projects.new.tsx`), extension (`Dialog` modals),
and web. Desktop's inline forms are slated to move to `Dialog` in Phase 4.

## Iconography canon

One icon set: **Lucide** (`lucide-react` on web/extension, `lucide-react-native`
on mobile; `gpui_component::IconName` already maps to Lucide glyphs on desktop).
Canonical icon → section mapping (do not diverge per client):

| Section           | Icon          |
| ----------------- | ------------- |
| Dashboard         | `LayoutDashboard` |
| Projects          | `Folder`      |
| Vault             | `Eye`         |
| Generator         | `Settings2`   |
| Secrets           | `HardDrive`   |
| Machine accounts  | `Bot`         |
| Tokens            | `Globe`       |
| MFA & security    | `CircleCheck` |
| Import / export   | `Replace`     |
| Settings          | `Settings`    |

Applied across web (`_authed.tsx` NAV), mobile (`_app.tsx` PRIMARY + MORE),
and extension (`App.tsx` TABS) as of 2026-08-12. Avoid per-client ad-hoc glyphs
(`KeyRound`, `Wrench`, `Lock`, `Ticket`, `ShieldCheck`, `ArrowLeftRight`) for
these sections.
