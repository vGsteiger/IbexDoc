use serde::{Deserialize, Serialize};

/// Bump whenever the agent system/tool prompt semantics change.
pub const AGENT_PROMPT_VERSION: &str = "agent-v1";

/// The caller-owned portion of an inference-context identity.
///
/// Patient identity and revision are deliberately separate fields: a context
/// can never be reused for another patient, and updating a patient invalidates
/// every context built from the previous record revision.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct InferenceSession {
    pub conversation_id: String,
    pub patient_id: Option<String>,
    pub patient_revision: Option<String>,
    pub prompt_version: String,
    pub adapter_hash: String,
}

impl InferenceSession {
    pub fn agent(
        conversation_id: impl Into<String>,
        patient_id: Option<String>,
        patient_revision: Option<String>,
    ) -> Self {
        Self {
            conversation_id: conversation_id.into(),
            patient_id,
            patient_revision,
            prompt_version: AGENT_PROMPT_VERSION.to_string(),
            adapter_hash: "none".to_string(),
        }
    }

    pub fn isolated(id: impl Into<String>) -> Self {
        Self {
            conversation_id: id.into(),
            patient_id: None,
            patient_revision: None,
            prompt_version: "raw-v1".to_string(),
            adapter_hash: "none".to_string(),
        }
    }
}

/// Complete identity for a KV context. Every input that can alter tokenization
/// or KV layout is represented, in addition to patient/conversation isolation.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub(crate) struct ContextKey {
    pub model_hash: String,
    pub chat_template_hash: String,
    pub system_prompt_hash: String,
    pub prompt_version: String,
    pub adapter_hash: String,
    pub context_size: usize,
    pub batch_size: u32,
    pub kv_config_hash: String,
    pub conversation_id: String,
    pub patient_id: Option<String>,
    pub patient_revision: Option<String>,
}

impl ContextKey {
    /// Whether two keys belong to the same logical conversation. Used to
    /// eagerly invalidate stale revisions/configurations instead of waiting
    /// for LRU eviction.
    pub fn same_logical_context(&self, other: &Self) -> bool {
        self.conversation_id == other.conversation_id && self.patient_id == other.patient_id
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextCacheTelemetry {
    pub hits: u64,
    pub misses: u64,
    pub invalidations: u64,
    pub evictions: u64,
    pub reused_tokens: u64,
    pub evaluated_tokens: u64,
    pub estimated_prefill_saved_ms: f64,
    pub resident_contexts: usize,
    pub max_contexts: usize,
}

pub(crate) fn reusable_prefix<T: PartialEq>(cached: &[T], requested: &[T]) -> usize {
    cached
        .iter()
        .zip(requested)
        .take_while(|(left, right)| left == right)
        .count()
        // Sampling reads the logits from the last decoded token. Re-evaluate
        // that token so rollback never samples stale logits from a completion.
        .saturating_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(patient: &str, revision: &str, model: &str) -> ContextKey {
        ContextKey {
            model_hash: model.into(),
            chat_template_hash: "template".into(),
            system_prompt_hash: "prompt".into(),
            prompt_version: AGENT_PROMPT_VERSION.into(),
            adapter_hash: "none".into(),
            context_size: 16_384,
            batch_size: 2_048,
            kv_config_hash: "f16:512:enabled".into(),
            conversation_id: "session-1".into(),
            patient_id: Some(patient.into()),
            patient_revision: Some(revision.into()),
        }
    }

    #[test]
    fn cache_keys_isolate_patient_revision_model_and_template_inputs() {
        let base = key("patient-a", "revision-1", "model-a");
        assert_ne!(base, key("patient-b", "revision-1", "model-a"));
        assert_ne!(base, key("patient-a", "revision-2", "model-a"));
        assert_ne!(base, key("patient-a", "revision-1", "model-b"));

        let mut template_changed = base.clone();
        template_changed.chat_template_hash = "other-template".into();
        assert_ne!(base, template_changed);

        let mut prompt_changed = base.clone();
        prompt_changed.prompt_version = "agent-v2".into();
        assert_ne!(base, prompt_changed);

        let mut adapter_changed = base.clone();
        adapter_changed.adapter_hash = "adapter-a".into();
        assert_ne!(base, adapter_changed);

        let mut kv_changed = base.clone();
        kv_changed.kv_config_hash = "q8:512:enabled".into();
        assert_ne!(base, kv_changed);
    }

    #[test]
    fn revision_change_is_the_same_logical_context_but_a_different_key() {
        let old = key("patient-a", "revision-1", "model-a");
        let new = key("patient-a", "revision-2", "model-a");
        assert!(old.same_logical_context(&new));
        assert_ne!(old, new);
    }

    #[test]
    fn prefix_reuse_refreshes_last_token_logits() {
        assert_eq!(reusable_prefix(&[1, 2, 3, 8], &[1, 2, 3, 9]), 2);
        assert_eq!(reusable_prefix(&[1, 2, 3], &[1, 2, 3]), 2);
        assert_eq!(reusable_prefix(&[1], &[1]), 0);
        assert_eq!(reusable_prefix(&[1, 2], &[9, 2]), 0);
    }
}
