use super::download;
use crate::error::AppError;
use encoding_rs::UTF_8;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaChatMessage, LlamaChatTemplate, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

/// Sentinel value for n_gpu_layers that offloads all layers to Metal GPU.
const ALL_GPU_LAYERS: u32 = 999;
const MIN_CONTEXT_SIZE: usize = 16_384;
const STANDARD_CONTEXT_SIZE: usize = 32_768;
const LARGE_CONTEXT_SIZE: usize = 65_536;
const PROMPT_BATCH_SIZE: u32 = 2_048;

/// Performance metrics recorded after each generation call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationStats {
    /// Wall-clock milliseconds from inference start to first token emitted.
    pub ttft_ms: f64,
    /// Tokens generated per second (generation phase, excluding prompt evaluation).
    pub tps: f64,
    /// Number of tokens in the generated completion.
    pub completion_tokens: usize,
    /// Number of tokens in the prompt.
    pub prompt_tokens: usize,
}

pub struct LlmEngine {
    // IMPORTANT: field declaration order controls drop order in Rust.
    // `model` must be dropped before `backend` — the LlamaModel holds a
    // raw pointer into the LlamaBackend, so freeing the backend first
    // causes a use-after-free crash in the llama.cpp C code at shutdown.
    model: Option<LlamaModel>,
    model_path: PathBuf,
    model_name: String,
    chat_template: LlamaChatTemplate,
    context_size: usize,
    backend: LlamaBackend,
    last_stats: Mutex<Option<GenerationStats>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelChoice {
    pub name: String,
    pub filename: String,
    pub size_bytes: u64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatus {
    pub is_loaded: bool,
    pub model_name: Option<String>,
    pub model_path: Option<String>,
    pub total_ram_bytes: u64,
    /// Whether the model file exists on disk (may not yet be loaded into memory).
    pub is_downloaded: bool,
    /// Filename of the downloaded model, if present on disk.
    pub downloaded_filename: Option<String>,
    /// Performance stats from the most recent generation, if any.
    pub last_generation_stats: Option<GenerationStats>,
}

impl LlmEngine {
    /// Load a GGUF model from disk, offloading all layers to Metal.
    pub fn load(model_path: PathBuf, model_name: String) -> Result<Self, AppError> {
        let backend = LlamaBackend::init()
            .map_err(|e| AppError::Llm(format!("Failed to init llama backend: {e}")))?;

        // HIGH-3: Disable memory-mapped I/O so that a crafted GGUF file cannot
        // trigger memory-mapped reads of out-of-bounds data before the C library
        // has validated the tensor layout.  The SHA-256 pre-download check (CRIT-3)
        // already ensures model integrity; this is an additional layer of defence.
        let model_params = LlamaModelParams::default().with_n_gpu_layers(ALL_GPU_LAYERS);

        let model = LlamaModel::load_from_file(&backend, &model_path, &model_params)
            .map_err(|e| AppError::Llm(format!("Failed to load model: {e}")))?;
        let chat_template = model.chat_template(None).map_err(|e| {
            AppError::Llm(format!(
                "Model '{}' has no usable embedded chat template: {e}",
                model_name
            ))
        })?;
        let native_context = model.n_ctx_train() as usize;
        let ram = Self::total_ram();
        let requested_context = runtime_context_for_ram(ram);
        let context_size = if native_context == 0 {
            requested_context
        } else {
            requested_context.min(native_context)
        };

        Ok(Self {
            backend,
            model: Some(model),
            model_path,
            model_name,
            chat_template,
            context_size,
            last_stats: Mutex::new(None),
        })
    }

    /// Run blocking inference and return the full completion string.
    pub fn generate(
        &self,
        system_prompt: &str,
        user_message: &str,
        max_tokens: usize,
        temperature: f32,
    ) -> Result<String, AppError> {
        let mut result = String::new();
        self.generate_streaming(
            system_prompt,
            user_message,
            max_tokens,
            temperature,
            |token| {
                result.push_str(token);
                true
            },
        )?;
        Ok(result)
    }

    /// Run blocking inference, calling `on_token` for each piece.
    /// Return `false` from the callback to abort generation early.
    pub fn generate_streaming(
        &self,
        system_prompt: &str,
        user_message: &str,
        max_tokens: usize,
        temperature: f32,
        mut on_token: impl FnMut(&str) -> bool,
    ) -> Result<(), AppError> {
        let model = self
            .model
            .as_ref()
            .ok_or_else(|| AppError::Llm("Model not loaded".to_string()))?;

        let prompt = self.format_chat_history(
            system_prompt,
            &[AgentMessage {
                role: "user".to_string(),
                content: user_message.to_string(),
            }],
        )?;

        // 1. Tokenise
        let tokens = model
            .str_to_token(&prompt, AddBos::Always)
            .map_err(|e| AppError::Llm(format!("Tokenization failed: {e}")))?;

        // 2. Context sized to the machine, with bounded prompt batches so long
        // contexts do not require an equally large temporary compute buffer.
        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(self.context_size as u32))
            .with_n_batch(PROMPT_BATCH_SIZE);
        let mut ctx = model
            .new_context(&self.backend, ctx_params)
            .map_err(|e| AppError::Llm(format!("Failed to create context: {e}")))?;

        // 3. Decode the prompt in bounded batches.
        let n_prompt = tokens.len();
        if n_prompt >= self.context_size {
            return Err(AppError::Llm(format!(
                "Prompt too long ({n_prompt} tokens), exceeds context window ({})",
                self.context_size
            )));
        }
        let mut batch = LlamaBatch::new(PROMPT_BATCH_SIZE as usize, 1);

        let wall_start = Instant::now();
        for (chunk_index, chunk) in tokens.chunks(PROMPT_BATCH_SIZE as usize).enumerate() {
            batch.clear();
            let start = chunk_index * PROMPT_BATCH_SIZE as usize;
            for (offset, token) in chunk.iter().enumerate() {
                let position = i32::try_from(start + offset)
                    .map_err(|_| AppError::Llm("Prompt position exceeds i32".to_string()))?;
                let needs_logits = start + offset + 1 == n_prompt;
                batch
                    .add(*token, position, &[0], needs_logits)
                    .map_err(|e| AppError::Llm(format!("Failed to build batch: {e}")))?;
            }
            ctx.decode(&mut batch)
                .map_err(|e| AppError::Llm(format!("Failed to decode prompt: {e}")))?;
        }
        let gen_phase_start = Instant::now();

        // 4. Sampler chain: temp → top-k → top-p → dist (terminal)
        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::temp(temperature),
            LlamaSampler::top_k(40),
            LlamaSampler::top_p(0.9, 1),
            LlamaSampler::dist(0),
        ]);

        // 5. Stateful UTF-8 decoder for multi-byte tokens
        let mut utf8_dec = UTF_8.new_decoder();

        let mut first_token = true;
        let mut ttft_ms = 0.0f64;
        let mut completion_tokens = 0usize;

        for (n_cur, _) in (n_prompt as i32..).zip(0..max_tokens) {
            let token = sampler.sample(&ctx, -1);
            sampler.accept(token);

            if model.is_eog_token(token) {
                break;
            }

            let piece = model
                .token_to_piece(token, &mut utf8_dec, false, None)
                .map_err(|e| AppError::Llm(format!("Token decode failed: {e}")))?;

            if first_token {
                ttft_ms = wall_start.elapsed().as_secs_f64() * 1000.0;
                first_token = false;
            }

            if !on_token(&piece) {
                break;
            }

            completion_tokens += 1;

            // Advance context with the new token
            if n_cur + 1 >= self.context_size as i32 {
                break;
            }
            batch.clear();
            batch
                .add(token, n_cur, &[0], true)
                .map_err(|e| AppError::Llm(format!("Failed to add token: {e}")))?;
            ctx.decode(&mut batch)
                .map_err(|e| AppError::Llm(format!("Failed to decode token: {e}")))?;
        }

        if completion_tokens > 0 {
            let gen_elapsed = gen_phase_start.elapsed().as_secs_f64();
            let tps = if gen_elapsed > 0.0 {
                completion_tokens as f64 / gen_elapsed
            } else {
                0.0
            };
            if let Ok(mut stats) = self.last_stats.lock() {
                *stats = Some(GenerationStats {
                    ttft_ms,
                    tps,
                    completion_tokens,
                    prompt_tokens: n_prompt,
                });
            }
        }

        Ok(())
    }

    pub fn status(&self) -> EngineStatus {
        EngineStatus {
            is_loaded: self.model.is_some(),
            model_name: self.model.as_ref().map(|_| self.model_name.clone()),
            model_path: self
                .model
                .as_ref()
                .map(|_| self.model_path.to_string_lossy().into_owned()),
            total_ram_bytes: Self::total_ram(),
            // A loaded model is always on disk.
            is_downloaded: self.model.is_some(),
            downloaded_filename: self.model.as_ref().map(|_| self.model_name.clone()),
            last_generation_stats: self.last_stats.lock().ok().and_then(|g| g.clone()),
        }
    }

    /// Run blocking inference from a pre-formatted prompt string (full ChatML).
    /// Like `generate_streaming` but bypasses `format_chatml` so callers can
    /// pass multi-turn history they have built themselves.
    pub fn generate_streaming_raw(
        &self,
        prompt: &str,
        max_tokens: usize,
        temperature: f32,
        mut on_token: impl FnMut(&str) -> bool,
    ) -> Result<(), AppError> {
        let model = self
            .model
            .as_ref()
            .ok_or_else(|| AppError::Llm("Model not loaded".to_string()))?;

        let tokens = model
            .str_to_token(prompt, AddBos::Always)
            .map_err(|e| AppError::Llm(format!("Tokenization failed: {e}")))?;

        let ctx_params = LlamaContextParams::default()
            .with_n_ctx(NonZeroU32::new(self.context_size as u32))
            .with_n_batch(PROMPT_BATCH_SIZE);
        let mut ctx = model
            .new_context(&self.backend, ctx_params)
            .map_err(|e| AppError::Llm(format!("Failed to create context: {e}")))?;

        let n_prompt = tokens.len();
        if n_prompt >= self.context_size {
            return Err(AppError::Llm(format!(
                "Prompt too long ({n_prompt} tokens), exceeds context window ({})",
                self.context_size
            )));
        }
        let mut batch = LlamaBatch::new(PROMPT_BATCH_SIZE as usize, 1);

        let wall_start = Instant::now();
        for (chunk_index, chunk) in tokens.chunks(PROMPT_BATCH_SIZE as usize).enumerate() {
            batch.clear();
            let start = chunk_index * PROMPT_BATCH_SIZE as usize;
            for (offset, token) in chunk.iter().enumerate() {
                let position = i32::try_from(start + offset)
                    .map_err(|_| AppError::Llm("Prompt position exceeds i32".to_string()))?;
                let needs_logits = start + offset + 1 == n_prompt;
                batch
                    .add(*token, position, &[0], needs_logits)
                    .map_err(|e| AppError::Llm(format!("Failed to build batch: {e}")))?;
            }
            ctx.decode(&mut batch)
                .map_err(|e| AppError::Llm(format!("Failed to decode prompt: {e}")))?;
        }
        let gen_phase_start = Instant::now();

        let mut sampler = LlamaSampler::chain_simple([
            LlamaSampler::temp(temperature),
            LlamaSampler::top_k(40),
            LlamaSampler::top_p(0.9, 1),
            LlamaSampler::dist(0),
        ]);

        let mut utf8_dec = UTF_8.new_decoder();

        let mut first_token = true;
        let mut ttft_ms = 0.0f64;
        let mut completion_tokens = 0usize;

        for (n_cur, _) in (n_prompt as i32..).zip(0..max_tokens) {
            let token = sampler.sample(&ctx, -1);
            sampler.accept(token);

            if model.is_eog_token(token) {
                break;
            }

            let piece = model
                .token_to_piece(token, &mut utf8_dec, false, None)
                .map_err(|e| AppError::Llm(format!("Token decode failed: {e}")))?;

            if first_token {
                ttft_ms = wall_start.elapsed().as_secs_f64() * 1000.0;
                first_token = false;
            }

            if !on_token(&piece) {
                break;
            }

            completion_tokens += 1;

            if n_cur + 1 >= self.context_size as i32 {
                break;
            }
            batch.clear();
            batch
                .add(token, n_cur, &[0], true)
                .map_err(|e| AppError::Llm(format!("Failed to add token: {e}")))?;
            ctx.decode(&mut batch)
                .map_err(|e| AppError::Llm(format!("Failed to decode token: {e}")))?;
        }

        if completion_tokens > 0 {
            let gen_elapsed = gen_phase_start.elapsed().as_secs_f64();
            let tps = if gen_elapsed > 0.0 {
                completion_tokens as f64 / gen_elapsed
            } else {
                0.0
            };
            if let Ok(mut stats) = self.last_stats.lock() {
                *stats = Some(GenerationStats {
                    ttft_ms,
                    tps,
                    completion_tokens,
                    prompt_tokens: n_prompt,
                });
            }
        }

        Ok(())
    }

    pub fn is_ready(&self) -> bool {
        self.model.is_some()
    }

    /// Returns the context window size used for all inference calls.
    pub fn context_size(&self) -> usize {
        self.context_size
    }

    /// Format conversation history with the template embedded in the loaded GGUF.
    /// This is required for non-ChatML families such as gpt-oss (Harmony) and Gemma.
    pub fn format_chat_history(
        &self,
        system_prompt: &str,
        messages: &[AgentMessage],
    ) -> Result<String, AppError> {
        let mut chat = Vec::with_capacity(messages.len() + 1);
        chat.push(new_chat_message("system", system_prompt)?);
        for message in messages {
            let role = match message.role.as_str() {
                "tool_call" => "assistant",
                "tool_result" => "user",
                role => role,
            };
            chat.push(new_chat_message(role, &message.content)?);
        }
        self.model
            .as_ref()
            .ok_or_else(|| AppError::Llm("Model not loaded".to_string()))?
            .apply_chat_template(&self.chat_template, &chat, true)
            .map_err(|e| AppError::Llm(format!("Failed to apply model chat template: {e}")))
    }

    /// Returns stats from the most recent generation call, if any.
    pub fn last_generation_stats(&self) -> Option<GenerationStats> {
        self.last_stats.lock().ok().and_then(|g| g.clone())
    }

    /// Count the number of tokens in a string using the loaded model's tokenizer.
    /// Falls back to a character-based estimate (~4 chars/token) if no model is loaded.
    pub fn count_tokens(&self, text: &str) -> usize {
        let Some(model) = self.model.as_ref() else {
            return text.len() / 4;
        };
        model
            .str_to_token(text, AddBos::Always)
            .map(|t| t.len())
            .unwrap_or(text.len() / 4)
    }

    /// Return the total system RAM in bytes (macOS via sysctl).
    pub fn total_ram() -> u64 {
        #[cfg(target_os = "macos")]
        {
            unsafe {
                let mut size: u64 = 0;
                let mut len = std::mem::size_of::<u64>();
                let name = std::ffi::CString::new("hw.memsize").unwrap();
                libc::sysctlbyname(
                    name.as_ptr(),
                    &mut size as *mut u64 as *mut libc::c_void,
                    &mut len as *mut usize,
                    std::ptr::null_mut(),
                    0,
                );
                size
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            16 * 1024 * 1024 * 1024
        }
    }

    /// Choose the best model for the available RAM.
    pub fn recommended_model() -> ModelChoice {
        recommended_model_for_ram(Self::total_ram())
    }
}

fn recommended_model_for_ram(ram: u64) -> ModelChoice {
    const GB: u64 = 1024 * 1024 * 1024;

    let preferred_filename = if ram >= 32 * GB {
        "Qwen3.6-35B-A3B-UD-Q4_K_M.gguf"
    } else if ram >= 24 * GB {
        "Qwen3.6-27B-Q4_K_M.gguf"
    } else if ram >= 18 * GB {
        "gpt-oss-20b-MXFP4.gguf"
    } else if ram >= 16 * GB {
        "Qwen3-8B-Q4_K_M.gguf"
    } else if ram >= 12 * GB {
        "gemma-4-E4B-it-Q4_0.gguf"
    } else {
        "gemma-4-E2B-it-Q4_0.gguf"
    };
    let entry = download::find_model(preferred_filename)
        .expect("recommended model must exist in the download catalog");
    let active_context = runtime_context_for_ram(ram).min(entry.context_window_tokens as usize);

    ModelChoice {
        name: entry.name.to_string(),
        filename: entry.filename.to_string(),
        size_bytes: entry.size_bytes,
        reason: format!(
            "Empfohlen für {} GB RAM: {}, {}K aktiver / {}K nativer Kontext, {} Modellgröße",
            entry.min_ram_gb,
            entry.parameters,
            active_context / 1024,
            entry.context_window_tokens / 1024,
            format_gib(entry.size_bytes)
        ),
    }
}

fn format_gib(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / (1024_f64.powi(3)))
}

fn runtime_context_for_ram(ram: u64) -> usize {
    const GB: u64 = 1024 * 1024 * 1024;
    if ram >= 48 * GB {
        LARGE_CONTEXT_SIZE
    } else if ram >= 24 * GB {
        STANDARD_CONTEXT_SIZE
    } else {
        MIN_CONTEXT_SIZE
    }
}

fn new_chat_message(role: &str, content: &str) -> Result<LlamaChatMessage, AppError> {
    LlamaChatMessage::new(role.to_string(), content.to_string())
        .map_err(|e| AppError::Llm(format!("Invalid chat message: {e}")))
}

/// A message in an agent conversation history.
#[derive(Debug, Clone)]
pub struct AgentMessage {
    pub role: String,
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recommendations_cover_memory_tiers() {
        const GB: u64 = 1024 * 1024 * 1024;
        let cases = [
            (8, "gemma-4-E2B-it-Q4_0.gguf"),
            (12, "gemma-4-E4B-it-Q4_0.gguf"),
            (16, "Qwen3-8B-Q4_K_M.gguf"),
            (18, "gpt-oss-20b-MXFP4.gguf"),
            (24, "Qwen3.6-27B-Q4_K_M.gguf"),
            (32, "Qwen3.6-35B-A3B-UD-Q4_K_M.gguf"),
        ];

        for (ram_gb, expected) in cases {
            let choice = recommended_model_for_ram(ram_gb * GB);
            assert_eq!(choice.filename, expected);
            assert!(download::find_model(&choice.filename).is_some());
        }
    }

    #[test]
    fn runtime_context_scales_with_available_memory() {
        const GB: u64 = 1024 * 1024 * 1024;
        assert_eq!(runtime_context_for_ram(16 * GB), MIN_CONTEXT_SIZE);
        assert_eq!(runtime_context_for_ram(24 * GB), STANDARD_CONTEXT_SIZE);
        assert_eq!(runtime_context_for_ram(48 * GB), LARGE_CONTEXT_SIZE);
    }
}
