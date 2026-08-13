#!/usr/bin/env node
/**
 * Fails if a view reintroduces a raw Tailwind palette utility, a dark: colour
 * variant, or a radius/shadow from Tailwind's default scale. All of them
 * hardcode a decision at the call site, which is exactly the duplication the
 * token layer in src/app.css exists to remove.
 *
 * Run: node scripts/check-design-tokens.mjs
 * See: docs/design-language.md
 */
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const SRC = join(ROOT, 'src');

const HUES =
  'gray|slate|zinc|neutral|stone|blue|indigo|sky|cyan|purple|violet|fuchsia|pink|red|rose|green|emerald|teal|lime|amber|yellow|orange';
const PROPS =
  'bg|text|border|ring|divide|placeholder|from|via|to|outline|decoration|caret|fill|stroke';

// Deliberately property-agnostic: match ANY utility ending in -<hue>-<shade>,
// whatever prefixes it. Enumerating properties previously let compound ones
// through — `ring-offset-gray-900` is not `ring-<hue>`, so two real violations
// survived the first sweep.
const RAW_PALETTE = new RegExp(
  `(?:^|[\\s"'\`{}])[\\w:./-]*?-(?:${HUES})-(?:50|[1-9]00|950)(?:/\\d{1,3})?(?![\\w-])`,
  'g'
);
// Raw white/black are equally theme-hardcoded. Scrims are the one honest use,
// so an opacity modifier is required — bare bg-black is almost always a bug
// (Tailwind v4 dropped bg-opacity-*, which silently turns a scrim opaque).
const RAW_NEUTRAL = new RegExp(`(?:^|[\\s"'\`{}])[\\w:./-]*?-(?:white|black)(?![\\w/-])`, 'g');
const DEAD_OPACITY = /(?:^|[\s"'`{}])(?:bg|text|border|ring|divide)-opacity-\d+(?![\w-])/g;
const DARK_VARIANT = new RegExp(
  `(?:^|[\\s"'\`{}])dark:(?!prose-invert)(?:[a-z-]+:)*(?:${PROPS})-`,
  'g'
);
// Geometry is two radii — rounded-control and rounded-card — plus rounded-full
// for avatars and dots. Anything on Tailwind's default scale is a third radius,
// and the corner variants (rounded-bl-sm) are the ones that slip through review.
// Bare `rounded` counts: it silently means 4px.
const RAW_RADIUS =
  /(?:^|[\s"'`{}])rounded(?:-(?:[tblrse]|tl|tr|bl|br|ss|se|es|ee))?(?:-(?:none|sm|md|lg|xl|2xl|3xl|4xl))?(?![\w-])/g;
// Elevation is shadow-popover and shadow-modal. Nothing else, and never on
// inline content — cards and rows separate with a hairline border.
const RAW_SHADOW = /(?:^|[\s"'`{}])shadow(?:-(?:2xs|xs|sm|md|lg|xl|2xl|inner|none))?(?![\w-])/g;

// Narrow, deliberate exceptions, matched against the whole source line.
// Anything added here needs a stated reason.
const ALLOW = [
  // The modal scrim is a plain black wash whose opacity differs per theme;
  // tinting it with a surface token would make it read as a surface.
  { file: 'src/lib/components/ui/Dialog.svelte', line: /bg-black\/\d+ dark:bg-black\/\d+/ },
];

function walk(dir) {
  return readdirSync(dir).flatMap((entry) => {
    const full = join(dir, entry);
    return statSync(full).isDirectory() ? walk(full) : [full];
  });
}

const violations = [];

for (const file of walk(SRC)) {
  if (!/\.(svelte|ts)$/.test(file)) continue;
  const rel = relative(ROOT, file);
  // The token definitions themselves, and the doc comment describing what not
  // to write, necessarily name raw utilities.
  if (rel === 'src/app.css' || rel === 'src/lib/components/ui/index.ts') continue;
  if (rel.startsWith('src/tests/')) continue;

  const lines = readFileSync(file, 'utf8').split('\n');
  lines.forEach((line, i) => {
    // Comments necessarily name the utilities they tell you not to write. `/*`
    // matters as much as `*` and `//`: it is the line the prose starts on.
    const trimmed = line.trimStart();
    if (trimmed.startsWith('*') || trimmed.startsWith('//') || trimmed.startsWith('/*')) return;
    for (const [label, re] of [
      ['raw palette utility', RAW_PALETTE],
      ['raw white/black', RAW_NEUTRAL],
      ['removed in Tailwind v4', DEAD_OPACITY],
      ['dark: colour variant', DARK_VARIANT],
      ['radius outside the two-radius scale', RAW_RADIUS],
      ['shadow outside the elevation tokens', RAW_SHADOW],
    ]) {
      for (const m of line.matchAll(re)) {
        if (ALLOW.some((a) => a.file === rel && a.line.test(line))) continue;
        violations.push(`${rel}:${i + 1}  ${label}: ${m[0].trim()}`);
      }
    }
  });
}

if (violations.length) {
  console.error(`\n✖ ${violations.length} design-token violation(s):\n`);
  for (const v of violations) console.error(`  ${v}`);
  console.error(`
Views must use the semantic tokens from src/app.css — bg-surface, bg-surface-raised,
text-fg, text-fg-muted, border-line, bg-accent, text-danger-fg, … — and compose the
primitives in src/lib/components/ui/. Tokens theme themselves, so a dark: colour
variant is never needed. Geometry is rounded-control / rounded-card (plus
rounded-full for avatars and dots) and elevation is shadow-popover / shadow-modal.
See docs/design-language.md.
`);
  process.exit(1);
}

console.log('✔ src/ uses only the colour, radius and elevation tokens from app.css');
