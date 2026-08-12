<script lang="ts">
  import type { AppError } from '$lib/api';
  import { getUserFriendlyMessage } from '$lib/api';

  interface Props {
    error: AppError | null;
    showDetails?: boolean;
  }

  let { error, showDetails = false }: Props = $props();

  let expanded = $state(false);

  function copyErrorRef() {
    if (error?.ref) {
      navigator.clipboard.writeText(error.ref);
    }
  }

  function copyFullError() {
    if (error) {
      const text = `Error Code: ${error.code}\nError Reference: ${error.ref}\nMessage: ${error.message}`;
      navigator.clipboard.writeText(text);
    }
  }
</script>

{#if error}
  <div class="bg-danger-subtle border border-danger-line rounded-card p-4">
    <div class="flex items-start justify-between mb-2">
      <div class="flex-1">
        <p class="text-danger-fg text-body font-medium mb-1">
          {getUserFriendlyMessage(error)}
        </p>
        <div class="flex items-center gap-2 text-caption text-fg-muted">
          <span class="font-mono">{error.ref}</span>
          <button
            onclick={copyErrorRef}
            class="text-accent-fg hover:text-accent-fg underline"
            title="Copy error reference"
          >
            Copy
          </button>
        </div>
      </div>
      {#if showDetails}
        <button
          onclick={() => (expanded = !expanded)}
          class="text-fg-muted hover:text-fg text-caption ml-4"
        >
          {expanded ? 'Hide Details' : 'Show Details'}
        </button>
      {/if}
    </div>

    {#if showDetails && expanded}
      <div class="mt-3 pt-3 border-t border-danger-line">
        <div class="space-y-2 text-caption">
          <div>
            <span class="text-fg-muted">Error Code:</span>
            <span class="ml-2 font-mono text-fg-muted">{error.code}</span>
          </div>
          <div>
            <span class="text-fg-muted">Technical Message:</span>
            <span class="ml-2 text-fg-muted">{error.message}</span>
          </div>
          <div>
            <span class="text-fg-muted">Reference ID:</span>
            <span class="ml-2 font-mono text-fg-muted">{error.ref}</span>
          </div>
        </div>
        <button
          onclick={copyFullError}
          class="mt-3 text-caption text-accent-fg hover:text-accent-fg underline"
        >
          Copy Full Error Details
        </button>
      </div>
    {/if}

    <p class="text-caption text-fg-muted mt-2">
      Share the error reference with support if you need help resolving this issue.
    </p>
  </div>
{/if}
