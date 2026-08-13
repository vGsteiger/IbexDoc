<script lang="ts">
  import { t } from '$lib/translations';
  import { onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import {
    queryPatientHistory,
    parseError,
    formatError,
    type AnswerAudit,
    type EvidenceManifest,
  } from '$lib/api';
  import { Loader2, Send, ChevronDown, ChevronUp, AlertTriangle } from 'lucide-svelte';

  interface Props {
    patientId: string;
  }

  let { patientId }: Props = $props();

  let question = $state('');
  let response = $state('');
  let isQuerying = $state(false);
  let error = $state('');
  let isExpanded = $state(true);
  let manifest = $state<EvidenceManifest | null>(null);
  let audit = $state<AnswerAudit | null>(null);
  let showEvidence = $state(false);

  let unlistenChunk: UnlistenFn | null = null;
  let unlistenDone: UnlistenFn | null = null;

  /// Entries the answer actually cited, in citation order.
  let citedEntries = $derived(
    manifest && audit
      ? audit.citations
          .filter((check) => check.in_manifest)
          .map((check) => ({
            check,
            entry: manifest?.entries.find((entry) => entry.citation === check.citation),
          }))
          .filter((pair) => pair.entry !== undefined)
      : []
  );

  let citationWarnings = $derived(
    audit ? [...audit.unsupported_citations, ...audit.stale_citations] : []
  );

  // Suggested queries
  let suggestedQueries = $derived(
    ['q1', 'q2', 'q3', 'q4', 'q5'].map((q) => $t(`patients.historySuggestions.${q}`))
  );

  async function handleQuery() {
    if (!question.trim() || isQuerying) return;

    try {
      isQuerying = true;
      error = '';
      response = '';
      manifest = null;
      audit = null;

      // Setup event listeners before invoking
      if (unlistenChunk) {
        unlistenChunk();
        unlistenChunk = null;
      }
      if (unlistenDone) {
        unlistenDone();
        unlistenDone = null;
      }

      unlistenChunk = await listen<string>('patient-history-chunk', (event) => {
        response += event.payload;
      });

      unlistenDone = await listen('patient-history-done', () => {
        isQuerying = false;
        // Clean up listeners
        if (unlistenChunk) {
          unlistenChunk();
          unlistenChunk = null;
        }
        if (unlistenDone) {
          unlistenDone();
          unlistenDone = null;
        }
      });

      // Invoke the command; the result carries the evidence behind the answer.
      const result = await queryPatientHistory(patientId, question);
      response = result.answer;
      manifest = result.manifest;
      audit = result.audit;
    } catch (e) {
      const appError = parseError(e);
      error = formatError(appError);
      console.error('Error querying patient history:', appError);
      isQuerying = false;
      // Clean up listeners on error
      if (unlistenChunk) {
        unlistenChunk();
        unlistenChunk = null;
      }
      if (unlistenDone) {
        unlistenDone();
        unlistenDone = null;
      }
    }
  }

  function handleSuggestedQuery(suggestedQuestion: string) {
    question = suggestedQuestion;
    handleQuery();
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault();
      handleQuery();
    }
  }

  onDestroy(() => {
    if (unlistenChunk) {
      unlistenChunk();
    }
    if (unlistenDone) {
      unlistenDone();
    }
  });
</script>

<div class="bg-surface-raised rounded-card border border-line">
  <button
    onclick={() => (isExpanded = !isExpanded)}
    class="flex items-center justify-between w-full p-4 hover:bg-surface-hover transition-colors rounded-t-card"
  >
    <h3 class="text-heading font-semibold text-fg">Ask about this patient</h3>
    {#if isExpanded}
      <ChevronUp class="w-5 h-5 text-fg-muted" />
    {:else}
      <ChevronDown class="w-5 h-5 text-fg-muted" />
    {/if}
  </button>

  {#if isExpanded}
    <div class="p-4 border-t border-line space-y-4">
      <!-- Suggested queries -->
      <div class="space-y-2">
        <p class="text-body text-fg-muted">Suggested queries:</p>
        <div class="flex flex-wrap gap-2">
          {#each suggestedQueries as suggested (suggested)}
            <button
              onclick={() => handleSuggestedQuery(suggested)}
              disabled={isQuerying}
              class="h-7 px-2.5 text-body bg-surface-hover text-fg-muted rounded-full hover:bg-surface-selected transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {suggested}
            </button>
          {/each}
        </div>
      </div>

      <!-- Query input -->
      <div class="flex gap-2">
        <textarea
          bind:value={question}
          onkeydown={handleKeydown}
          disabled={isQuerying}
          placeholder="Ask a question about this patient's history..."
          class="flex-1 px-4 py-2 border border-line rounded-control focus:ring-2 focus:ring-accent/30 focus:border-transparent bg-surface-raised text-fg disabled:opacity-50 disabled:cursor-not-allowed resize-none"
          rows="2"></textarea>
        <button
          onclick={handleQuery}
          disabled={isQuerying || !question.trim()}
          class="h-8 px-3 bg-accent text-on-accent rounded-control hover:bg-accent-hover transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2 self-start"
        >
          {#if isQuerying}
            <Loader2 class="w-4 h-4 animate-spin" />
            <span>Querying...</span>
          {:else}
            <Send class="w-4 h-4" />
            <span>Ask</span>
          {/if}
        </button>
      </div>

      <!-- Error display -->
      {#if error}
        <div class="bg-danger-subtle border border-danger-line rounded-card p-4 text-danger-fg">
          {error}
        </div>
      {/if}

      <!-- Response display -->
      {#if response || isQuerying}
        <div class="bg-surface-sunken rounded-card p-4 border border-line">
          <div class="flex items-center justify-between mb-2">
            <h4 class="text-body font-semibold text-fg-muted">Response:</h4>
            {#if isQuerying}
              <div class="flex items-center gap-2 text-body text-fg-muted">
                <Loader2 class="w-4 h-4 animate-spin" />
                <span>Generating...</span>
              </div>
            {/if}
          </div>
          <div class="prose prose-sm dark:prose-invert max-w-none">
            {#if response}
              <div class="whitespace-pre-wrap text-fg">
                {response}
              </div>
            {:else}
              <div class="text-fg-muted italic">Waiting for response...</div>
            {/if}
          </div>
        </div>
      {/if}

      <!-- Evidence behind the answer -->
      {#if manifest && !isQuerying}
        <div class="rounded-card border border-line p-4 space-y-3">
          <button
            onclick={() => (showEvidence = !showEvidence)}
            class="flex items-center justify-between w-full text-left"
          >
            <span class="text-body font-semibold text-fg-muted">
              Evidence ({manifest.entries.length} excerpts, {manifest.prompt_tokens} of {manifest.token_budget}
              tokens)
            </span>
            {#if showEvidence}
              <ChevronUp class="w-4 h-4 text-fg-muted" />
            {:else}
              <ChevronDown class="w-4 h-4 text-fg-muted" />
            {/if}
          </button>

          {#if citationWarnings.length > 0}
            <div
              class="flex items-start gap-2 text-body text-warning-fg bg-warning-subtle rounded-card p-2"
            >
              <AlertTriangle class="w-4 h-4 mt-0.5 shrink-0" />
              <span>
                Citations without a current source: {citationWarnings.join(', ')}. Re-run the query
                after record changes.
              </span>
            </div>
          {/if}

          {#if showEvidence}
            <ul class="space-y-2 text-body">
              {#each citedEntries as { check, entry } (check.citation)}
                <li class="text-fg-muted">
                  <span class="font-mono text-caption">[{check.citation}]</span>
                  {entry?.label}
                  <span class="text-fg-muted">
                    — {entry?.occurred_at}, characters {entry?.char_start}–{entry?.char_end},
                    revision {entry?.revision}
                  </span>
                  {#if !check.traceable}
                    <span class="text-warning-fg"> (source changed)</span>
                  {/if}
                  <div class="text-caption text-fg-muted">
                    Selected because: {entry?.selection_reasons?.join('; ') ?? '—'}
                  </div>
                </li>
              {/each}
              {#if citedEntries.length === 0}
                <li class="text-fg-muted italic">The answer cited no excerpts.</li>
              {/if}
            </ul>
            {#if manifest.omitted.length > 0}
              <p class="text-caption text-fg-muted">
                {manifest.omitted.length} further excerpt(s) were not included (budget or archive pointers).
              </p>
            {/if}
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</div>
