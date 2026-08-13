import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { language } from '$lib/stores/language';
import { sessionTypeLabel, reportTypeLabel } from '$lib/translations/labels';

beforeEach(() => {
  language.set('en');
});

describe('sessionTypeLabel', () => {
  it('translates a stored German value for display', () => {
    expect(get(sessionTypeLabel)('Erstgespräch')).toBe('Initial consultation');
    language.set('de');
    expect(get(sessionTypeLabel)('Erstgespräch')).toBe('Erstgespräch');
  });

  it('covers every value the session picker can store', () => {
    // Mirrors sessionTypes in routes/patients/[id]/sessions/new/+page.svelte.
    const picker = [
      'Erstgespräch',
      'Verlaufskontrolle',
      'Krisenintervention',
      'Psychotherapie',
      'Medikamentenanpassung',
      'Andere',
    ];
    for (const value of picker) {
      expect(get(sessionTypeLabel)(value)).not.toBe(value);
    }
  });

  it('covers every value the LLM tool whitelist can store', () => {
    // Mirrors allowed_types in src-tauri/src/llm/tools.rs.
    const whitelist = [
      'Erstgespräch',
      'Einzeltherapie',
      'Gruppentherapie',
      'Diagnostik',
      'Verlaufskontrolle',
      'Abschlussgespräch',
      'Krisenintervention',
      'Konsultation',
    ];
    for (const value of whitelist) {
      expect(get(sessionTypeLabel)(value)).not.toBe(value);
    }
  });

  it('falls back to the raw value for free-text and legacy types', () => {
    // The session editor takes session_type as a free-text input, so unknown
    // values must still render rather than leaking a translation key.
    expect(get(sessionTypeLabel)('Initial Assessment')).toBe('Initial Assessment');
    expect(get(sessionTypeLabel)('Hausbesuch')).toBe('Hausbesuch');
  });

  it('passes an empty value straight through', () => {
    expect(get(sessionTypeLabel)('')).toBe('');
  });
});

describe('reportTypeLabel', () => {
  it('translates the three stored report types', () => {
    expect(get(reportTypeLabel)('Befundbericht')).toBe('Assessment report');
    expect(get(reportTypeLabel)('Verlaufsbericht')).toBe('Progress report');
    expect(get(reportTypeLabel)('Ueberweisungsschreiben')).toBe('Referral letter');
  });

  it('renders the umlaut form in German while the stored value stays ASCII', () => {
    language.set('de');
    expect(get(reportTypeLabel)('Ueberweisungsschreiben')).toBe('Überweisungsschreiben');
  });

  it('falls back to the raw value for an unknown type', () => {
    expect(get(reportTypeLabel)('Kurzbericht')).toBe('Kurzbericht');
  });
});
