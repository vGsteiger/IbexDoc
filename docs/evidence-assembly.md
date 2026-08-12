# Provenance-bearing evidence assembly

Issue #403. Implemented in `dokassist/src-tauri/src/llm/evidence/`, schema in
`migrations/015_evidence_provenance.sql`.

The context this layer fills is planned by the
[memory governor](local-inference-16gb-research.md): it decides how large a
context a machine can afford, and `budget_for_context` sizes the evidence block
inside it.

A patient-history question must be answerable on a 16K-context local model
without putting the raw record in the KV cache, and every sentence of the answer
must be traceable to an exact, current source revision. This layer sits between
the record and the model: it indexes the record into provenance-bearing units,
retrieves the ones a question needs, and assembles them into a token-budgeted
evidence block with a manifest.

## Interface

`llm::evidence` is the stable surface. Callers (issue #164) depend on it, not on
the retrieval strategy behind it.

```rust
let request = EvidenceRequest::new(&patient_id, &question)
    .with_token_budget(budget_for_context(engine.context_size(), 4_096));
let assembled = assemble_patient_evidence(&conn, &request, engine.as_ref())?;

let prompt = prompts::evidence_query_prompt(&assembled.evidence, &question);
// … run inference …
let audit = audit_answer(&conn, &assembled.manifest, &answer)?;
```

Tauri commands: `query_patient_history` (answer + manifest + citation audit),
`preview_patient_evidence` (assemble without inference),
`index_patient_evidence` (refresh + embed), `get_patient_evidence_manifest`,
`resolve_evidence_units` (cited unit → current source text).

## Provenance

An evidence unit is a character range inside one *section* of one record:

| Field | Meaning |
|-------|---------|
| `patient_id` | Scope. Present on every unit, embedding, FTS row and manifest. |
| `record_kind` / `record_id` | The source row (`session`, `file`, `medication`, …). |
| `section` | A column (`notes`, `clinical_summary`, `extracted_text`, `description`) or `canonical`, a deterministic rendering of the row's typed columns. |
| `revision` | `SHA-256(kind, id, section, updated_at, length, text)`, truncated. Covers content, so an edit is detected even when a timestamp is not refreshed. |
| `char_start` / `char_end` | Exact **character** offsets into that section's text. |
| `occurred_at` | Clinical date (session date, diagnosis date …), used for the timeline. |

Character offsets are model-independent and therefore what is stored. Token
offsets are model-specific, so they are recorded per assembly instead: each
manifest entry carries `prompt_token_start` / `prompt_token_end` inside the
assembled block, measured with the tokenizer that will run the prompt.

`provenance::resolve_span` re-reads the section, recomputes the revision and
re-slices the range, so a citation is verifiable rather than merely plausible.
`patient_revision` aggregates all section revisions into one record stamp; it
keys the llama.cpp KV-context cache so a context built from an older record is
never reused.

## Three tiers

* **Structured clinical truth** — `canonical` renderings of diagnoses,
  medications, outcome scores, plans and goals. Always offered first, capped at
  35 % of the budget.
* **Hot verbatim** — text from the last 180 days (plus the three newest
  sessions, however old), quoted exactly.
* **Cold searchable history** — everything older. Retrievable and promoted into
  the prompt verbatim when it wins retrieval; otherwise represented by a dated
  pointer line so the model can say what exists instead of inventing it.

## Retrieval

Four signals, merged with Reciprocal Rank Fusion (k = 60, matching
`search::hybrid_search`):

1. Lexical BM25 over `evidence_fts`.
2. Cosine similarity over this patient's unit embeddings (missing vectors
   degrade retrieval to lexical rather than failing).
3. Temporal expansion: a recency prior — stronger when the question itself is
   temporal ("zuletzt", "seit", "trend") — plus units within ±30 days of a
   strong hit.
4. Document-neighbour expansion: units adjacent to a hit inside the same
   section.

Every candidate keeps the signals that selected it. The manifest reports them
both structurally and as sentences (`"lexical rank 1 (bm25 -1.826)"`,
`"same clinical period as 2 hit(s)"`).

## Protected spans

`protect::detect` marks spans that must not be lost to summarisation:
medication names (built-in stems plus the patient's own substances),
doses (`50 mg`, `1-0-1`, `2x täglich`), dates (ISO, Swiss dotted, month names,
years), negation cues, uncertainty cues, risk statements, and provenance tokens
(ICD-10 codes, AHV numbers). Two consequences:

* Unit boundaries never fall inside a protected span — a unit may exceed its
  size limit rather than split a dose.
* Assembly never truncates a unit. It fits whole or is recorded as omitted, with
  a reason. The manifest reports the retained protections per kind.

## Invalidation

The index is derived state, and every derived row records the revision it came
from. `refresh_patient_index` compares each section's current revision with the
indexed one and, on a change, deletes that section's units (embeddings cascade),
drops `document_chunks`/`chunk_embeddings` derived from the same file text, and
re-cuts the section. Sections whose record was deleted or emptied are removed.
Unchanged sections are untouched, so repeated questions do no indexing work.

## Manifest and answer audit

The manifest is metadata-only: unit ids, provenance, digests, token spans,
selection reasons, omissions with reasons, retrieval diagnostics and index
statistics — plus the question terms the caller already has. It carries no record
text, and is persisted per assembled prompt in `evidence_manifests`.

The prompt labels every excerpt `[E# | date | source | revision]` and requires
the model to cite those markers. `audit_answer` extracts the citations and
classifies each one:

* not issued by the manifest → **unsupported**;
* issued, but the source moved on or vanished → **stale**;
* otherwise re-resolved to identical text at the current revision → traceable.

## Benchmark

`llm::evidence::benchmark` holds both passes.

`assembled_evidence_holds_its_budget_while_raw_history_grows` runs in CI: over a
200-session synthetic history it asserts the assembled block stays inside its
budget, stays far below the raw history, still contains the probed facts, and
reports a selection reason for every included unit.

The scored pass needs a model and is therefore ignored by default:

```sh
cd dokassist/src-tauri
RAMDOC_BENCH_MODEL=/absolute/path/to/model.gguf \
  cargo test benchmark_assembled_vs_raw_history --release -- --ignored --nocapture
```

It compares a ~16K assembled prompt with 64K and 128K raw-history baselines and
emits JSON per arm: factual recall against planted facts, unsupported claims
(citation audit for the assembled arm, dates asserted but absent from the prompt
for all arms), `ttft_ms`, `prefill_ms`, `total_latency_ms` and `peak_rss_bytes`.
A baseline whose prompt exceeds the context the memory governor allows is
reported as skipped with its token count — that is a result, not a gap.

Baselines are built from an uncapped record dump rather than the previous
`assemble_patient_context`, which silently truncated to the 20 most recent
sessions.
