# Persistent llama context benchmark

Issue #400 includes an ignored, hardware-backed benchmark because CI does not
ship an approved GGUF model. It compares the reference cold path with two
identical leased-session calls and verifies deterministic-answer equivalence at
temperature 0.

Run on the target Mac:

```sh
cd dokassist/src-tauri
RAMDOC_BENCH_MODEL=/absolute/path/to/model.gguf \
  cargo test benchmark_cold_and_warm_contexts --release -- --ignored --nocapture
```

The emitted JSON reports, for both cold and warm calls:

- time to first token (`ttft_ms`)
- total latency (`total_latency_ms`)
- prompt prefill time (`prefill_ms`)
- prompt tokens evaluated and reused
- estimated prefill time saved
- completion throughput
- process peak resident memory (`peak_rss_bytes`)

Use the same model, inference profile, prompt, and idle machine for comparisons.
Peak RSS is process-lifetime high-water memory, so run cold and warm benchmark
captures in fresh processes when comparing absolute peak-memory changes.
Sampling equivalence is exact in this deterministic harness. For non-zero
temperature production sampling, compare semantic/task scores rather than raw
text equality.
