//! Benchmark harness for issue #403: a ~16K assembled evidence prompt against
//! 64K and 128K raw-history baselines.
//!
//! The scoring pass that needs a model is `#[ignore]`d because CI ships no
//! approved GGUF (same convention as `engine::benchmark_cold_and_warm_contexts`).
//! The size and containment comparisons run in CI, because they need no model:
//! they show that assembly holds a fixed budget while the raw history grows
//! without bound, and that the facts the probes ask about survive assembly.
//!
//! ```sh
//! cd dokassist/src-tauri
//! RAMDOC_BENCH_MODEL=/absolute/path/to/model.gguf \
//!   cargo test benchmark_assembled_vs_raw_history --release -- --ignored --nocapture
//! ```

use rusqlite::Connection;

use super::protect::{self, ProtectedKind, ProtectionLexicon};
use super::test_support::{seed_patient, TestVault};
use super::tokens::{estimate_tokens, HeuristicCounter};
use super::{assemble_patient_evidence, budget_for_context, EvidenceRequest};

/// An uncapped dump of the record, as a 64K/128K baseline would have to carry
/// it.
///
/// `llm::patient_context::assemble_patient_context` is not used as the baseline
/// because it already truncates to the 20 most recent sessions — a silent,
/// unreported form of the loss this issue is about.
fn raw_history_dump(conn: &Connection, patient_id: &str) -> String {
    let mut dump = String::new();
    for (kind, sql) in [
        (
            "DIAGNOSEN",
            "SELECT icd10_code || ' ' || description || ' (' || status || ', ' || diagnosed_date \
                || ')' FROM diagnoses WHERE patient_id = ?1 ORDER BY diagnosed_date",
        ),
        (
            "MEDIKATION",
            "SELECT substance || ' ' || dosage || ' ' || frequency || ' seit ' || start_date \
             FROM medications WHERE patient_id = ?1 ORDER BY start_date",
        ),
        (
            "SITZUNGEN",
            "SELECT session_date || ' (' || session_type || '): ' || COALESCE(notes, '') \
             FROM sessions WHERE patient_id = ?1 ORDER BY session_date",
        ),
    ] {
        dump.push_str(&format!("===== {kind} =====\n"));
        let mut stmt = conn.prepare(sql).expect("prepare baseline query");
        let rows = stmt
            .query_map([patient_id], |row| row.get::<_, String>(0))
            .expect("baseline rows");
        for row in rows {
            dump.push_str(&row.expect("baseline row"));
            dump.push('\n');
        }
    }
    dump
}

/// A question with a fact that must survive into the answer.
struct Probe {
    question: &'static str,
    /// A string the answer must contain to count as recalled.
    needle: &'static str,
}

const PROBES: &[Probe] = &[
    Probe {
        question: "Wann wurde die Sertralin-Dosis zuletzt erhöht und auf welche Dosis?",
        needle: "150 mg",
    },
    Probe {
        question: "Welcher PHQ-9-Wert wurde zuletzt dokumentiert?",
        needle: "PHQ-9",
    },
    Probe {
        question: "Wurde jemals Suizidalität dokumentiert?",
        needle: "Suizidalität",
    },
];

/// Fill a patient record with `sessions` dated sessions of routine text, plus
/// the specific facts the probes ask about, so retrieval has to find a needle
/// in a long history rather than in a handful of notes.
fn seed_long_history(conn: &Connection, patient_id: &str, sessions: usize) {
    for index in 0..sessions {
        // Oldest first; the planted facts sit in the middle of the history.
        let day = (sessions - index) as i64 * 7;
        let date = (chrono::Utc::now() - chrono::Duration::days(day))
            .format("%Y-%m-%d")
            .to_string();
        let notes = match index {
            i if i == sessions / 2 => "Sertralin von 100 mg auf 150 mg erhöht wegen \
                 persistierender Morgentiefs. Verträglichkeit gut, keine Suizidalität."
                .to_string(),
            i if i == sessions / 2 + 3 => "Verlaufskontrolle nach Dosiserhöhung: PHQ-9 bei 9, \
                 Schlaf stabil, keine unerwünschten Wirkungen."
                .to_string(),
            _ => format!(
                "Routinesitzung {index}: Aktivitätsaufbau besprochen und das Wochenprotokoll \
                 gemeinsam durchgesehen. Die Übungen zur Achtsamkeit wurden wiederholt und um \
                 eine kurze Atemübung für den Arbeitsalltag ergänzt. Stimmung im Verlauf \
                 unverändert, Antrieb tagesformabhängig, Alltagsstruktur eingehalten, \
                 Arbeitsfähigkeit erhalten. Soziale Kontakte werden weiterhin gepflegt, das \
                 Wochenende wurde mit Spaziergängen und Lesen verbracht. Kein Hinweis auf \
                 psychotisches Erleben, kein Substanzkonsum berichtet. Hausaufgabe: \
                 Aktivitätenprotokoll fortführen und Schlafzeiten notieren. Nächster Termin in \
                 einer Woche vereinbart, Fortsetzung der begonnenen Expositionsplanung."
            ),
        };
        conn.execute(
            "INSERT INTO sessions (id, patient_id, session_date, session_type, notes) \
             VALUES (?1, ?2, ?3, 'Verlaufsgespräch', ?4)",
            rusqlite::params![format!("bench-session-{index}"), patient_id, date, notes],
        )
        .expect("insert benchmark session");
    }
}

/// Dates the answer asserts that do not occur in the prompt it was given.
///
/// A cheap, model-independent proxy for unsupported claims that works for the
/// raw baselines too, which have no citation markers to audit.
fn unsupported_dates(answer: &str, prompt: &str) -> Vec<String> {
    let lexicon = ProtectionLexicon::builtin();
    protect::detect(answer, &lexicon)
        .into_iter()
        .filter(|span| span.kind == ProtectedKind::Date)
        .map(|span| protect::char_slice(answer, span.start, span.end))
        .filter(|date| !prompt.contains(date.as_str()))
        .collect()
}

#[test]
fn assembled_evidence_holds_its_budget_while_raw_history_grows() {
    let vault = TestVault::new();
    let conn = vault.conn();
    let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
    seed_long_history(&conn, &patient.patient_id, 200);

    let budget = budget_for_context(16_384, 4_096);
    let raw = raw_history_dump(&conn, &patient.patient_id);
    let raw_tokens = estimate_tokens(&raw);

    for probe in PROBES {
        let request =
            EvidenceRequest::new(&patient.patient_id, probe.question).with_token_budget(budget);
        let assembled = assemble_patient_evidence(&conn, &request, &HeuristicCounter).unwrap();

        assert!(
            assembled.manifest.prompt_tokens <= budget,
            "assembled evidence must stay inside the budget"
        );
        assert!(
            assembled.manifest.prompt_tokens * 2 < raw_tokens,
            "assembled evidence ({} tokens) should be far smaller than the raw history ({} tokens)",
            assembled.manifest.prompt_tokens,
            raw_tokens
        );
        assert!(
            assembled.evidence.contains(probe.needle),
            "probe fact {:?} must survive assembly",
            probe.needle
        );
        assert!(
            !assembled.manifest.entries.is_empty()
                && assembled
                    .manifest
                    .entries
                    .iter()
                    .all(|entry| !entry.selection_reasons.is_empty()),
            "every included unit must report why it was selected"
        );
    }

    // The raw history is what the 64K/128K baselines would have to hold.
    assert!(
        raw_tokens > 16_384,
        "the synthetic history must exceed a 16K context to be a meaningful baseline \
         (got {raw_tokens} tokens)"
    );
}

/// Hardware benchmark: factual recall, unsupported claims, latency and peak
/// memory for the assembled prompt against truncated raw-history baselines.
#[test]
#[ignore = "requires a local GGUF model"]
fn benchmark_assembled_vs_raw_history() {
    use crate::llm::engine::LlmEngine;
    use crate::llm::prompts::{
        evidence_query_prompt, patient_history_query_prompt, SYSTEM_PROMPT_DE,
    };

    let model_path = std::env::var("RAMDOC_BENCH_MODEL").expect("RAMDOC_BENCH_MODEL is required");
    let model_path = std::path::PathBuf::from(model_path);
    let model_name = model_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("benchmark.gguf")
        .to_string();
    let engine = LlmEngine::load(model_path, model_name).expect("load model");

    let vault = TestVault::new();
    let conn = vault.conn();
    let patient = seed_patient(&conn, "756.1111.1111.11", "Anna", "Amsler");
    seed_long_history(&conn, &patient.patient_id, 200);

    let raw_history = raw_history_dump(&conn, &patient.patient_id);
    let raw_history_tokens = engine.count_tokens(&raw_history);
    let mut arms = Vec::new();

    for probe in PROBES {
        // ── Assembled evidence, 16K context ─────────────────────────────────
        let budget = budget_for_context(16_384, 4_096);
        let request =
            EvidenceRequest::new(&patient.patient_id, probe.question).with_token_budget(budget);
        let assembled = assemble_patient_evidence(&conn, &request, &engine).expect("assemble");
        let prompt = evidence_query_prompt(&assembled.evidence, probe.question);
        let answer = engine
            .generate(SYSTEM_PROMPT_DE, &prompt, 512, 0.0)
            .expect("assembled generation");
        let stats = engine.last_generation_stats().expect("stats");
        let audit = super::audit_answer(&conn, &assembled.manifest, &answer).expect("audit");

        arms.push(serde_json::json!({
            "arm": "assembled-16k",
            "question": probe.question,
            "prompt_tokens": stats.prompt_tokens,
            "evidence_tokens": assembled.manifest.prompt_tokens,
            "recalled": answer.contains(probe.needle),
            "citations": audit.citations.len(),
            "unsupported_citations": audit.unsupported_citations,
            "stale_citations": audit.stale_citations,
            "all_citations_traceable": audit.all_citations_traceable(),
            "unsupported_dates": unsupported_dates(&answer, &prompt),
            "protected_spans_retained": assembled.manifest.protected_spans_retained,
            "ttft_ms": stats.ttft_ms,
            "prefill_ms": stats.prefill_ms,
            "total_latency_ms": stats.total_latency_ms,
            "peak_rss_bytes": stats.peak_rss_bytes,
        }));

        // ── Raw-history baselines ───────────────────────────────────────────
        for baseline_context in [65_536usize, 131_072usize] {
            let baseline_budget = budget_for_context(baseline_context, 4_096);
            let truncated = crate::llm::utf8::truncate_to_boundary(
                &raw_history,
                // Rough char budget; the exact token count is reported below.
                baseline_budget * 3,
            );
            let prompt = patient_history_query_prompt(truncated, probe.question);
            let prompt_tokens = engine.count_tokens(&prompt);

            if prompt_tokens + 512 > engine.context_size() {
                // This is the finding, not a gap: the baseline does not fit in
                // the context the memory governor allows on this machine.
                arms.push(serde_json::json!({
                    "arm": format!("raw-{}k", baseline_context / 1024),
                    "question": probe.question,
                    "prompt_tokens": prompt_tokens,
                    "skipped": "prompt exceeds the loaded context window",
                    "loaded_context": engine.context_size(),
                }));
                continue;
            }

            let answer = engine
                .generate(SYSTEM_PROMPT_DE, &prompt, 512, 0.0)
                .expect("baseline generation");
            let stats = engine.last_generation_stats().expect("stats");
            arms.push(serde_json::json!({
                "arm": format!("raw-{}k", baseline_context / 1024),
                "question": probe.question,
                "prompt_tokens": stats.prompt_tokens,
                "recalled": answer.contains(probe.needle),
                "unsupported_dates": unsupported_dates(&answer, &prompt),
                "ttft_ms": stats.ttft_ms,
                "prefill_ms": stats.prefill_ms,
                "total_latency_ms": stats.total_latency_ms,
                "peak_rss_bytes": stats.peak_rss_bytes,
            }));
        }
    }

    println!(
        "{}",
        serde_json::json!({
            "raw_history_tokens": raw_history_tokens,
            "loaded_context": engine.context_size(),
            "arms": arms,
        })
    );
}
