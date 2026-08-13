import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { language } from '$lib/stores/language';
import {
  sessionTypeLabel,
  reportTypeLabel,
  errorMessage,
  errorText,
} from '$lib/translations/labels';

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

describe('errorMessage', () => {
  it('translates a known AppError code', () => {
    expect(get(errorMessage)('PATIENT_NOT_FOUND', 'raw')).toBe(
      'The requested patient could not be found. They may have been deleted.'
    );
    language.set('de');
    expect(get(errorMessage)('PATIENT_NOT_FOUND', 'raw')).toMatch(/nicht gefunden/);
  });

  it('falls back to the supplied message for an unmapped code', () => {
    expect(get(errorMessage)('DB_ERROR', 'Database error: disk full')).toBe(
      'Database error: disk full'
    );
  });
});

describe('errorText', () => {
  it('translates a thrown AppError by its code', () => {
    const thrown = { code: 'AUTH_REQUIRED', message: 'auth required', ref: 'R1' };
    expect(get(errorText)(thrown)).toBe('Please unlock the application to continue.');
  });

  it('keeps the backend message for an unmapped code, since it carries detail', () => {
    const thrown = { code: 'DB_ERROR', message: 'Database error: disk full', ref: 'R2' };
    expect(get(errorText)(thrown)).toBe('Database error: disk full');
  });

  it("uses an Error's own message rather than its stringified form", () => {
    // parseError stringifies a non-AppError throw, which would prefix "Error: ".
    expect(get(errorText)(new Error('Boom'))).toBe('Boom');
  });

  it('falls back to the supplied text when the throw carries no message', () => {
    expect(get(errorText)({ code: 'DB_ERROR', message: '', ref: 'R3' }, 'Could not load')).toBe(
      'Could not load'
    );
  });
});
