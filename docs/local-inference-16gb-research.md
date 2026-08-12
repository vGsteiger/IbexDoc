# Local inference memory governor

The memory governor plans local GGUF inference before llama.cpp loads model
tensors. It reserves one third of physical memory, clamped to 4.5–6 GiB, for
macOS and the rest of RamDoc; a 16 GiB Mac therefore has an approximately
10.7 GiB inference budget.

For each GGUF it reads `general.architecture` plus the architecture-specific
layer, embedding, attention-head, KV-head and context fields. KV cache is
estimated as `context × layers × KV heads × head dimension × K/V bytes`, with
a 15% margin. This deliberately models grouped-query attention correctly.
Weights use 110% of the GGUF file size plus 128 MiB. Graph/scratch, allocator,
tokenizer and retrieval overhead are also reserved. The result is visible in
`get_engine_status.inference_config.memory_governor`.

`load_model` defaults to the `governed` profile. Named profiles (`f16-32k`,
`q8-32k`, and `q4-32k`) remain research overrides, but an override outside the
declared budget is refused before model allocation. Loading stays serialized
and drains the prior engine before a swap, preventing overlapping model and KV
allocations.

Whether a named profile is worth making the default is decided by the
inference-profile sweep, not by the planner alone. See
[`inference-profile-benchmark.md`](inference-profile-benchmark.md), which
records the reference-machine plan for Qwen3-8B on 16 GiB: 32K fits only with a
quantized KV cache, and the KV cache is the component that limits it.

## Calibration protocol

Run the ignored `benchmark_cold_and_warm_contexts` test with one GGUF from at
least three materially different architectures (for example a GQA Llama/Qwen,
a full-attention Gemma, and a recurrent or hybrid model). Record peak RSS from
fresh cold processes and compare it with `memory_governor.estimate.total_bytes`.
Until measurements show otherwise, the planner is intentionally conservative:
an observed peak greater than its estimate is a release blocker. Sustained swap
or macOS memory-pressure warnings are failure signals, not capacity to use.
