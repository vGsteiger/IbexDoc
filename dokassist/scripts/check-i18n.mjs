#!/usr/bin/env node
/**
 * Guards the translation layer:
 *
 *   1. de.json and en.json hold exactly the same keys, and every leaf is a
 *      string. `getNestedValue` in $lib/translations only returns strings, so
 *      an array or number leaf is silently unreachable.
 *   2. No new hardcoded user-visible string in .svelte markup. Text nodes and
 *      the user-visible attributes have to come from `$t(...)`.
 *
 * The second check is a heuristic, so genuinely untranslatable literals are
 * listed in ALLOW below with a stated reason. Anything not listed fails.
 *
 * Run: node scripts/check-i18n.mjs
 */
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;
const SRC = join(ROOT, 'src');

/* ------------------------------------------------------------------ *
 * Untranslatable by nature. Each entry needs a reason.
 * ------------------------------------------------------------------ */
const ALLOW = [
  // Language names are always written in their own language.
  'English',
  'Deutsch',
  'Français',
  // Product name.
  'RamDoc',
  // Keyboard shortcuts are not localised.
  'Cmd+K',
  'Cmd+N',
  'Cmd+Shift+S',
  'Esc',
  // The literal word the user must type; the handler string-compares it.
  'RESTORE',
  // Model identifier.
  'nomic-embed-text-v1.5',
  // Input format hints, not prose.
  '+41 XX XXX XX XX',
  '756.____.____.__',
  'practice@example.com',
  'recipient@example.com',
  // The required-field marker rendered by ui/Field.
  '&nbsp;*',
];

// Text that carries no words to translate: punctuation, symbols, numbers,
// interpolations, short acronyms, and anything that looks like a CSS class.
const NOISE =
  /^(?:[\s—•·▸▾▶▼●✓→←↑↓↵|/\\:;,.\-–+*#%()[\]{}0-9]+|[A-Z]{2,5}|https?:.*|\{.*\}|[a-z-]+(?:\s[a-z-]+)*=.*)$/;

const ATTRS = /\b(placeholder|aria-label|title|alt|label|description|hint)="([^"{}]+)"/g;

function walk(dir) {
  return readdirSync(dir).flatMap((entry) => {
    const full = join(dir, entry);
    return statSync(full).isDirectory() ? walk(full) : [full];
  });
}

function flatten(obj, prefix = '') {
  return Object.entries(obj).flatMap(([key, value]) =>
    // Plain objects are namespaces and get walked. Arrays must NOT be — an
    // array leaf would otherwise flatten into `key.0`, `key.1`, … and look
    // valid, when getNestedValue cannot reach it at all.
    value && typeof value === 'object' && !Array.isArray(value)
      ? flatten(value, `${prefix}${key}.`)
      : [[`${prefix}${key}`, value]]
  );
}

const problems = [];

/* ---------------------------- 1. bundle parity ---------------------------- */
const de = JSON.parse(readFileSync(join(SRC, 'lib/translations/de.json'), 'utf8'));
const en = JSON.parse(readFileSync(join(SRC, 'lib/translations/en.json'), 'utf8'));
const deEntries = flatten(de);
const enEntries = flatten(en);
const deKeys = new Set(deEntries.map(([k]) => k));
const enKeys = new Set(enEntries.map(([k]) => k));

for (const key of deKeys) {
  if (!enKeys.has(key)) problems.push(`en.json is missing key: ${key}`);
}
for (const key of enKeys) {
  if (!deKeys.has(key)) problems.push(`de.json is missing key: ${key}`);
}
for (const [lang, entries] of [
  ['de', deEntries],
  ['en', enEntries],
]) {
  for (const [key, value] of entries) {
    if (typeof value !== 'string') {
      const kind = Array.isArray(value) ? 'an array' : `a ${typeof value}`;
      problems.push(`${lang}.json: ${key} is ${kind}, but only strings are reachable`);
    }
  }
}

/* ------------------------ 2. hardcoded UI strings ------------------------ */
for (const file of walk(SRC)) {
  if (!file.endsWith('.svelte')) continue;
  const rel = relative(ROOT, file);
  if (rel.startsWith('src/tests/')) continue;

  const markup = readFileSync(file, 'utf8')
    // Blank out script/style but keep line numbering intact.
    .replace(/<script[\s\S]*?<\/script>/g, (m) => '\n'.repeat(m.split('\n').length - 1))
    .replace(/<style[\s\S]*?<\/style>/g, (m) => '\n'.repeat(m.split('\n').length - 1))
    .replace(/<!--[\s\S]*?-->/g, '');

  const report = (line, kind, text) => {
    const value = text.replace(/\s+/g, ' ').trim();
    if (value.length < 3) return;
    if (!/[A-Za-zÄÖÜäöü]{3}/.test(value)) return;
    if (NOISE.test(value)) return;
    if (ALLOW.includes(value)) return;
    problems.push(`${rel}:${line}  hardcoded ${kind}: ${value}`);
  };

  for (const m of markup.matchAll(/>([^<>{}]+)</g)) {
    report(markup.slice(0, m.index).split('\n').length, 'text', m[1]);
  }
  markup.split('\n').forEach((line, i) => {
    for (const m of line.matchAll(ATTRS)) report(i + 1, m[1], m[2]);
  });
}

if (problems.length) {
  console.error(`\n✖ ${problems.length} i18n problem(s):\n`);
  for (const p of problems) console.error(`  ${p}`);
  console.error(`
User-visible text belongs in src/lib/translations/{de,en}.json and is read with
$t('namespace.key') — see the existing namespaces for where a new key fits.
Values that are persisted (session types, report types, status slugs) keep their
stored form and are translated for display only; see src/lib/translations/labels.ts.
If a literal genuinely cannot be translated, add it to ALLOW in
scripts/check-i18n.mjs with a reason.
`);
  process.exit(1);
}

console.log(`✔ de/en at ${deKeys.size} keys with no drift, and no hardcoded UI strings in src/`);
