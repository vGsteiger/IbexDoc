import { derived, type Readable } from 'svelte/store';
import { t } from './index';

/**
 * Session and report types are *persisted* values, not labels. `report_type`
 * is matched as a string against the Rust `ReportType` enum in
 * `llm/tools.rs`, and `session_type` is validated against a whitelist there,
 * so the stored form has to stay German in both languages. Only the display
 * label is translated.
 *
 * Both helpers fall back to the raw value: session types can be typed freely
 * in the session editor, and older rows hold values that were never in any
 * picker.
 */
function labelStore(namespace: string): Readable<(value: string) => string> {
  return derived(t, ($t) => (value: string) => {
    if (!value) return value;
    const key = `${namespace}.${value}`;
    const label = $t(key);
    // getNestedValue returns the key itself when there is no translation.
    return label === key ? value : label;
  });
}

export const sessionTypeLabel = labelStore('sessions.types');
export const reportTypeLabel = labelStore('reports.typeNames');

/**
 * Treatment plan statuses and intervention types are stored as stable slugs
 * ('in_progress', 'psychotherapy'), which were previously rendered raw.
 */
export const planStatusLabel = labelStore('treatmentPlans.status');
export const goalStatusLabel = labelStore('treatmentPlans.goalStatus');
export const interventionTypeLabel = labelStore('treatmentPlans.interventionType');

/**
 * Turns an AppError code into a readable message, falling back to the raw
 * backend message for codes without translated copy. Mirrors the set handled
 * by `getUserFriendlyMessage` in $lib/api, which stays for non-UI callers.
 */
export const errorMessage: Readable<(code: string, fallback: string) => string> = derived(
  t,
  ($t) => (code: string, fallback: string) => {
    const key = `errors.codes.${code}`;
    const message = $t(key);
    return message === key ? fallback : message;
  }
);

/**
 * Outcome score interpretations are computed and stored by the backend as
 * English title-case strings ("Moderately Severe"), and the severity styling
 * matches on that raw value — so only the display label is translated.
 */
export const interpretationLabel = labelStore('outcomeScores.interpretationValues');
