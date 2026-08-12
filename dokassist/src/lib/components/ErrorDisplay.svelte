<script lang="ts">
  import type { AppError } from '$lib/api';
  import { getUserFriendlyMessage } from '$lib/api';
  import { Alert } from '$lib/components/ui';

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
  <Alert tone="danger" title={getUserFriendlyMessage(error)}>
    <div class="flex items-start justify-between gap-4">
      <div class="flex items-center gap-2 text-caption">
        <span class="font-mono">{error.ref}</span>
        <button onclick={copyErrorRef} class="underline hover:text-fg" title="Copy error reference">
          Copy
        </button>
      </div>
      {#if showDetails}
        <button onclick={() => (expanded = !expanded)} class="shrink-0 text-caption hover:text-fg">
          {expanded ? 'Hide Details' : 'Show Details'}
        </button>
      {/if}
    </div>

    {#if showDetails && expanded}
      <div class="mt-3 border-t border-danger-line pt-3">
        <dl class="space-y-1 text-caption">
          <div class="flex gap-2">
            <dt>Error Code:</dt>
            <dd class="font-mono">{error.code}</dd>
          </div>
          <div class="flex gap-2">
            <dt>Technical Message:</dt>
            <dd>{error.message}</dd>
          </div>
          <div class="flex gap-2">
            <dt>Reference ID:</dt>
            <dd class="font-mono">{error.ref}</dd>
          </div>
        </dl>
        <button onclick={copyFullError} class="mt-3 text-caption underline hover:text-fg">
          Copy Full Error Details
        </button>
      </div>
    {/if}

    <p class="mt-2 text-caption">
      Share the error reference with support if you need help resolving this issue.
    </p>
  </Alert>
{/if}
