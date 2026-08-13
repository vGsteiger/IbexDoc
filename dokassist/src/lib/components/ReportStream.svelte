<script lang="ts">
  import { t } from '$lib/translations';
  export let content: string = '';
  export let isStreaming: boolean = false;
  export let isSummarizing: boolean = false;

  const THINK_START = '<think>';
  const THINK_END = '</think>';

  let thinkContent = '';
  let reportContent = '';

  $: {
    if (content.startsWith(THINK_START)) {
      const endIdx = content.indexOf(THINK_END);
      if (endIdx !== -1) {
        thinkContent = content.slice(THINK_START.length, endIdx).trim();
        reportContent = content.slice(endIdx + THINK_END.length).trim();
      } else {
        // Still streaming inside the think block
        thinkContent = content.slice(THINK_START.length).trim();
        reportContent = '';
      }
    } else {
      thinkContent = '';
      reportContent = content;
    }
  }
</script>

<div class="space-y-4">
  {#if thinkContent}
    <div class="bg-surface-hover border border-line rounded-card p-4">
      <p class="text-caption font-medium text-fg-subtle uppercase tracking-wide mb-2">Thinking</p>
      <pre
        class="not-prose whitespace-pre-wrap font-sans text-body text-fg-muted italic">{thinkContent}</pre>
      {#if isStreaming && !reportContent}
        <div class="flex items-center space-x-2 text-fg-subtle mt-2">
          <div class="animate-pulse text-caption">●</div>
          <span class="text-caption">Thinking...</span>
        </div>
      {/if}
    </div>
  {/if}

  <div class="bg-surface-raised border border-line rounded-card p-6 min-h-[300px] relative">
    {#if isStreaming && reportContent}
      <div class="absolute top-4 right-4">
        <div class="flex items-center space-x-2 text-body text-accent-fg">
          <div class="animate-pulse">●</div>
          <span>Generating...</span>
        </div>
      </div>
    {/if}

    <div class="prose dark:prose-invert max-w-none">
      {#if reportContent}
        <pre class="not-prose whitespace-pre-wrap font-sans text-fg">{reportContent}</pre>
      {:else if !isStreaming && !thinkContent}
        <p class="text-fg-subtle italic">Report will appear here as it's generated...</p>
      {:else if isStreaming && isSummarizing && !thinkContent && !reportContent}
        <div class="flex items-center space-x-2 text-fg-subtle">
          <div class="animate-spin rounded-full h-4 w-4 border-b-2 border-warning"></div>
          <span>{$t('reports.compressingContext')}</span>
        </div>
      {:else if isStreaming && !thinkContent && !reportContent}
        <div class="flex items-center space-x-2 text-fg-subtle">
          <div class="animate-spin rounded-full h-4 w-4 border-b-2 border-accent"></div>
          <span>Waiting for LLM...</span>
        </div>
      {:else if !isStreaming && thinkContent && !reportContent}
        <p class="text-fg-subtle italic">No report content was generated.</p>
      {/if}
    </div>
  </div>
</div>
