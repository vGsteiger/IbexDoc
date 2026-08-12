use super::{
    engine::{AgentMessage, LlmEngine},
    prompts::{self, LetterType, ReportType},
    sanitize::{build_delimited_prompt, sanitize_for_prompt},
    utf8,
};
use crate::error::AppError;
use tauri::Emitter;

/// Maximum tokens the model may spend inside a `<think>` block before the block
/// is force-closed and generation continues with the actual report/letter/summary.
const MAX_THINK_TOKENS: usize = 1024;

/// Max tokens for the condensed-context summary output.
const SUMMARIZE_MAX_TOKENS: usize = 800;

/// Char limits for truncating inputs before the summarizer pass (prevents summarizer overflow).
const MAX_CONTEXT_CHARS: usize = 16_000;
const MAX_NOTES_CHARS: usize = 6_000;

/// Returns `true` when the model-formatted prompt would leave insufficient
/// output headroom in the active context window.
fn needs_summarization(
    engine: &LlmEngine,
    system_prompt: &str,
    patient_context: &str,
    session_notes: &str,
) -> Result<bool, AppError> {
    let message = AgentMessage {
        role: "user".to_string(),
        content: format!(
            "Patientenkontext:\n{patient_context}\n\nSitzungsnotizen:\n{session_notes}"
        ),
    };
    let formatted = engine.format_chat_history(system_prompt, &[message])?;
    let max_input_tokens = engine.context_size().saturating_sub(4_096 + 256);
    Ok(engine.count_tokens(&formatted) > max_input_tokens)
}

/// Condenses `patient_context` + `session_notes` into a shorter summary string
/// that fits within the context window.
fn run_summarization(
    engine: &LlmEngine,
    system_prompt: &str,
    patient_context: &str,
    session_notes: &str,
) -> Result<String, AppError> {
    let ctx = utf8::truncate_to_boundary(patient_context, MAX_CONTEXT_CHARS);
    let notes = utf8::truncate_to_boundary(session_notes, MAX_NOTES_CHARS);
    let summarization_msg = prompts::context_summarization_prompt(ctx, notes);
    engine.generate(system_prompt, &summarization_msg, SUMMARIZE_MAX_TOKENS, 0.3)
}

/// Runs two-phase generation that caps the `<think>` block.
///
/// **Phase 1** – normal streaming; if `MAX_THINK_TOKENS` thinking tokens are
/// consumed before `</think>` appears, generation stops early.
///
/// **Phase 2** (only if budget was hit) – a `</think>` marker is injected into
/// the output stream, then generation resumes through the GGUF's own chat template.
fn generate_with_think_budget(
    engine: &LlmEngine,
    system_prompt: &str,
    user_message: &str,
    max_tokens: usize,
    temperature: f32,
    emit: &dyn Fn(&str),
) -> Result<String, AppError> {
    let mut output = String::new();
    let mut think_tokens: usize = 0;
    let mut budget_hit = false;
    let mut in_think = false;
    // Rolling tail buffer to detect tags that may be split across token boundaries,
    // without rescanning the full output on every token (which would be O(n²)).
    let mut tag_tail = String::new();

    engine.generate_streaming(
        system_prompt,
        user_message,
        max_tokens,
        temperature,
        |token| {
            output.push_str(token);
            tag_tail.push_str(token);

            if !in_think && tag_tail.contains("<think>") {
                in_think = true;
            }
            if in_think && tag_tail.contains("</think>") {
                in_think = false;
            }
            // Keep tail short enough to span a split tag; "</think>" is 8 chars.
            if tag_tail.len() > 16 {
                let drain_end = tag_tail.len() - 16;
                let drain_end = utf8::find_boundary_backward(&tag_tail, drain_end);
                tag_tail.drain(..drain_end);
            }

            if in_think {
                think_tokens += 1;
                if think_tokens >= MAX_THINK_TOKENS {
                    budget_hit = true;
                    return false; // stop phase 1
                }
            }

            emit(token);
            true
        },
    )?;

    // Read phase-1 stats before any phase-2 call overwrites them.
    let phase1_stats = engine.last_generation_stats();

    if budget_hit {
        // Inject the closing tag so the frontend renders thinking as complete.
        let close_tag = "</think>\n\n";
        output.push_str(close_tag);
        emit(close_tag);

        // Continue through the model's own embedded chat template. Supplying a new
        // continuation turn works across ChatML, Harmony, Gemma, and Phi templates.
        let tail_start = output.len().saturating_sub(1_200);
        let tail_start = utf8::find_boundary_forward(&output, tail_start);
        let continuation = format!(
            "Setze die Antwort auf die folgende ursprüngliche Aufgabe unmittelbar fort. \
             Wiederhole nichts und gib nur den fertigen Inhalt aus.\n\nAufgabe:\n{user_message}\n\n\
             Bisheriges Ende:\n{}",
            &output[tail_start..]
        );

        engine.generate_streaming(
            system_prompt,
            &continuation,
            max_tokens.saturating_sub(MAX_THINK_TOKENS),
            temperature,
            |token| {
                output.push_str(token);
                emit(token);
                true
            },
        )?;
    } else if let Some(stats) = phase1_stats {
        // Detect context-window overflow when prompt + completion reach the
        // active per-engine context limit.
        let ctx_size = engine.context_size();
        let was_cut_off = stats.completion_tokens > 0
            && stats.prompt_tokens + stats.completion_tokens + 10 >= ctx_size;

        if was_cut_off {
            // Anchor the continuation with the tail of the partial output.
            let tail_start = output.len().saturating_sub(800);
            let tail_start = utf8::find_boundary_forward(&output, tail_start);
            let tail = &output[tail_start..];
            let continuation_msg = prompts::continuation_prompt(tail);

            engine.generate_streaming(
                system_prompt,
                &continuation_msg,
                max_tokens,
                temperature,
                |token| {
                    output.push_str(token);
                    emit(token);
                    true
                },
            )?;
        }
    }

    Ok(output)
}

/// Generate a report using the built-in system prompt.
pub fn generate_report_streaming(
    app: &tauri::AppHandle,
    engine: &LlmEngine,
    report_type: ReportType,
    patient_context: &str,
    session_notes: &str,
) -> Result<String, AppError> {
    generate_report_streaming_with_prompt(
        app,
        engine,
        report_type,
        patient_context,
        session_notes,
        None,
        None,
        prompts::SYSTEM_PROMPT_DE,
    )
}

/// Generate a report using a caller-supplied system prompt.
/// Emits `"report-chunk"` Tauri events for each token as it is produced.
/// If inputs are too long, emits `"report-summarizing"` then condenses them first.
/// Returns the full completed report string.
#[allow(clippy::too_many_arguments)]
pub fn generate_report_streaming_with_prompt(
    app: &tauri::AppHandle,
    engine: &LlmEngine,
    report_type: ReportType,
    patient_context: &str,
    session_notes: &str,
    additional_context: Option<&str>,
    instructions: Option<&str>,
    system_prompt: &str,
) -> Result<String, AppError> {
    let summary_opt = if needs_summarization(engine, system_prompt, patient_context, session_notes)?
    {
        let _ = app.emit("report-summarizing", ());
        Some(run_summarization(
            engine,
            system_prompt,
            patient_context,
            session_notes,
        )?)
    } else {
        None
    };
    let (eff_ctx, eff_notes) = match &summary_opt {
        Some(s) => (s.as_str(), ""),
        None => (patient_context, session_notes),
    };

    let user_message = prompts::report_generation_prompt(
        report_type,
        eff_ctx,
        eff_notes,
        additional_context,
        instructions,
    );

    generate_with_think_budget(engine, system_prompt, &user_message, 4096, 0.7, &|token| {
        let _ = app.emit("report-chunk", token);
    })
}

/// Improve text based on provided instruction using the built-in system prompt.
pub fn improve_text_streaming(
    app: &tauri::AppHandle,
    engine: &LlmEngine,
    text: &str,
    instruction: &str,
) -> Result<String, AppError> {
    improve_text_streaming_with_prompt(app, engine, text, instruction, prompts::SYSTEM_PROMPT_DE)
}

/// Improve text based on provided instruction using a caller-supplied system prompt.
/// Emits `"text-improvement-chunk"` Tauri events for each token as it is produced.
/// Returns the full improved text string.
pub fn improve_text_streaming_with_prompt(
    app: &tauri::AppHandle,
    engine: &LlmEngine,
    text: &str,
    instruction: &str,
    system_prompt: &str,
) -> Result<String, AppError> {
    let safe_text = sanitize_for_prompt(text);
    let safe_instruction = sanitize_for_prompt(instruction);
    let user_message = build_delimited_prompt(&safe_instruction, &safe_text);

    generate_with_think_budget(engine, system_prompt, &user_message, 4096, 0.7, &|token| {
        let _ = app.emit("text-improvement-chunk", token);
    })
}

/// Generate a session summary using the built-in system prompt.
pub fn generate_session_summary_streaming(
    app: &tauri::AppHandle,
    engine: &LlmEngine,
    patient_context: &str,
    session_notes: &str,
) -> Result<String, AppError> {
    generate_session_summary_streaming_with_prompt(
        app,
        engine,
        patient_context,
        session_notes,
        prompts::SYSTEM_PROMPT_DE,
    )
}

/// Generate a session summary using a caller-supplied system prompt.
/// Emits `"session-summary-chunk"` Tauri events for each token as it is produced.
/// If inputs are too long, emits `"session-summary-summarizing"` then condenses them first.
/// Returns the full completed session summary string.
pub fn generate_session_summary_streaming_with_prompt(
    app: &tauri::AppHandle,
    engine: &LlmEngine,
    patient_context: &str,
    session_notes: &str,
    system_prompt: &str,
) -> Result<String, AppError> {
    let summary_opt = if needs_summarization(engine, system_prompt, patient_context, session_notes)?
    {
        let _ = app.emit("session-summary-summarizing", ());
        Some(run_summarization(
            engine,
            system_prompt,
            patient_context,
            session_notes,
        )?)
    } else {
        None
    };
    let (eff_ctx, eff_notes) = match &summary_opt {
        Some(s) => (s.as_str(), ""),
        None => (patient_context, session_notes),
    };

    let user_message = prompts::session_summary_prompt(eff_ctx, eff_notes);

    generate_with_think_budget(engine, system_prompt, &user_message, 4096, 0.7, &|token| {
        let _ = app.emit("session-summary-chunk", token);
    })
}

/// Generate a letter using a caller-supplied system prompt.
/// Emits `"letter-chunk"` Tauri events for each token as it is produced.
/// If inputs are too long, emits `"letter-summarizing"` then condenses them first.
/// Returns the full completed letter string.
#[allow(clippy::too_many_arguments)]
pub fn generate_letter_streaming_with_prompt(
    app: &tauri::AppHandle,
    engine: &LlmEngine,
    letter_type: LetterType,
    language: &str,
    patient_context: &str,
    clinical_summary: &str,
    recipient_name: Option<&str>,
    system_prompt: &str,
) -> Result<String, AppError> {
    let summary_opt =
        if needs_summarization(engine, system_prompt, patient_context, clinical_summary)? {
            let _ = app.emit("letter-summarizing", ());
            Some(run_summarization(
                engine,
                system_prompt,
                patient_context,
                clinical_summary,
            )?)
        } else {
            None
        };
    let (eff_ctx, eff_summary) = match &summary_opt {
        Some(s) => (s.as_str(), ""),
        None => (patient_context, clinical_summary),
    };

    let user_message = prompts::letter_generation_prompt(
        letter_type,
        language,
        eff_ctx,
        eff_summary,
        recipient_name,
    );

    generate_with_think_budget(engine, system_prompt, &user_message, 4096, 0.7, &|token| {
        let _ = app.emit("letter-chunk", token);
    })
}

/// Answer a patient-history question from an assembled evidence block.
///
/// No summarisation pass is needed or wanted here: `llm::evidence` already fit
/// the evidence into the model's budget, and condensing it would be exactly the
/// lossy step the evidence layer exists to avoid. Emits
/// `"patient-history-chunk"` Tauri events for each token.
pub fn generate_evidence_answer_streaming(
    app: &tauri::AppHandle,
    engine: &LlmEngine,
    evidence: &str,
    question: &str,
    system_prompt: &str,
) -> Result<String, AppError> {
    let user_message = prompts::evidence_query_prompt(evidence, question);
    generate_with_think_budget(engine, system_prompt, &user_message, 4096, 0.3, &|token| {
        let _ = app.emit("patient-history-chunk", token);
    })
}
