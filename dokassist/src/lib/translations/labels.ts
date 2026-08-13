import { derived, type Readable } from 'svelte/store';
import { parseError } from '$lib/api';
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
 * The same lookup starting from a thrown value, for the common
 * `catch (e) { error = … }` shape.
 *
 * Backend messages are English (see src-tauri/src/error.rs), so showing one
 * raw puts English in a German UI. Translated copy for the error's code wins;
 * otherwise the backend message is still shown, because it carries detail the
 * generic text does not. `fallback` covers a throw that isn't an AppError.
 */
export const errorText: Readable<(thrown: unknown, fallback?: string) => string> = derived(
  errorMessage,
  ($errorMessage) => (thrown: unknown, fallback?: string) => {
    const { code, message } = parseError(thrown);
    // parseError stringifies a non-AppError throw, which prefixes "Error: ";
    // the instance's own message is what these call sites showed before.
    const detail = thrown instanceof Error ? thrown.message : message;
    return $errorMessage(code, detail || fallback || '');
  }
);

/**
 * Outcome score interpretations are computed and stored by the backend as
 * English title-case strings ("Moderately Severe"), and the severity styling
 * matches on that raw value — so only the display label is translated.
 */
export const interpretationLabel = labelStore('outcomeScores.interpretationValues');
