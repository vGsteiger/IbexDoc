# Inference-profile benchmark

Issue #399 made context size, KV-cache type, Flash Attention and micro-batch
size configurable. This is the harness that decides whether any of those
settings should change, and the reference-machine result it produces today.

The harness lives in `dokassist/src-tauri/src/llm/profile_benchmark.rs` and has
three layers. Only the middle one needs hardware.

| Layer | Runs in CI | What it answers |
| --- | --- | --- |
| Planning | yes | Does this profile fit on a reference machine, and if not, what has to shrink? |
| Measurement | no (needs a GGUF) | What does it actually cost in recall, latency and resident memory? |
| Collation | yes | Did any arm lose quality against the baseline? |

## Arms

The sweep compares five profiles, baseline first.

| Arm | Context | KV cache | Role |
| --- | --- | --- | --- |
| `conservative` | 16K on a 16 GiB Mac | F16 | Baseline: what `LlmEngine::load` uses |
| `governed` | governor's choice | governor's choice | The shipped default |
| `f16-32k` | 32K | F16 | Long context at full KV precision |
| `q8-32k` | 32K | Q8_0 | Long context, half the KV cache |
| `q4-32k` | 32K | Q4_0 | Long context, quarter the KV cache |

All arms use `n_batch` 2048, `n_ubatch` 512 and request Flash Attention. When
the backend or model rejects Flash Attention or a quantized KV type, the engine
falls back in explicit stages and records `fallback_code` in the diagnostics;
the recorded arm carries whatever was actually in force, so a fallback can never
be mistaken for a measurement of the requested profile.

## Reference-machine plan: Qwen3-8B on 16 GiB

Produced by `plan_inference_profiles` from the same memory governor the loader
uses, against the approved `Qwen3-8B-Q4_K_M.gguf` entry (5.03 GB, 36 layers,
4096-wide, 32 attention heads over 8 KV heads, 32K native context). The
inference budget on a 16 GiB Mac is 10.67 GiB after the system reserve.

| Arm | Context | KV | KV cache | Total estimate | Fits | Limiting component |
| --- | --- | --- | --- | --- | --- | --- |
| `conservative` | 16,384 | F16 | 2.59 GiB | 9.11 GiB | yes | — |
| `governed` | 16,384 | F16 | 2.59 GiB | 9.11 GiB | yes | — |
| `f16-32k` | 32,768 | F16 | 5.18 GiB | 11.70 GiB | **no**, over by 1.03 GiB | `kv_cache` |
| `q8-32k` | 32,768 | Q8_0 | 2.59 GiB | 9.11 GiB | yes | — |
| `q4-32k` | 32,768 | Q4_0 | 1.29 GiB | 7.82 GiB | yes | — |

**The finding for acceptance criterion 3.** Qwen3-8B can be evaluated at 32K on
a 16 GiB reference machine, but only with a quantized KV cache. At F16 the plan
exceeds the budget by 1.03 GiB, and the limiting component is the KV cache: the
weights, graph and fixed runtime overhead are identical across all three 32K
arms, and only the KV cache changes. Q8_0 buys 32K for exactly the memory the
current 16K F16 configuration already uses.

"Limiting component" deliberately does not mean "largest component". For a
4-bit 8B model the weights are the largest line item in every arm, including the
ones that fit — reporting that would send a reader to shrink a context that was
never the problem. The harness names the weights only when the immovable part
(weights plus fixed runtime) alone exceeds the budget, which is the case where
the answer really is a smaller model.

Re-run the plan for a different machine:

```sh
cd dokassist/src-tauri
RAMDOC_BENCH_RAM_GIB=24 cargo test plan_inference_profiles -- --ignored --nocapture
```

## Measuring on hardware

```sh
./scripts/bench-inference-profiles.sh /absolute/path/to/Qwen3-8B-Q4_K_M.gguf
```

The script plans, then runs each arm in a **fresh process**, then collates. One
arm per process is required, not tidy: `LlamaBackend::init` may only be called
once per process, and resident memory only returns to a comparable baseline in a
new one. An arm the memory governor refuses is recorded as a refusal and the
sweep continues.

To run a single arm by hand:

```sh
cd dokassist/src-tauri
RAMDOC_BENCH_MODEL=/absolute/path/to/model.gguf \
RAMDOC_BENCH_PROFILE=q4-32k \
RAMDOC_BENCH_OUT=/tmp/sweep.jsonl \
  cargo test benchmark_inference_profile --release -- --ignored --nocapture
```

`RAMDOC_BENCH_FILL` (default 75) sets the percentage of the usable context the
probe prompt fills, so every arm is exercised at a comparable fraction of its
own window rather than at a fixed token count.

### What each arm records

The probe set is a needle-in-a-haystack over synthetic German session notes,
with one fact planted at 5%, 50% and 95% depth. Depth matters: a quantized KV
cache degrades attention over distance, so three depths separate "this model is
worse" from "the KV cache lost the far end of the context". Filler text carries
no digits or substrings that could be confused for a needle, so a recall hit can
only come from the planted fact. All probes run at temperature 0.

Per arm: the effective profile and any fallback, the governor estimate and
budget, three resident-memory readings, system swap growth, and page-ins. Per
probe: prompt tokens, recall, the answer text, TTFT, prefill, total latency and
throughput.

The three memory readings exist because loading a model does not allocate a
context: `load_with_profile` brings in the weights, and the KV cache only
appears when the first generation creates a context. So the harness records
resident memory after the model load (weights), after the first context
(weights plus KV cache plus graph) and at its peak across the probe set. The
difference between the first two is what the context configuration actually
costs, which is the number to compare against the governor's KV estimate.

Resident memory is read from `proc_pidinfo`, not `getrusage`. The `getrusage`
high-water mark used by the older benchmarks never falls, which makes it useless
for comparing arms; the resident figure drops when an engine is dropped.

The effective configuration is re-read *after* the probes, not at load time. A
Flash Attention or KV-type fallback is only chosen when the first context is
created, so a configuration read at load would report what was requested rather
than what these numbers were measured under.

### Reading the comparison

```sh
RAMDOC_BENCH_OUT=/tmp/sweep.jsonl \
  cargo test collate_profile_benchmark -- --ignored --nocapture
```

Collation compares each arm against `conservative` probe-by-probe, matching on
needle depth. The verdict is ordered so a real problem cannot be hidden by a
good-looking number:

1. **Quality regression** — a needle the baseline recalled and this arm did not.
   The lost depths are named. This outranks every latency or memory result.
2. **Swap growth** — recall held, but the run grew system swap. Reported as not
   fitting regardless of how fast it was. Sustained swap is a failure signal,
   not capacity to use.
3. **Equivalent** — byte-identical answers at temperature 0.
4. **Recall preserved, wording differs** — the expected outcome for a KV-cache
   change that costs nothing measurable.

Latency and throughput are reported as ratios against the baseline, so a sweep
on a different machine is still comparable.

## Interpreting a result

- A missing baseline arm makes the sweep uninterpretable; collation errors
  rather than printing a partial table.
- Exact answer equality is meaningful only at temperature 0 in this harness. For
  production sampling, compare task scores, not raw text.
- A measured peak above the governor's estimate is a release blocker, per
  [`local-inference-16gb-research.md`](local-inference-16gb-research.md). Record
  both and compare.
- Defaults do not change because an arm looks good once. The governor keeps
  preferring 16K F16 over 32K Q8 until a sweep on the reference machine shows no
  recall regression, no swap, and an acceptable latency ratio — and a test
  (`shipped_default_stays_conservative_on_the_reference_machine`) fails if that
  default is changed without a deliberate decision.
