//! Protected clinical spans (issue #403).
//!
//! Lossy summarisation of clinical text silently changes meaning: dropping a
//! negation flips a finding, dropping a unit changes a prescription, dropping a
//! date detaches an event from the timeline.  The detectors here mark the spans
//! that must survive verbatim in an assembled evidence prompt so the assembler
//! can refuse to cut a unit through one of them and can report which
//! protections it retained.
//!
//! All offsets are **character** offsets (Unicode scalar values) relative to
//! the text that was scanned, never byte offsets, so they stay valid when the
//! same text is re-sliced elsewhere.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// A class of span that must never be summarised away.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtectedKind {
    /// A medication or substance name.
    Medication,
    /// A dose, strength, or dosing schedule (`50 mg`, `1-0-1`, `2x täglich`).
    Dose,
    /// A calendar date or year.
    Date,
    /// A negation cue (`keine`, `denies`, `sans`).
    Negation,
    /// An uncertainty / hedging cue (`Verdacht auf`, `suspected`).
    Uncertainty,
    /// A risk statement (`Suizidalität`, `self-harm`, `overdose`).
    Risk,
    /// A provenance token: ICD-10 code, AHV number, record identifier.
    Provenance,
}

impl ProtectedKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Medication => "medication",
            Self::Dose => "dose",
            Self::Date => "date",
            Self::Negation => "negation",
            Self::Uncertainty => "uncertainty",
            Self::Risk => "risk",
            Self::Provenance => "provenance",
        }
    }
}

/// A protected span, in character offsets relative to the scanned text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtectedSpan {
    pub kind: ProtectedKind,
    pub start: usize,
    pub end: usize,
}

/// How a lexicon term is matched against text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatchMode {
    /// The term must be a complete word (`kein`, but not `keinesfalls`).
    Word,
    /// The term is a stem; trailing letters are absorbed (`suizid` →
    /// `Suizidalität`).
    Stem,
}

/// Negation cues (German, French, English).
const NEGATION_CUES: &[&str] = &[
    "kein",
    "keine",
    "keinen",
    "keinem",
    "keiner",
    "keinerlei",
    "nicht",
    "nie",
    "niemals",
    "ohne",
    "verneint",
    "negativ",
    "unauffällig",
    "no",
    "not",
    "none",
    "never",
    "without",
    "denies",
    "denied",
    "negative",
    "absent",
    "aucun",
    "aucune",
    "sans",
    "pas de",
    "ni",
    "non",
];

/// Uncertainty / hedging cues.
const UNCERTAINTY_CUES: &[&str] = &[
    "verdacht auf",
    "verdacht",
    "v.a.",
    "möglicherweise",
    "möglich",
    "vermutlich",
    "wahrscheinlich",
    "fraglich",
    "unklar",
    "eventuell",
    "könnte",
    "differentialdiagnostisch",
    "suspected",
    "suspicion",
    "possible",
    "possibly",
    "probable",
    "probably",
    "likely",
    "unclear",
    "rule out",
    "cannot exclude",
    "peut-être",
    "probablement",
    "suspicion de",
];

/// Risk statements. Matched as stems so inflections are covered.
const RISK_STEMS: &[&str] = &[
    "suizid",
    "suizidal",
    "selbstverletz",
    "selbstgefährd",
    "selbstschädig",
    "fremdgefährd",
    "fremdaggress",
    "eigengefährd",
    "überdos",
    "intoxikation",
    "zwangseinweis",
    "fürsorgerische unterbringung",
    "notfall",
    "krisenintervention",
    "suicid",
    "self-harm",
    "self harm",
    "overdose",
    "homicid",
    "violence",
    "emergency",
    "crisis",
    "risque suicidaire",
];

/// Substance stems that are protected even when the patient has no matching
/// medication row (for example a substance only mentioned in a session note).
const BUILTIN_SUBSTANCE_STEMS: &[&str] = &[
    "sertralin",
    "escitalopram",
    "citalopram",
    "fluoxetin",
    "paroxetin",
    "fluvoxamin",
    "venlafaxin",
    "duloxetin",
    "bupropion",
    "mirtazapin",
    "trazodon",
    "agomelatin",
    "amitriptylin",
    "nortriptylin",
    "clomipramin",
    "lithium",
    "lamotrigin",
    "valproat",
    "carbamazepin",
    "quetiapin",
    "olanzapin",
    "risperidon",
    "aripiprazol",
    "haloperidol",
    "clozapin",
    "paliperidon",
    "amisulprid",
    "lurasidon",
    "zuclopenthixol",
    "lorazepam",
    "diazepam",
    "oxazepam",
    "clonazepam",
    "alprazolam",
    "midazolam",
    "zolpidem",
    "zopiclon",
    "pregabalin",
    "gabapentin",
    "methylphenidat",
    "lisdexamfetamin",
    "atomoxetin",
    "melatonin",
    "naltrexon",
    "acamprosat",
    "buprenorphin",
    "methadon",
    "levothyroxin",
];

/// Dose units. The unit run is protected in full, so `mg/Tag` and `mg/kg` are
/// covered by the `mg` entry.
const DOSE_UNITS: &[&str] = &[
    "mg", "mcg", "µg", "ug", "g", "kg", "ml", "l", "dl", "iu", "ie", "mmol", "mval", "meq", "%",
];

/// Frequency words that extend a `2x` / `3 mal` dose span.
const FREQUENCY_WORDS: &[&str] = &[
    "täglich",
    "tgl",
    "tgl.",
    "wöchentlich",
    "monatlich",
    "pro",
    "tag",
    "woche",
    "monat",
    "daily",
    "weekly",
    "monthly",
    "per",
    "day",
    "week",
    "jour",
    "semaine",
];

/// German / French / English month names, used for `3. März 2026` style dates.
const MONTH_NAMES: &[&str] = &[
    "januar",
    "februar",
    "märz",
    "april",
    "mai",
    "juni",
    "juli",
    "august",
    "september",
    "oktober",
    "november",
    "dezember",
    "january",
    "february",
    "march",
    "may",
    "june",
    "july",
    "october",
    "december",
    "janvier",
    "février",
    "mars",
    "avril",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

/// The terms used to detect medication mentions.
///
/// Built from the patient's own medication rows (patient-scoped, so no other
/// patient's substances ever influence detection) plus a fixed list of common
/// substances.
#[derive(Debug, Clone, Default)]
pub struct ProtectionLexicon {
    substance_stems: Vec<Vec<char>>,
}

impl ProtectionLexicon {
    /// Lexicon containing only the built-in substance stems.
    pub fn builtin() -> Self {
        let mut lexicon = Self::default();
        for stem in BUILTIN_SUBSTANCE_STEMS {
            lexicon.push_stem(stem);
        }
        lexicon
    }

    /// Built-in stems plus the substances recorded for `patient_id`.
    ///
    /// The query is patient-scoped; substances of other patients are never
    /// loaded.
    pub fn for_patient(conn: &Connection, patient_id: &str) -> Result<Self, AppError> {
        let mut lexicon = Self::builtin();
        let mut stmt = conn.prepare("SELECT substance FROM medications WHERE patient_id = ?1")?;
        let rows = stmt
            .query_map([patient_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        for substance in rows {
            // Trade names are often written as "Zoloft 50 mg" — index each word.
            for word in substance.split(|c: char| !c.is_alphanumeric()) {
                lexicon.push_stem(word);
            }
        }
        Ok(lexicon)
    }

    fn push_stem(&mut self, stem: &str) {
        let stem = stem.trim();
        // Very short tokens produce false positives ("er", "mg", numbers).
        if stem.chars().count() < 4 || stem.chars().any(|c| c.is_ascii_digit()) {
            return;
        }
        let lowered = lowercase_chars(stem);
        if !self.substance_stems.contains(&lowered) {
            self.substance_stems.push(lowered);
        }
    }
}

/// Detect every protected span in `text`.
///
/// Returned spans are sorted by `(start, end)` and de-duplicated. Spans of
/// different kinds may overlap (a dose often sits next to a medication name);
/// that is intentional, both protections apply.
pub fn detect(text: &str, lexicon: &ProtectionLexicon) -> Vec<ProtectedSpan> {
    let chars: Vec<char> = text.chars().collect();
    let lower = lowercase_chars(text);
    let mut spans: Vec<ProtectedSpan> = Vec::new();

    scan_terms(
        &lower,
        NEGATION_CUES,
        MatchMode::Word,
        ProtectedKind::Negation,
        &mut spans,
    );
    scan_terms(
        &lower,
        UNCERTAINTY_CUES,
        MatchMode::Word,
        ProtectedKind::Uncertainty,
        &mut spans,
    );
    scan_terms(
        &lower,
        RISK_STEMS,
        MatchMode::Stem,
        ProtectedKind::Risk,
        &mut spans,
    );
    scan_substances(&lower, lexicon, &mut spans);
    scan_numeric(&chars, &lower, &mut spans);

    spans.sort_by_key(|span| (span.start, span.end, span.kind));
    spans.dedup();
    spans
}

/// Whether cutting the text at character offset `at` would split a protected
/// span. Cutting exactly at a span boundary is safe.
pub fn is_safe_cut(spans: &[ProtectedSpan], at: usize) -> bool {
    !spans.iter().any(|s| s.start < at && at < s.end)
}

/// The spans fully contained in `[start, end)`, re-based so their offsets are
/// relative to that window.
pub fn spans_within(spans: &[ProtectedSpan], start: usize, end: usize) -> Vec<ProtectedSpan> {
    spans
        .iter()
        .filter(|s| s.start >= start && s.end <= end)
        .map(|s| ProtectedSpan {
            kind: s.kind,
            start: s.start - start,
            end: s.end - start,
        })
        .collect()
}

/// Slice `text` by character offsets. Offsets outside the text are clamped.
pub fn char_slice(text: &str, start: usize, end: usize) -> String {
    text.chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect()
}

/// Lowercase `text` while preserving a 1:1 character mapping so offsets
/// computed on the lowercase form stay valid for the original.
fn lowercase_chars(text: &str) -> Vec<char> {
    text.chars()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .collect()
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric()
}

fn at_word_start(lower: &[char], pos: usize) -> bool {
    pos == 0 || !is_word_char(lower[pos - 1])
}

/// Match `term` at `pos`, returning the exclusive end offset on success.
fn match_at(lower: &[char], pos: usize, term: &[char], mode: MatchMode) -> Option<usize> {
    if pos + term.len() > lower.len() || !at_word_start(lower, pos) {
        return None;
    }
    if lower[pos..pos + term.len()] != *term {
        return None;
    }
    let mut end = pos + term.len();
    match mode {
        MatchMode::Word => {
            if end < lower.len() && is_word_char(lower[end]) {
                return None;
            }
        }
        MatchMode::Stem => {
            while end < lower.len() && lower[end].is_alphabetic() {
                end += 1;
            }
        }
    }
    Some(end)
}

fn scan_terms(
    lower: &[char],
    terms: &[&str],
    mode: MatchMode,
    kind: ProtectedKind,
    out: &mut Vec<ProtectedSpan>,
) {
    let prepared: Vec<Vec<char>> = terms.iter().map(|t| lowercase_chars(t)).collect();
    for pos in 0..lower.len() {
        if !at_word_start(lower, pos) {
            continue;
        }
        // Longest match wins so "verdacht auf" beats "verdacht".
        let mut best: Option<usize> = None;
        for term in &prepared {
            if let Some(end) = match_at(lower, pos, term, mode) {
                best = Some(best.map_or(end, |current: usize| current.max(end)));
            }
        }
        if let Some(end) = best {
            out.push(ProtectedSpan {
                kind,
                start: pos,
                end,
            });
        }
    }
}

fn scan_substances(lower: &[char], lexicon: &ProtectionLexicon, out: &mut Vec<ProtectedSpan>) {
    for pos in 0..lower.len() {
        if !at_word_start(lower, pos) {
            continue;
        }
        let mut best: Option<usize> = None;
        for stem in &lexicon.substance_stems {
            if let Some(end) = match_at(lower, pos, stem, MatchMode::Stem) {
                best = Some(best.map_or(end, |current: usize| current.max(end)));
            }
        }
        if let Some(end) = best {
            out.push(ProtectedSpan {
                kind: ProtectedKind::Medication,
                start: pos,
                end,
            });
        }
    }
}

/// Scan digit-led constructs: dates, identifiers, doses, dosing schemes.
fn scan_numeric(chars: &[char], lower: &[char], out: &mut Vec<ProtectedSpan>) {
    scan_icd_codes(lower, out);

    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() || !at_word_start(lower, i) {
            i += 1;
            continue;
        }
        let start = i;
        let digits_end = digit_run_end(chars, i);

        // Dotted / slashed dates and dotted identifiers (AHV, "756.1234.5678.97").
        if let Some((end, kind)) = match_separated_numeric(chars, start) {
            out.push(ProtectedSpan { kind, start, end });
            i = end;
            continue;
        }

        // ISO date 2026-03-04.
        if let Some(end) = match_iso_date(chars, start) {
            out.push(ProtectedSpan {
                kind: ProtectedKind::Date,
                start,
                end,
            });
            i = end;
            continue;
        }

        // Dosing scheme 1-0-1 (-0).
        if let Some(end) = match_dosing_scheme(chars, digits_end) {
            out.push(ProtectedSpan {
                kind: ProtectedKind::Dose,
                start,
                end,
            });
            i = end;
            continue;
        }

        // Decimal number, optionally followed by a unit or frequency.
        let number_end = match_decimal_end(chars, digits_end);
        if let Some(end) = match_dose_tail(chars, lower, number_end) {
            out.push(ProtectedSpan {
                kind: ProtectedKind::Dose,
                start,
                end,
            });
            i = end;
            continue;
        }

        // `3. März 2026` and bare years.
        if let Some(end) = match_month_name_date(chars, lower, number_end) {
            out.push(ProtectedSpan {
                kind: ProtectedKind::Date,
                start,
                end,
            });
            i = end;
            continue;
        }
        if number_end == digits_end && is_year(&chars[start..digits_end]) {
            out.push(ProtectedSpan {
                kind: ProtectedKind::Date,
                start,
                end: digits_end,
            });
        }

        i = number_end.max(start + 1);
    }
}

fn digit_run_end(chars: &[char], from: usize) -> usize {
    let mut end = from;
    while end < chars.len() && chars[end].is_ascii_digit() {
        end += 1;
    }
    end
}

fn match_decimal_end(chars: &[char], digits_end: usize) -> usize {
    if digits_end + 1 < chars.len()
        && (chars[digits_end] == '.' || chars[digits_end] == ',')
        && chars[digits_end + 1].is_ascii_digit()
    {
        return digit_run_end(chars, digits_end + 1);
    }
    digits_end
}

fn is_year(digits: &[char]) -> bool {
    if digits.len() != 4 {
        return false;
    }
    matches!(digits[0], '1' | '2') && matches!(digits[1], '0' | '9')
}

/// `1-0-1`, `1-0-1-0`: at least two `-<digits>` groups so plain ranges such as
/// `5-10` are treated as numbers, not schedules.
fn match_dosing_scheme(chars: &[char], digits_end: usize) -> Option<usize> {
    let mut end = digits_end;
    let mut groups = 0;
    while end + 1 < chars.len() && chars[end] == '-' && chars[end + 1].is_ascii_digit() {
        end = digit_run_end(chars, end + 1);
        groups += 1;
    }
    (groups >= 2).then_some(end)
}

fn match_iso_date(chars: &[char], start: usize) -> Option<usize> {
    let year_end = digit_run_end(chars, start);
    if year_end - start != 4 {
        return None;
    }
    let month_start = expect_char(chars, year_end, '-')?;
    let month_end = digit_run_end(chars, month_start);
    if !(1..=2).contains(&(month_end - month_start)) {
        return None;
    }
    let day_start = expect_char(chars, month_end, '-')?;
    let day_end = digit_run_end(chars, day_start);
    (1..=2).contains(&(day_end - day_start)).then_some(day_end)
}

/// Dotted or slashed numeric groups: `04.03.2026`, `04/03/2026` (dates) and
/// longer dotted runs such as AHV numbers (provenance).
fn match_separated_numeric(chars: &[char], start: usize) -> Option<(usize, ProtectedKind)> {
    for separator in ['.', '/'] {
        let mut end = digit_run_end(chars, start);
        let mut groups = 1;
        while let Some(next) = expect_char(chars, end, separator) {
            let group_end = digit_run_end(chars, next);
            if group_end == next {
                break;
            }
            end = group_end;
            groups += 1;
        }
        if groups >= 4 {
            return Some((end, ProtectedKind::Provenance));
        }
        if groups == 3 {
            return Some((end, ProtectedKind::Date));
        }
    }
    None
}

fn expect_char(chars: &[char], pos: usize, expected: char) -> Option<usize> {
    (pos < chars.len() && chars[pos] == expected).then_some(pos + 1)
}

/// A unit (`50 mg`, `2.5 ml/h`) or a frequency (`2x täglich`) after a number.
fn match_dose_tail(chars: &[char], lower: &[char], number_end: usize) -> Option<usize> {
    let mut pos = number_end;
    while pos < chars.len() && chars[pos] == ' ' {
        pos += 1;
    }
    if pos >= chars.len() {
        return None;
    }
    if chars[pos] == '%' {
        return Some(pos + 1);
    }

    let token_end = unit_token_end(chars, pos);
    if token_end == pos {
        return None;
    }
    let token: String = lower[pos..token_end].iter().collect();
    let head = token.split('/').next().unwrap_or(&token);

    if DOSE_UNITS.contains(&head) {
        return Some(token_end);
    }
    if head == "x" || head == "mal" {
        return Some(extend_frequency(chars, lower, token_end));
    }
    None
}

fn unit_token_end(chars: &[char], from: usize) -> usize {
    let mut end = from;
    while end < chars.len()
        && (chars[end].is_alphabetic() || chars[end] == '/' || chars[end] == 'µ')
    {
        end += 1;
    }
    end
}

fn extend_frequency(chars: &[char], lower: &[char], from: usize) -> usize {
    let mut pos = from;
    while pos < chars.len() && chars[pos] == ' ' {
        pos += 1;
    }
    let word_end = unit_token_end(chars, pos);
    let word: String = lower[pos..word_end].iter().collect();
    if FREQUENCY_WORDS.contains(&word.as_str()) {
        return word_end;
    }
    from
}

/// `3. März 2026` / `3 mars 2026`, starting from the end of the day number.
fn match_month_name_date(chars: &[char], lower: &[char], number_end: usize) -> Option<usize> {
    let mut pos = number_end;
    if pos < chars.len() && chars[pos] == '.' {
        pos += 1;
    }
    while pos < chars.len() && chars[pos] == ' ' {
        pos += 1;
    }
    let month_end = unit_token_end(chars, pos);
    let month: String = lower[pos..month_end].iter().collect();
    if !MONTH_NAMES.contains(&month.as_str()) {
        return None;
    }
    let mut year_pos = month_end;
    while year_pos < chars.len() && year_pos - month_end < 2 && chars[year_pos] == ' ' {
        year_pos += 1;
    }
    let year_end = digit_run_end(chars, year_pos);
    if is_year(&chars[year_pos..year_end]) {
        return Some(year_end);
    }
    Some(month_end)
}

/// ICD-10 style codes (`F32.1`, `Z00`) are provenance for a diagnosis claim.
fn scan_icd_codes(lower: &[char], out: &mut Vec<ProtectedSpan>) {
    let mut i = 0;
    while i < lower.len() {
        if !at_word_start(lower, i) || !lower[i].is_ascii_alphabetic() {
            i += 1;
            continue;
        }
        let digits_start = i + 1;
        let digits_end = digit_run_end_lower(lower, digits_start);
        if digits_end - digits_start != 2 {
            i += 1;
            continue;
        }
        let mut end = digits_end;
        if end + 1 < lower.len() && lower[end] == '.' && lower[end + 1].is_ascii_digit() {
            end = digit_run_end_lower(lower, end + 1);
        }
        if end < lower.len() && is_word_char(lower[end]) {
            i += 1;
            continue;
        }
        out.push(ProtectedSpan {
            kind: ProtectedKind::Provenance,
            start: i,
            end,
        });
        i = end;
    }
}

fn digit_run_end_lower(lower: &[char], from: usize) -> usize {
    let mut end = from;
    while end < lower.len() && lower[end].is_ascii_digit() {
        end += 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds_at(text: &str, needle: &str) -> Vec<ProtectedKind> {
        let spans = detect(text, &ProtectionLexicon::builtin());
        let start = text.chars().collect::<Vec<_>>();
        let needle_chars: Vec<char> = needle.chars().collect();
        let offset = (0..start.len())
            .find(|&i| start[i..].starts_with(&needle_chars[..]))
            .expect("needle must occur in text");
        spans
            .iter()
            .filter(|s| s.start == offset && s.end == offset + needle_chars.len())
            .map(|s| s.kind)
            .collect()
    }

    #[test]
    fn detects_doses_with_units_and_schedules() {
        assert_eq!(
            kinds_at("Sertralin 50 mg täglich", "50 mg"),
            vec![ProtectedKind::Dose]
        );
        assert_eq!(
            kinds_at("Quetiapin 1-0-1", "1-0-1"),
            vec![ProtectedKind::Dose]
        );
        assert_eq!(
            kinds_at("Lorazepam 2x täglich", "2x täglich"),
            vec![ProtectedKind::Dose]
        );
        assert_eq!(
            kinds_at("Dosis 12.5 mg/Tag", "12.5 mg/Tag"),
            vec![ProtectedKind::Dose]
        );
        assert_eq!(
            kinds_at("Reduktion um 25%", "25%"),
            vec![ProtectedKind::Dose]
        );
    }

    #[test]
    fn detects_dates_in_swiss_and_iso_formats() {
        assert_eq!(
            kinds_at("Sitzung am 04.03.2026", "04.03.2026"),
            vec![ProtectedKind::Date]
        );
        assert_eq!(
            kinds_at("Visit on 2026-03-04", "2026-03-04"),
            vec![ProtectedKind::Date]
        );
        assert_eq!(
            kinds_at("Seit 3. März 2026 stabil", "3. März 2026"),
            vec![ProtectedKind::Date]
        );
        assert_eq!(
            kinds_at("Erstkontakt 2019", "2019"),
            vec![ProtectedKind::Date]
        );
    }

    #[test]
    fn detects_negation_and_uncertainty_cues() {
        assert_eq!(
            kinds_at("keine Suizidgedanken", "keine"),
            vec![ProtectedKind::Negation]
        );
        assert_eq!(
            kinds_at("Patient denies intent", "denies"),
            vec![ProtectedKind::Negation]
        );
        assert_eq!(
            kinds_at("Verdacht auf bipolare Störung", "Verdacht auf"),
            vec![ProtectedKind::Uncertainty]
        );
        // A cue must be a whole word, not a prefix of another word.
        let spans = detect("keinesfalls dokumentiert", &ProtectionLexicon::builtin());
        assert!(!spans.iter().any(|s| s.kind == ProtectedKind::Negation));
    }

    #[test]
    fn detects_risk_stems_with_inflections() {
        assert_eq!(
            kinds_at("Suizidalität verneint", "Suizidalität"),
            vec![ProtectedKind::Risk]
        );
        assert_eq!(
            kinds_at("Suizidalität verneint", "verneint"),
            vec![ProtectedKind::Negation]
        );
        assert_eq!(
            kinds_at("keine Selbstverletzungen", "Selbstverletzungen"),
            vec![ProtectedKind::Risk]
        );
    }

    #[test]
    fn detects_provenance_tokens() {
        assert_eq!(
            kinds_at("Diagnose F32.1 gesichert", "F32.1"),
            vec![ProtectedKind::Provenance]
        );
        assert_eq!(
            kinds_at("AHV 756.1234.5678.97", "756.1234.5678.97"),
            vec![ProtectedKind::Provenance]
        );
    }

    #[test]
    fn detects_substances_from_builtin_and_patient_lexicon() {
        assert_eq!(
            kinds_at("Sertralin erhöht", "Sertralin"),
            vec![ProtectedKind::Medication]
        );

        let mut lexicon = ProtectionLexicon::default();
        lexicon.push_stem("Fictionalol");
        let spans = detect("Fictionalol 10 mg", &lexicon);
        assert!(spans
            .iter()
            .any(|s| s.kind == ProtectedKind::Medication && s.start == 0 && s.end == 11));
    }

    #[test]
    fn short_or_numeric_lexicon_terms_are_ignored() {
        let mut lexicon = ProtectionLexicon::default();
        lexicon.push_stem("mg");
        lexicon.push_stem("50");
        assert!(lexicon.substance_stems.is_empty());
    }

    #[test]
    fn cuts_inside_protected_spans_are_rejected() {
        let text = "Sertralin 50 mg täglich";
        let spans = detect(text, &ProtectionLexicon::builtin());
        let dose_start = text.chars().position(|c| c == '5').unwrap();
        assert!(is_safe_cut(&spans, dose_start));
        assert!(!is_safe_cut(&spans, dose_start + 1));
        assert!(is_safe_cut(&spans, text.chars().count()));
    }

    #[test]
    fn spans_within_rebases_offsets() {
        let text = "abc keine Suizidgedanken";
        let spans = detect(text, &ProtectionLexicon::builtin());
        let window = spans_within(&spans, 4, text.chars().count());
        assert!(window
            .iter()
            .any(|s| s.kind == ProtectedKind::Negation && s.start == 0));
    }

    #[test]
    fn char_slice_respects_multibyte_offsets() {
        let text = "Ärztin: Sertralin";
        assert_eq!(char_slice(text, 0, 6), "Ärztin");
        assert_eq!(char_slice(text, 8, 100), "Sertralin");
    }
}
