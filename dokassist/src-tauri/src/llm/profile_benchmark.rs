//! Benchmark harness for issue #399: quality, latency and total memory across
//! the inference profiles (context size, KV-cache quantization, Flash
//! Attention, bounded micro-batches) that #418 made configurable.
//!
//! The harness has three layers, and only the middle one needs hardware:
//!
//! 1. **Planning** runs in CI. It asks the same [`MemoryGovernor`] the loader
//!    uses whether the reference model fits at each context/KV combination on a
//!    reference machine, and names the component that limits it. This answers
//!    "can Qwen3-8B be evaluated at 32K on a 16 GiB Mac" analytically, before
//!    anyone spends an hour on a measurement that was never going to fit.
//! 2. **Measurement** is `#[ignore]`d because CI ships no approved GGUF (same
//!    convention as `engine::benchmark_cold_and_warm_contexts`). It loads *one*
//!    profile, runs a needle-in-a-haystack probe set at temperature 0, and
//!    records recall, latency, resident memory, swap and page-ins.
//! 3. **Collation** runs in CI. It compares the recorded arms against the
//!    baseline arm — recall per needle depth, answer equivalence, latency and
//!    memory ratios — and is the part that decides whether a Q4 KV cache costs
//!    quality.
//!
//! One profile per process is not a convenience: `LlamaBackend::init` may only
//! be called once per process, and macOS peak resident memory is a
//! process-lifetime high-water mark, so a four-arm sweep inside one process
//! would report neither honest memory nor a second loadable profile.
//!
//! ```sh
//! cd dokassist/src-tauri
//! ../../scripts/bench-inference-profiles.sh /abs/path/Qwen3-8B-Q4_K_M.gguf
//! ```

use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::download;
use super::engine::{runtime_context_for_ram, LlmEngine};
use super::inference::InferenceProfile;
use super::memory_governor::{GgufArchitecture, MemoryEstimate, MemoryGovernor};

/// The arm every other arm is compared against: what `LlmEngine::load` used
/// before this issue, and what ships today.
pub(crate) const BASELINE_ARM: &str = "conservative";

/// The profiles the sweep covers, baseline first. `governed` is the shipped
/// default that the memory governor picks; the `*-32k` arms are the research
/// overrides whose cost this benchmark exists to measure.
pub(crate) const SWEEP_ARMS: &[&str] = &[BASELINE_ARM, "governed", "f16-32k", "q8-32k", "q4-32k"];

/// Tokens reserved for the completion in every probe.
const PROBE_MAX_TOKENS: usize = 256;

// ── Probe set ───────────────────────────────────────────────────────────────

/// A fact planted at a known depth in a long history, plus the question that
/// retrieves it.
///
/// Depth matters: a quantized KV cache degrades attention over *distance*, so
/// an early, a middle and a late needle separate "the model is worse" from "the
/// KV cache lost the far end of the context".
struct Needle {
    /// Position in the haystack, in per mille of its length.
    depth_permille: usize,
    fact: &'static str,
    question: &'static str,
    /// Substring an answer must contain to count as recalled.
    needle: &'static str,
}

const NEEDLES: &[Needle] = &[
    Needle {
        depth_permille: 50,
        fact: "Wichtig: Es besteht eine dokumentierte Penicillinallergie mit Exanthem.",
        question: "Welche Allergie ist in der Akte dokumentiert?",
        needle: "Penicillin",
    },
    Needle {
        depth_permille: 500,
        fact: "Wichtig: Die Sertralin-Dosis wurde auf 150 mg täglich erhöht.",
        question: "Auf welche Tagesdosis wurde Sertralin erhöht?",
        needle: "150 mg",
    },
    Needle {
        depth_permille: 950,
        fact: "Wichtig: Der nächste Kontrolltermin ist am 02.06.2025 vereinbart.",
        question: "Wann ist der nächste Kontrolltermin vereinbart?",
        needle: "02.06.2025",
    },
];

/// Routine session text with no digits or substrings that could be mistaken for
/// a needle, so a recall hit can only come from the planted fact.
fn filler_paragraph(index: usize) -> String {
    let variant = match index % 4 {
        0 => "Aktivitätsaufbau besprochen und das Wochenprotokoll gemeinsam durchgesehen.",
        1 => "Achtsamkeitsübungen wiederholt und um eine kurze Atemübung ergänzt.",
        2 => "Stimmung im Verlauf unverändert, Antrieb tagesformabhängig, Alltagsstruktur eingehalten.",
        _ => "Soziale Kontakte werden gepflegt, kein Hinweis auf psychotisches Erleben.",
    };
    format!(
        "Verlaufsgespräch: {variant} Arbeitsfähigkeit erhalten, kein Substanzkonsum berichtet. \
         Hausaufgabe: Protokoll fortführen und Schlafzeiten notieren."
    )
}

/// Build a haystack of `paragraphs` routine entries with every needle planted
/// at its declared depth.
fn planted_haystack(paragraphs: usize) -> String {
    let paragraphs = paragraphs.max(NEEDLES.len() + 1);
    let mut lines: Vec<String> = (0..paragraphs).map(filler_paragraph).collect();
    // Insert from the deepest needle backwards so earlier insertions do not
    // shift the position of later ones.
    let mut planted: Vec<&Needle> = NEEDLES.iter().collect();
    planted.sort_by_key(|needle| std::cmp::Reverse(needle.depth_permille));
    for needle in planted {
        let at = (paragraphs * needle.depth_permille / 1000).min(lines.len());
        lines.insert(at, needle.fact.to_string());
    }
    lines.join("\n")
}

/// Grow the haystack to just under `target_tokens` as measured by `count`.
///
/// It never exceeds the target: an over-long prompt would be rejected by the
/// context budget and turn a memory finding into a test error.
fn haystack_for_tokens(target_tokens: usize, count: &dyn Fn(&str) -> usize) -> String {
    let sample_paragraphs = 64;
    let per_paragraph = count(&planted_haystack(sample_paragraphs))
        .div_ceil(sample_paragraphs)
        .max(1);
    let mut paragraphs = (target_tokens / per_paragraph).max(NEEDLES.len() + 1);
    let mut text = planted_haystack(paragraphs);
    while count(&text) > target_tokens {
        let smaller = (paragraphs * 9 / 10).max(NEEDLES.len() + 1);
        if smaller == paragraphs {
            break;
        }
        paragraphs = smaller;
        text = planted_haystack(paragraphs);
    }
    text
}

// ── Planning (no model required) ────────────────────────────────────────────

/// What a profile would cost on a reference machine, and what limits it.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct PlanReport {
    pub arm: String,
    pub context_size: usize,
    pub kv_cache: String,
    pub estimate: MemoryEstimate,
    pub budget_bytes: u64,
    pub fits: bool,
    /// How far over the budget the estimate is; zero when it fits.
    pub overflow_bytes: u64,
    /// The largest single component of the estimate, whether or not the arm
    /// fits. Informational: for a 4-bit 8B model the weights usually win, which
    /// says nothing about what to change.
    pub dominant_component: &'static str,
    /// What has to shrink for this arm to fit, or `None` when it already does.
    pub limiting_component: Option<&'static str>,
}

fn dominant_component(estimate: &MemoryEstimate) -> &'static str {
    [
        ("kv_cache", estimate.kv_cache_bytes),
        ("weights", estimate.weights_bytes),
        ("graph", estimate.graph_bytes),
        ("runtime", estimate.runtime_bytes),
    ]
    .into_iter()
    .max_by_key(|(_, bytes)| *bytes)
    .map(|(name, _)| name)
    .unwrap_or("unknown")
}

/// Name the component that has to give.
///
/// The largest component is the wrong answer: model weights and fixed runtime
/// overhead do not move when a profile changes context size or KV type, so
/// naming them tells the reader nothing actionable. Only when the immovable
/// part alone already exceeds the budget are the weights genuinely the limit —
/// and then the answer is a smaller model, not a smaller context.
fn limiting_component(estimate: &MemoryEstimate, budget_bytes: u64) -> Option<&'static str> {
    if estimate.total_bytes <= budget_bytes {
        return None;
    }
    let immovable = estimate
        .weights_bytes
        .saturating_add(estimate.runtime_bytes);
    if immovable >= budget_bytes {
        return Some("weights");
    }
    Some(if estimate.kv_cache_bytes >= estimate.graph_bytes {
        "kv_cache"
    } else {
        "graph"
    })
}

/// Plan one arm against a governor without loading the model.
fn plan_arm(governor: &MemoryGovernor, arm: &str, total_ram_bytes: u64) -> PlanReport {
    let native = governor.architecture().native_context;
    let profile = match arm {
        "governed" | "auto" => governor.plan(None).0,
        name => InferenceProfile::named(name, runtime_context_for_ram(total_ram_bytes))
            .expect("sweep arms are known profile names")
            .resolved_for_model(native)
            .expect("a profile capped to the native context stays valid"),
    };
    let estimate = governor.estimate(&profile);
    let budget_bytes = governor.inference_budget_bytes();
    PlanReport {
        arm: arm.to_string(),
        context_size: profile.n_ctx,
        kv_cache: profile.kv_cache.label().to_string(),
        dominant_component: dominant_component(&estimate),
        limiting_component: limiting_component(&estimate, budget_bytes),
        fits: estimate.total_bytes <= budget_bytes,
        overflow_bytes: estimate.total_bytes.saturating_sub(budget_bytes),
        estimate,
        budget_bytes,
    }
}

/// The 16 GiB reference machine from the issue, planned against the approved
/// Qwen3-8B entry so the dimensions cannot drift away from what RamDoc ships.
fn qwen3_8b_reference_governor(total_ram_bytes: u64) -> MemoryGovernor {
    let entry = download::find_model("Qwen3-8B-Q4_K_M.gguf")
        .expect("Qwen3-8B stays on the approved model list");
    MemoryGovernor::from_parts(
        GgufArchitecture {
            architecture: "qwen3".to_string(),
            // Qwen3-8B: 36 blocks, 4096-wide, 32 attention heads over 8 KV
            // heads (GQA), head dimension 4096 / 32 = 128.
            layers: 36,
            embedding_length: 4096,
            attention_heads: 32,
            kv_heads: 8,
            native_context: entry.context_window_tokens as usize,
            recurrent: false,
        },
        entry.size_bytes,
        total_ram_bytes,
    )
}

// ── Measurement (requires a model) ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProbeRecord {
    pub depth_permille: usize,
    pub question: String,
    pub prompt_tokens: usize,
    pub recalled: bool,
    pub answer: String,
    pub ttft_ms: f64,
    pub prefill_ms: f64,
    pub total_latency_ms: f64,
    pub tps: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ArmRecord {
    /// The profile name that was requested.
    pub arm: String,
    pub model: String,
    /// The profile actually in force — may differ from `arm` after a fallback.
    pub effective_profile: String,
    pub context_size: usize,
    pub kv_cache: String,
    pub n_batch: u32,
    pub n_ubatch: u32,
    pub flash_attention: String,
    pub fallback: Option<String>,
    pub fallback_code: Option<String>,
    pub estimated_total_bytes: Option<u64>,
    pub inference_budget_bytes: Option<u64>,
    /// Resident memory with the weights loaded but no context allocated.
    pub resident_after_model_load_bytes: u64,
    /// Resident memory once the first context — and therefore the KV cache —
    /// exists. The difference from `resident_after_model_load_bytes` is what
    /// the context configuration actually costs.
    pub resident_after_first_context_bytes: u64,
    /// Highest resident memory seen across the probe set.
    pub peak_resident_bytes: u64,
    /// System-wide swap growth over the run. Sustained growth is a failure
    /// signal, not headroom.
    pub swap_used_delta_bytes: i64,
    /// Page-ins charged to this process over the run.
    pub page_ins_delta: u64,
    pub probes: Vec<ProbeRecord>,
}

// ── Collation ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct ArmComparison {
    pub arm: String,
    pub context_size: usize,
    pub kv_cache: String,
    /// Needles recalled, out of those probed.
    pub recalled: usize,
    pub probes: usize,
    /// Needle depths the baseline recalled and this arm did not.
    pub lost_depths: Vec<usize>,
    /// Answers byte-identical to the baseline's, out of those compared.
    pub identical_answers: usize,
    pub answers_compared: usize,
    /// Ratios against the baseline; >1 is slower for latency, faster for tps.
    pub ttft_ratio: f64,
    pub total_latency_ratio: f64,
    pub tps_ratio: f64,
    pub peak_resident_bytes: u64,
    pub peak_resident_delta_bytes: i64,
    pub swap_used_delta_bytes: i64,
    pub page_ins_delta: u64,
    pub verdict: String,
}

fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let (sum, count) = values.fold((0.0, 0usize), |(sum, count), value| {
        (sum + value, count + 1)
    });
    if count == 0 {
        0.0
    } else {
        sum / count as f64
    }
}

fn ratio(arm: f64, baseline: f64) -> f64 {
    if baseline == 0.0 {
        0.0
    } else {
        arm / baseline
    }
}

/// Compare every recorded arm against the baseline arm.
///
/// Returns an error rather than a partial table when the baseline is missing:
/// a sweep without its baseline measures nothing.
pub(crate) fn collate(records: &[ArmRecord]) -> Result<Vec<ArmComparison>, String> {
    let baseline = records
        .iter()
        .find(|record| record.arm == BASELINE_ARM)
        .ok_or_else(|| {
            format!("sweep has no '{BASELINE_ARM}' baseline arm; nothing to compare against")
        })?;

    Ok(records
        .iter()
        .map(|record| {
            let paired: Vec<(&ProbeRecord, &ProbeRecord)> = record
                .probes
                .iter()
                .filter_map(|probe| {
                    baseline
                        .probes
                        .iter()
                        .find(|other| other.depth_permille == probe.depth_permille)
                        .map(|other| (probe, other))
                })
                .collect();

            let lost_depths: Vec<usize> = paired
                .iter()
                .filter(|(probe, base)| base.recalled && !probe.recalled)
                .map(|(probe, _)| probe.depth_permille)
                .collect();
            let identical_answers = paired
                .iter()
                .filter(|(probe, base)| probe.answer == base.answer)
                .count();
            let recalled = record.probes.iter().filter(|probe| probe.recalled).count();

            let swapped = record.swap_used_delta_bytes > 0;
            let verdict = if !lost_depths.is_empty() {
                format!(
                    "quality regression: needle(s) at depth {} lost against the baseline",
                    lost_depths
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            } else if swapped {
                "recall preserved, but the run grew system swap; treat as not fitting".to_string()
            } else if identical_answers == paired.len() && !paired.is_empty() {
                "equivalent: identical answers at temperature 0".to_string()
            } else {
                "recall preserved; wording differs from the baseline".to_string()
            };

            ArmComparison {
                arm: record.arm.clone(),
                context_size: record.context_size,
                kv_cache: record.kv_cache.clone(),
                recalled,
                probes: record.probes.len(),
                lost_depths,
                identical_answers,
                answers_compared: paired.len(),
                ttft_ratio: ratio(
                    mean(record.probes.iter().map(|probe| probe.ttft_ms)),
                    mean(baseline.probes.iter().map(|probe| probe.ttft_ms)),
                ),
                total_latency_ratio: ratio(
                    mean(record.probes.iter().map(|probe| probe.total_latency_ms)),
                    mean(baseline.probes.iter().map(|probe| probe.total_latency_ms)),
                ),
                tps_ratio: ratio(
                    mean(record.probes.iter().map(|probe| probe.tps)),
                    mean(baseline.probes.iter().map(|probe| probe.tps)),
                ),
                peak_resident_bytes: record.peak_resident_bytes,
                peak_resident_delta_bytes: record.peak_resident_bytes as i64
                    - baseline.peak_resident_bytes as i64,
                swap_used_delta_bytes: record.swap_used_delta_bytes,
                page_ins_delta: record.page_ins_delta,
                verdict,
            }
        })
        .collect())
}

// ── Host memory probes ──────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
fn task_info() -> Option<libc::proc_taskinfo> {
    let mut info = std::mem::MaybeUninit::<libc::proc_taskinfo>::zeroed();
    let size = std::mem::size_of::<libc::proc_taskinfo>() as i32;
    let read = unsafe {
        libc::proc_pidinfo(
            std::process::id() as i32,
            libc::PROC_PIDTASKINFO,
            0,
            info.as_mut_ptr() as *mut libc::c_void,
            size,
        )
    };
    (read == size).then(|| unsafe { info.assume_init() })
}

/// Current resident memory. Unlike `getrusage`'s high-water mark this falls
/// again when an engine is dropped, which is what makes per-arm comparison
/// possible at all.
#[cfg(target_os = "macos")]
fn resident_bytes() -> u64 {
    task_info().map_or(0, |info| info.pti_resident_size)
}

#[cfg(target_os = "macos")]
fn page_ins() -> u64 {
    task_info().map_or(0, |info| info.pti_pageins.max(0) as u64)
}

/// System-wide swap in use, from `vm.swapusage`.
#[cfg(target_os = "macos")]
fn swap_used_bytes() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::xsw_usage>::zeroed();
    let mut len = std::mem::size_of::<libc::xsw_usage>();
    let name = std::ffi::CString::new("vm.swapusage").expect("static sysctl name");
    let ok = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            usage.as_mut_ptr() as *mut libc::c_void,
            &mut len as *mut usize,
            std::ptr::null_mut(),
            0,
        )
    } == 0;
    if ok {
        unsafe { usage.assume_init() }.xsu_used
    } else {
        0
    }
}

#[cfg(not(target_os = "macos"))]
fn resident_bytes() -> u64 {
    0
}
#[cfg(not(target_os = "macos"))]
fn page_ins() -> u64 {
    0
}
#[cfg(not(target_os = "macos"))]
fn swap_used_bytes() -> u64 {
    0
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1024 * 1024 * 1024;

    fn probe(depth: usize, recalled: bool, answer: &str, latency: f64) -> ProbeRecord {
        ProbeRecord {
            depth_permille: depth,
            question: format!("q{depth}"),
            prompt_tokens: 1_000,
            recalled,
            answer: answer.to_string(),
            ttft_ms: latency / 4.0,
            prefill_ms: latency / 8.0,
            total_latency_ms: latency,
            tps: 20.0,
        }
    }

    fn record(arm: &str, probes: Vec<ProbeRecord>) -> ArmRecord {
        ArmRecord {
            arm: arm.to_string(),
            model: "test.gguf".to_string(),
            effective_profile: arm.to_string(),
            context_size: 16_384,
            kv_cache: "F16".to_string(),
            n_batch: 2_048,
            n_ubatch: 512,
            flash_attention: "enabled".to_string(),
            fallback: None,
            fallback_code: None,
            estimated_total_bytes: None,
            inference_budget_bytes: None,
            resident_after_model_load_bytes: 6 * GIB,
            resident_after_first_context_bytes: 8 * GIB,
            peak_resident_bytes: 9 * GIB,
            swap_used_delta_bytes: 0,
            page_ins_delta: 0,
            probes,
        }
    }

    // ── Probe set ───────────────────────────────────────────────────────────

    #[test]
    fn haystack_plants_every_needle_and_stays_inside_its_budget() {
        let count = |text: &str| super::super::evidence::tokens::estimate_tokens(text);
        let text = haystack_for_tokens(12_000, &count);

        assert!(
            count(&text) <= 12_000,
            "the haystack must not exceed the requested budget (got {})",
            count(&text)
        );
        assert!(
            count(&text) > 6_000,
            "the haystack must be long enough to be a long-context probe (got {})",
            count(&text)
        );
        for needle in NEEDLES {
            assert!(
                text.contains(needle.fact),
                "needle at depth {} must be planted",
                needle.depth_permille
            );
            let at = text.find(needle.needle).expect("needle text is present");
            assert_eq!(
                text.rfind(needle.needle),
                Some(at),
                "needle {:?} must be unique, or recall cannot be attributed",
                needle.needle
            );
        }
    }

    #[test]
    fn tiny_haystacks_still_carry_every_needle() {
        let count = |text: &str| super::super::evidence::tokens::estimate_tokens(text);
        let text = haystack_for_tokens(1, &count);
        for needle in NEEDLES {
            assert!(text.contains(needle.fact));
        }
    }

    // ── Planning ────────────────────────────────────────────────────────────

    #[test]
    fn sweep_arms_resolve_to_valid_profiles() {
        let governor = qwen3_8b_reference_governor(16 * GIB);
        for arm in SWEEP_ARMS {
            let plan = plan_arm(&governor, arm, 16 * GIB);
            assert!(plan.context_size > 0, "{arm} must plan a non-zero context");
            assert!(
                plan.estimate.total_bytes > plan.estimate.weights_bytes,
                "{arm} must account for more than the weights"
            );
        }
    }

    /// The acceptance question from issue #399, answered without the model.
    #[test]
    fn qwen3_8b_needs_a_quantized_kv_cache_for_32k_on_16gib() {
        let governor = qwen3_8b_reference_governor(16 * GIB);
        let plan = |arm: &str| plan_arm(&governor, arm, 16 * GIB);

        let f16 = plan("f16-32k");
        assert_eq!(f16.context_size, 32_768);
        assert!(
            !f16.fits,
            "F16 KV at 32K is expected to exceed the 16 GiB budget; if this now fits, \
             re-measure and update docs/inference-profile-benchmark.md (estimate {} B, \
             budget {} B)",
            f16.estimate.total_bytes, f16.budget_bytes
        );
        assert_eq!(
            f16.limiting_component,
            Some("kv_cache"),
            "the KV cache, not the weights, is what 32K F16 cannot afford"
        );

        for arm in ["q8-32k", "q4-32k"] {
            let quantized = plan(arm);
            assert_eq!(quantized.context_size, 32_768);
            assert!(
                quantized.fits,
                "{arm} must fit the 16 GiB budget (estimate {} B, budget {} B)",
                quantized.estimate.total_bytes, quantized.budget_bytes
            );
            assert_eq!(quantized.limiting_component, None);
        }

        // Halving the KV element width must halve the KV cache, or the sweep is
        // not measuring what it claims to.
        assert!(plan("q8-32k").estimate.kv_cache_bytes < f16.estimate.kv_cache_bytes);
        assert!(plan("q4-32k").estimate.kv_cache_bytes < plan("q8-32k").estimate.kv_cache_bytes);
    }

    #[test]
    fn a_model_that_cannot_fit_at_any_context_names_the_weights_not_the_kv_cache() {
        // 8 GiB leaves less budget than Qwen3-8B's weights alone need, so no
        // context or KV type rescues it. Saying "kv_cache" here would send a
        // reader to shrink a context that was never the problem.
        let plan = plan_arm(&qwen3_8b_reference_governor(8 * GIB), "q4-32k", 8 * GIB);
        assert!(!plan.fits);
        assert_eq!(plan.limiting_component, Some("weights"));
        assert!(plan.overflow_bytes > 0);
    }

    /// #399 requires the shipped default to stay conservative until this
    /// benchmark says otherwise.
    #[test]
    fn shipped_default_stays_conservative_on_the_reference_machine() {
        let governed = plan_arm(&qwen3_8b_reference_governor(16 * GIB), "governed", 16 * GIB);
        assert!(governed.fits);
        assert_eq!(
            (governed.context_size, governed.kv_cache.as_str()),
            (16_384, "F16"),
            "the governor still prefers a 16K F16 context over a quantized 32K one; \
             changing that is a deliberate decision this benchmark has to support first"
        );
    }

    // ── Collation ───────────────────────────────────────────────────────────

    #[test]
    fn collation_without_a_baseline_is_an_error() {
        let records = vec![record("q4-32k", vec![probe(50, true, "a", 100.0)])];
        assert!(collate(&records).is_err());
    }

    #[test]
    fn identical_answers_at_temperature_zero_are_reported_as_equivalent() {
        let probes = || vec![probe(50, true, "a", 100.0), probe(950, true, "b", 100.0)];
        let table = collate(&[record(BASELINE_ARM, probes()), record("q8-32k", probes())]).unwrap();

        let q8 = &table[1];
        assert_eq!(q8.identical_answers, 2);
        assert_eq!(q8.answers_compared, 2);
        assert!(q8.lost_depths.is_empty());
        assert!(q8.verdict.contains("equivalent"), "{}", q8.verdict);
        assert!((q8.total_latency_ratio - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_needle_the_baseline_recalled_and_the_arm_lost_is_a_regression() {
        let baseline = record(
            BASELINE_ARM,
            vec![probe(50, true, "a", 100.0), probe(950, true, "b", 100.0)],
        );
        let q4 = record(
            "q4-32k",
            vec![probe(50, true, "a", 100.0), probe(950, false, "?", 100.0)],
        );
        let table = collate(&[baseline, q4]).unwrap();

        assert_eq!(table[1].lost_depths, vec![950]);
        assert_eq!(table[1].recalled, 1);
        assert!(
            table[1].verdict.contains("quality regression"),
            "{}",
            table[1].verdict
        );
    }

    #[test]
    fn a_faster_arm_that_swapped_is_not_reported_as_a_win() {
        let baseline = record(BASELINE_ARM, vec![probe(50, true, "a", 200.0)]);
        let mut swapping = record("q4-32k", vec![probe(50, true, "a", 100.0)]);
        swapping.swap_used_delta_bytes = 512 * 1024 * 1024;
        let table = collate(&[baseline, swapping]).unwrap();

        assert!((table[1].total_latency_ratio - 0.5).abs() < 1e-9);
        assert!(
            table[1].verdict.contains("swap"),
            "swap growth must survive into the verdict: {}",
            table[1].verdict
        );
    }

    #[test]
    fn arms_that_probed_different_depths_only_compare_the_shared_ones() {
        let baseline = record(
            BASELINE_ARM,
            vec![probe(50, true, "a", 100.0), probe(500, true, "b", 100.0)],
        );
        let partial = record("q4-32k", vec![probe(50, true, "a", 100.0)]);
        let table = collate(&[baseline, partial]).unwrap();

        assert_eq!(table[1].answers_compared, 1);
        assert!(table[1].lost_depths.is_empty());
    }

    // ── Reporting passes ────────────────────────────────────────────────────

    /// Print the planning table for a reference machine. Needs no model, so it
    /// answers "will this arm fit" before anyone downloads 4.7 GiB.
    ///
    /// ```sh
    /// RAMDOC_BENCH_RAM_GIB=16 \
    ///   cargo test plan_inference_profiles -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "prints the reference-machine planning table"]
    fn plan_inference_profiles() {
        let ram_gib: u64 = std::env::var("RAMDOC_BENCH_RAM_GIB")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(16);
        let ram = ram_gib * GIB;
        let governor = qwen3_8b_reference_governor(ram);
        let plans: Vec<PlanReport> = SWEEP_ARMS
            .iter()
            .map(|arm| plan_arm(&governor, arm, ram))
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "reference_machine_gib": ram_gib,
                "model": "Qwen3-8B-Q4_K_M.gguf",
                "inference_budget_bytes": governor.inference_budget_bytes(),
                "arms": plans,
            }))
            .expect("planning table serializes")
        );
    }

    // ── Hardware passes ─────────────────────────────────────────────────────

    /// Measure one inference profile. One arm per process: `LlamaBackend::init`
    /// is a once-per-process call, and resident-memory comparison across arms
    /// is only meaningful in a fresh process.
    ///
    /// ```sh
    /// RAMDOC_BENCH_MODEL=/abs/path/model.gguf RAMDOC_BENCH_PROFILE=q4-32k \
    /// RAMDOC_BENCH_OUT=/tmp/sweep.jsonl \
    ///   cargo test benchmark_inference_profile --release -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "requires a local GGUF model"]
    fn benchmark_inference_profile() {
        let model_path = PathBuf::from(
            std::env::var("RAMDOC_BENCH_MODEL").expect("RAMDOC_BENCH_MODEL is required"),
        );
        let model_name = model_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("benchmark.gguf")
            .to_string();
        let arm =
            std::env::var("RAMDOC_BENCH_PROFILE").unwrap_or_else(|_| BASELINE_ARM.to_string());
        // Fraction of the context the haystack fills, in percent.
        let fill_percent: usize = std::env::var("RAMDOC_BENCH_FILL")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(75);

        let swap_before = swap_used_bytes();
        let page_ins_before = page_ins();

        let engine = LlmEngine::load_with_profile(model_path, model_name.clone(), &arm)
            .unwrap_or_else(|error| panic!("load profile '{arm}': {error}"));
        // Weights only: `load_with_profile` allocates no context, so the KV
        // cache this benchmark exists to size does not exist yet.
        let resident_after_model_load = resident_bytes();
        let requested = engine
            .status()
            .inference_config
            .expect("a loaded engine reports its effective configuration");

        // Reserve the completion the profile itself reserves, so the probe is
        // rejected by the context budget only if the haystack is genuinely too
        // long for this arm. Context size is fixed at load; a Flash Attention
        // or KV-type fallback never changes it, so sizing the haystack before
        // the first context exists is safe.
        let budget = requested
            .context_size
            .saturating_sub(requested.completion_headroom.max(PROBE_MAX_TOKENS))
            * fill_percent
            / 100;
        let haystack = haystack_for_tokens(budget, &|text| engine.count_tokens(text));

        let mut resident_after_first_context = 0;
        let mut peak_resident = resident_after_model_load;
        let mut probes = Vec::new();
        for needle in NEEDLES {
            let prompt = format!(
                "Patientenakte:\n{haystack}\n\nFrage: {}\nAntworte in einem Satz.",
                needle.question
            );
            let answer = engine
                .generate(
                    super::super::prompts::SYSTEM_PROMPT_DE,
                    &prompt,
                    PROBE_MAX_TOKENS,
                    0.0,
                )
                .unwrap_or_else(|error| {
                    panic!("probe at depth {}: {error}", needle.depth_permille)
                });
            let stats = engine
                .last_generation_stats()
                .expect("a completed generation records stats");
            let resident = resident_bytes();
            if resident_after_first_context == 0 {
                resident_after_first_context = resident;
            }
            peak_resident = peak_resident.max(resident);
            probes.push(ProbeRecord {
                depth_permille: needle.depth_permille,
                question: needle.question.to_string(),
                prompt_tokens: stats.prompt_tokens,
                recalled: answer.contains(needle.needle),
                answer: answer.trim().to_string(),
                ttft_ms: stats.ttft_ms,
                prefill_ms: stats.prefill_ms,
                total_latency_ms: stats.total_latency_ms,
                tps: stats.tps,
            });
        }

        // Re-read the configuration *after* the probes: a Flash Attention or
        // KV-type fallback is only chosen when the first context is created, so
        // the configuration read at load time is the requested one, not
        // necessarily the one these numbers were measured under.
        let effective = engine
            .status()
            .inference_config
            .expect("a loaded engine reports its effective configuration");
        let governor = effective.memory_governor.as_ref();
        let record = ArmRecord {
            arm: arm.clone(),
            model: model_name,
            effective_profile: effective.profile.clone(),
            context_size: effective.context_size,
            kv_cache: effective.kv_cache_k.clone(),
            n_batch: effective.n_batch,
            n_ubatch: effective.n_ubatch,
            flash_attention: effective.flash_attention.clone(),
            fallback: effective.fallback.clone(),
            fallback_code: effective.fallback_code.clone(),
            estimated_total_bytes: governor.map(|g| g.estimate.total_bytes),
            inference_budget_bytes: governor.map(|g| g.inference_budget_bytes),
            resident_after_model_load_bytes: resident_after_model_load,
            resident_after_first_context_bytes: resident_after_first_context,
            peak_resident_bytes: peak_resident,
            swap_used_delta_bytes: swap_used_bytes() as i64 - swap_before as i64,
            page_ins_delta: page_ins().saturating_sub(page_ins_before),
            probes,
        };

        let line = serde_json::to_string(&record).expect("arm record serializes");
        println!("{line}");
        if let Ok(path) = std::env::var("RAMDOC_BENCH_OUT") {
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .unwrap_or_else(|error| panic!("open {path}: {error}"));
            writeln!(file, "{line}").unwrap_or_else(|error| panic!("append to {path}: {error}"));
        }
    }

    /// Turn a sweep written by `benchmark_inference_profile` into the
    /// comparison table. Needs no model, only the JSONL.
    ///
    /// ```sh
    /// RAMDOC_BENCH_OUT=/tmp/sweep.jsonl \
    ///   cargo test collate_profile_benchmark -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "reads a sweep produced by benchmark_inference_profile"]
    fn collate_profile_benchmark() {
        let path = std::env::var("RAMDOC_BENCH_OUT").expect("RAMDOC_BENCH_OUT is required");
        let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"));
        let records: Vec<ArmRecord> = raw
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).expect("each line is an arm record"))
            .collect();

        let table = collate(&records).unwrap_or_else(|error| panic!("{error}"));
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "baseline": BASELINE_ARM,
                "arms": table,
            }))
            .expect("comparison table serializes")
        );
    }
}
