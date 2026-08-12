//! Conservative, architecture-aware inference memory planning.
//!
//! GGUF headers are read without allocating model tensors.  The estimate is a
//! planning guardrail, not a replacement for llama.cpp allocation failures:
//! all estimates deliberately include a safety margin and are surfaced to the
//! caller for research overrides.

use super::inference::{FlashAttentionMode, InferenceProfile, KvCacheQuantization};
use crate::error::AppError;
use llama_cpp_2::gguf::GgufContext;
use serde::{Deserialize, Serialize};
use std::path::Path;

const MIB: u64 = 1024 * 1024;
const GIB: u64 = 1024 * MIB;
const MIN_CONTEXT: usize = 4_096;
const DEFAULT_BATCH: u32 = 2_048;
const DEFAULT_UBATCH: u32 = 512;
const HEADROOM: usize = 4_096;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GgufArchitecture {
    pub architecture: String,
    pub layers: u32,
    pub embedding_length: u32,
    pub attention_heads: u32,
    pub kv_heads: u32,
    pub native_context: usize,
    pub recurrent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryEstimate {
    pub weights_bytes: u64,
    pub kv_cache_bytes: u64,
    pub graph_bytes: u64,
    pub runtime_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryGovernorDiagnostics {
    pub mode: String,
    pub architecture: GgufArchitecture,
    pub total_ram_bytes: u64,
    pub reserved_system_bytes: u64,
    pub inference_budget_bytes: u64,
    pub estimate: MemoryEstimate,
    pub safe: bool,
    pub reason: String,
}

/// Metadata and file-size based planner.  It is intentionally pure so its
/// calibration can be tested against the benchmark harness without Metal.
#[derive(Debug, Clone)]
pub struct MemoryGovernor {
    architecture: GgufArchitecture,
    weight_bytes: u64,
    total_ram_bytes: u64,
}

impl MemoryGovernor {
    pub fn inspect(path: &Path, total_ram_bytes: u64) -> Result<Self, AppError> {
        let file_bytes = std::fs::metadata(path)
            .map_err(|e| {
                AppError::Llm(format!("Failed to inspect model '{}': {e}", path.display()))
            })?
            .len();
        let gguf = GgufContext::from_file(path).ok_or_else(|| {
            AppError::Llm(format!(
                "Failed to read GGUF metadata from '{}'",
                path.display()
            ))
        })?;
        let architecture_name =
            string(&gguf, "general.architecture").unwrap_or_else(|| "unknown".into());
        let prefix = architecture_name.as_str();
        let layers = number(&gguf, &format!("{prefix}.block_count")).unwrap_or(0) as u32;
        let embedding_length =
            number(&gguf, &format!("{prefix}.embedding_length")).unwrap_or(0) as u32;
        let attention_heads =
            number(&gguf, &format!("{prefix}.attention.head_count")).unwrap_or(0) as u32;
        let kv_heads = number(&gguf, &format!("{prefix}.attention.head_count_kv"))
            .unwrap_or(attention_heads as u64) as u32;
        let native_context =
            number(&gguf, &format!("{prefix}.context_length")).unwrap_or(0) as usize;
        let recurrent = matches!(prefix, "mamba" | "rwkv" | "jamba")
            || number(&gguf, &format!("{prefix}.ssm.conv_kernel")).is_some();
        if native_context == 0
            || layers == 0
            || embedding_length == 0
            || attention_heads == 0
            || kv_heads == 0
        {
            return Err(AppError::Llm(format!(
                "GGUF '{}' is missing architecture dimensions required for safe memory planning",
                path.display()
            )));
        }
        Ok(Self {
            architecture: GgufArchitecture {
                architecture: architecture_name,
                layers,
                embedding_length,
                attention_heads,
                kv_heads,
                native_context,
                recurrent,
            },
            // Loaded unified-memory weights regularly exceed the file footprint.
            weight_bytes: file_bytes
                .saturating_mul(110)
                .saturating_div(100)
                .saturating_add(128 * MIB),
            total_ram_bytes,
        })
    }

    pub fn plan(
        &self,
        requested: Option<&InferenceProfile>,
    ) -> (InferenceProfile, MemoryGovernorDiagnostics) {
        let reserve = system_reserve(self.total_ram_bytes);
        let budget = self.total_ram_bytes.saturating_sub(reserve);
        let candidates = match requested {
            Some(profile) => vec![profile.clone()],
            None => vec![
                profile("governed-f16", 16_384, KvCacheQuantization::F16),
                profile("governed-q8", 32_768, KvCacheQuantization::Q8),
                profile("governed-q4", 32_768, KvCacheQuantization::Q4),
                profile("governed-minimum", MIN_CONTEXT, KvCacheQuantization::Q4),
            ],
        };
        let native = self.architecture.native_context;
        let mut last = None;
        for candidate in candidates {
            let candidate = candidate.resolved_for_model(native).unwrap_or(candidate);
            let estimate = self.estimate(&candidate);
            if estimate.total_bytes <= budget {
                let reason = format!(
                    "{} fits within the {} GiB inference budget",
                    candidate.name,
                    gib(budget)
                );
                return (
                    candidate,
                    self.diagnostics("automatic", budget, estimate, true, reason),
                );
            }
            last = Some((candidate, estimate));
        }
        let (candidate, estimate) = last.expect("governor candidates are non-empty");
        let reason = format!(
            "Even the minimum profile exceeds the {} GiB budget; refusing an unsafe automatic load",
            gib(budget)
        );
        (
            candidate,
            self.diagnostics("automatic", budget, estimate, false, reason),
        )
    }

    pub fn estimate(&self, profile: &InferenceProfile) -> MemoryEstimate {
        // K and V: layers × KV heads × head dimension × two tensors.  This
        // captures GQA/MQA rather than assuming all attention heads are KV heads.
        let head_dim =
            (self.architecture.embedding_length / self.architecture.attention_heads.max(1)) as u64;
        let kv_element_bytes_x2 = match profile.kv_cache {
            KvCacheQuantization::F16 => 4,
            KvCacheQuantization::Q8 => 2,
            KvCacheQuantization::Q4 => 1,
        };
        let kv_cache_bytes = (profile.n_ctx as u64)
            .saturating_mul(self.architecture.layers as u64)
            .saturating_mul(self.architecture.kv_heads as u64)
            .saturating_mul(head_dim)
            .saturating_mul(kv_element_bytes_x2)
            .saturating_mul(115)
            / 100;
        // Metal graphs and temporary activation buffers scale primarily with
        // micro-batch and embedding width.  Fixed overhead covers tokenizer,
        // allocator fragmentation and the app's retrieval service.
        let graph_bytes = 512 * MIB
            + (profile.n_ubatch as u64)
                .saturating_mul(self.architecture.embedding_length as u64)
                .saturating_mul(64);
        let runtime_bytes = 640 * MIB;
        let weights_bytes = self.weight_bytes;
        MemoryEstimate {
            weights_bytes,
            kv_cache_bytes,
            graph_bytes,
            runtime_bytes,
            total_bytes: weights_bytes
                .saturating_add(kv_cache_bytes)
                .saturating_add(graph_bytes)
                .saturating_add(runtime_bytes),
        }
    }

    fn diagnostics(
        &self,
        mode: &str,
        budget: u64,
        estimate: MemoryEstimate,
        safe: bool,
        reason: String,
    ) -> MemoryGovernorDiagnostics {
        MemoryGovernorDiagnostics {
            mode: mode.into(),
            architecture: self.architecture.clone(),
            total_ram_bytes: self.total_ram_bytes,
            reserved_system_bytes: system_reserve(self.total_ram_bytes),
            inference_budget_bytes: budget,
            estimate,
            safe,
            reason,
        }
    }
}

fn profile(name: &str, n_ctx: usize, kv_cache: KvCacheQuantization) -> InferenceProfile {
    InferenceProfile {
        name: name.into(),
        n_ctx,
        kv_cache,
        n_batch: DEFAULT_BATCH.min(n_ctx as u32),
        n_ubatch: DEFAULT_UBATCH.min(n_ctx as u32),
        completion_headroom: HEADROOM.min(n_ctx / 4).max(1),
        flash_attention: FlashAttentionMode::Enabled,
    }
}

fn system_reserve(total: u64) -> u64 {
    (total / 3).clamp(4 * GIB + 512 * MIB, 6 * GIB)
}
fn gib(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / GIB as f64)
}

fn string(gguf: &GgufContext, key: &str) -> Option<String> {
    let index = gguf.find_key(key);
    (index >= 0)
        .then(|| gguf.val_str(index).map(str::to_owned))
        .flatten()
}
fn number(gguf: &GgufContext, key: &str) -> Option<u64> {
    let index = gguf.find_key(key);
    if index < 0 {
        return None;
    }
    match gguf.kv_type(index) {
        llama_cpp_sys_2::GGUF_TYPE_UINT32 => Some(gguf.val_u32(index) as u64),
        llama_cpp_sys_2::GGUF_TYPE_UINT64 => Some(gguf.val_u64(index)),
        llama_cpp_sys_2::GGUF_TYPE_INT32 => u64::try_from(gguf.val_i32(index)).ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn governor(ram: u64) -> MemoryGovernor {
        MemoryGovernor {
            architecture: GgufArchitecture {
                architecture: "llama".into(),
                layers: 32,
                embedding_length: 4096,
                attention_heads: 32,
                kv_heads: 8,
                native_context: 32_768,
                recurrent: false,
            },
            weight_bytes: 5 * GIB,
            total_ram_bytes: ram,
        }
    }
    #[test]
    fn gqa_kv_estimate_uses_kv_heads_not_attention_heads() {
        let p = profile("test", 16_384, KvCacheQuantization::F16);
        let gqa = governor(16 * GIB).estimate(&p).kv_cache_bytes;
        let mut full_attention = governor(16 * GIB);
        full_attention.architecture.kv_heads = full_attention.architecture.attention_heads;
        assert_eq!(full_attention.estimate(&p).kv_cache_bytes, gqa * 4);
    }
    #[test]
    fn sixteen_gib_plan_reserves_system_memory_and_is_safe() {
        let (_, d) = governor(16 * GIB).plan(None);
        assert!((4 * GIB + 512 * MIB..=6 * GIB).contains(&d.reserved_system_bytes));
        assert!(d.safe);
        assert!(d.estimate.total_bytes <= d.inference_budget_bytes);
    }
    #[test]
    fn explicit_research_override_is_reported_even_when_unsafe() {
        let p = profile("f16-32k", 32_768, KvCacheQuantization::F16);
        let (_, d) = governor(8 * GIB).plan(Some(&p));
        assert!(!d.safe);
    }
}
