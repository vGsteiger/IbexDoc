# RamDoc design language

RamDoc is a clinical record system. It should read like one: quiet, dense,
legible, and boring in the way good tools are boring. The reference points are
Notion and Linear — near-neutral surfaces, hairline separation, a scarce accent,
and a deliberate type scale.

The rule that makes this hold: **views never name a colour.** They compose the
primitives in `src/lib/components/ui/` and the semantic token utilities defined
in `src/app.css`. A raw palette class such as `bg-gray-100` or `text-blue-600`
hardcodes a theme at the call site, which is what previously required 3420
colour decisions and 1330 `dark:` variants to stay in sync by hand.

`pnpm check:design` enforces this and runs in CI.

---

## 1. Tokens

All tokens are CSS custom properties on `:root`, re-declared on `.dark`, and
exposed to Tailwind through `@theme inline`. Because the block is `inline`,
every generated utility resolves `var()` at use time — so toggling `dark` on
`<html>` reheats the entire UI and **no `dark:` colour variant is ever needed**.

Theme switching is therefore one line, in `src/routes/+layout.svelte`:

```js
document.documentElement.classList.toggle('dark', $resolvedTheme === 'dark');
```

### Surfaces

| Utility                 | Role                                              |
| ----------------------- | ------------------------------------------------- |
| `bg-surface`            | The page. Warm off-white / near-black.            |
| `bg-surface-raised`     | Cards, inputs, anything sitting on the page.      |
| `bg-surface-overlay`    | Dialogs, popovers, dropdowns, toasts.             |
| `bg-surface-sunken`     | Sidebar, wells, recessed regions.                 |
| `bg-surface-hover`      | Hover state, and quiet chips.                     |
| `bg-surface-selected`   | Selected row, active nav item, pressed state.     |

### Lines

`border-line-subtle` · `border-line` · `border-line-strong`

Hairlines are the primary separation mechanism. Reach for a border before a
shadow, and before a filled surface.

### Foreground

`text-fg` (primary) · `text-fg-muted` (secondary) · `text-fg-subtle` (metadata,
icons) · `text-fg-disabled`

### Accent

`bg-accent` · `bg-accent-hover` · `bg-accent-subtle` · `border-accent` ·
`border-accent-line` · `text-accent-fg` · `text-on-accent`

**The accent is scarce.** It appears only on:

- the single primary action in a view
- links
- focus rings
- selection indicators (e.g. the 2px bar on the active nav item)

It does **not** fill nav rows, headers, cards, badges-for-decoration, or a
second button. If two things on screen are accent-coloured, one of them is
probably wrong.

### Status

`danger` · `success` · `warning` · `info`, each with `bg-<tone>`,
`bg-<tone>-subtle`, `border-<tone>-line`, `text-<tone>-fg`, `text-on-<tone>`.

Colour carries meaning here and nowhere else. A tinted subtle surface plus a
matching icon (see `Alert`) beats a saturated block.

---

## 2. Type

Six steps. Hierarchy comes from size and colour, not weight — `font-bold` is
retired from UI chrome.

| Utility        | Size | Use                                          |
| -------------- | ---- | -------------------------------------------- |
| `text-display` | 26px | One per page: the page title.                |
| `text-title`   | 20px | Section and card titles.                     |
| `text-heading` | 16px | Sub-headings, emphasised rows.               |
| `text-body`    | 14px | The default.                                 |
| `text-label`   | 13px | Form labels, dense table cells.              |
| `text-caption` | 12px | Metadata, hints, timestamps.                 |

Weight stops at `font-semibold` (headings) and `font-medium` (emphasis).
Tabular figures are on by default for `table`, `time`, and `[data-numeric]`, so
dosages and scores align in columns.

---

## 3. Geometry

**Two radii.** `rounded-control` (6px) for buttons, inputs, selects, chips.
`rounded-card` (10px) for cards, dialogs, popovers. `rounded-full` is reserved
for avatars and dot indicators.

**Two control heights.** 32px (`h-8`, default) and 28px (`h-7`, dense). The
primitives encode these; don't hand-roll `px-4 py-2`.

**Spacing** follows the Tailwind 4px scale, kept compact — this is a
list-and-record app, not a marketing page.

---

## 4. Elevation

`shadow-popover` for dropdowns and toasts, `shadow-modal` for dialogs. That is
the complete set. Inline content — cards, list rows, panels — gets a border, not
a shadow. Both tokens are theme-aware, so never pair them with a `dark:` variant.

---

## 5. Motion

120–160ms, ease-out (`duration-150 ease-standard`), on **colour and opacity
only**. No scale, bounce, or layout transitions. `prefers-reduced-motion` is
honoured globally in `app.css`.

---

## 6. Focus

One treatment for the whole app, set once in `app.css`:

```css
:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
```

Form controls additionally take an accent border and a translucent accent ring.
Never remove a focus indicator without replacing it.

---

## 7. Primitives

From `$lib/components/ui`:

`Alert` · `Badge` · `Button` · `Card` · `Dialog` · `EmptyState` · `Field` ·
`IconButton` · `Input` · `Kbd` · `PageHeader` · `Select` · `Spinner` · `Textarea`

```svelte
<script lang="ts">
  import { Button, Card, Field, Input, PageHeader } from '$lib/components/ui';
</script>

<PageHeader title="Patients" description="All active records.">
  {#snippet actions()}
    <Button variant="primary">New patient</Button>
  {/snippet}
</PageHeader>

<Card>
  <Field label="Family name" required>
    <Input bind:value={name} />
  </Field>
</Card>
```

`Button` variants: `primary` (one per view), `secondary` (the default),
`ghost`, `subtle`, `danger`.

`Button` and `IconButton` render an `<a>` when given `href`. An anchor is never
`:disabled`, so Tailwind's `disabled:` variants do not apply to that branch —
both components instead attach `pointer-events-none opacity-50` directly, plus
`aria-disabled` and `tabindex="-1"`. The `href` is deliberately kept so the
element keeps its link role for assistive tech. If you hand-roll a disabled
link anywhere, do the same.

Note: `Select` renders a native `<select>`; pass an initial `value` that matches
one of the options, or it will show blank rather than defaulting to the first.

---

## 8. Not the AI app

RamDoc uses local language models, and that should feel like an ordinary
capability of the tool rather than its personality. No robot or brain
iconography, no sparkles, no purple "AI" buttons, no emoji in UI chrome.
Generated content is marked with a plain label and, where it helps, a subtle
surface tint — the same vocabulary as everything else.

---

## 9. Adding to the system

1. Reach for an existing token. Almost always there is one.
2. If genuinely missing, add it to **both** `:root` and `.dark` in `app.css`,
   map it under `@theme inline`, and document it here.
3. If a pattern appears three times, it belongs in `src/lib/components/ui/`.
4. Run `pnpm check:design`.

Exceptions to the no-raw-colour rule live in the `ALLOW` list in
`scripts/check-design-tokens.mjs` and each needs a stated reason. There is
currently one: the modal scrim, a plain black wash whose opacity differs per
theme.
