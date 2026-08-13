<script lang="ts">
  import { t } from '$lib/translations';
  export let content: string = '';
  export let readonly: boolean = false;

  let showPreview = false;
</script>

<div class="flex flex-col h-full border border-line rounded-card overflow-hidden">
  <div class="flex border-b border-line bg-surface-hover">
    <button
      on:click={() => (showPreview = false)}
      class="flex-1 h-8 px-3 text-body font-medium transition-colors {!showPreview
        ? 'bg-surface-raised text-fg'
        : 'text-fg-muted hover:text-fg'}"
    >
      {$t('reports.edit')}
    </button>
    <button
      on:click={() => (showPreview = true)}
      class="flex-1 h-8 px-3 text-body font-medium transition-colors {showPreview
        ? 'bg-surface-raised text-fg'
        : 'text-fg-muted hover:text-fg'}"
    >
      {$t('reports.preview')}
    </button>
  </div>

  <div class="flex-1 overflow-auto">
    {#if showPreview}
      <div class="p-6 prose dark:prose-invert max-w-none">
        {#if content}
          <pre class="whitespace-pre-wrap font-sans text-fg">{content}</pre>
        {:else}
          <p class="text-fg-subtle italic">{$t('reports.noPreviewContent')}</p>
        {/if}
      </div>
    {:else}
      <textarea
        bind:value={content}
        {readonly}
        class="w-full h-full p-6 bg-surface-raised text-fg font-mono text-body resize-none focus:outline-none"
        placeholder={$t('reports.contentPlaceholder')}></textarea>
    {/if}
  </div>
</div>
