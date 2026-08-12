<script lang="ts">
  import { AlertCircle, CheckCircle2, X } from 'lucide-svelte';
  import { toasts, removeToast } from '$lib/stores/toast';
</script>

<div
  class="fixed top-4 right-4 z-50 flex flex-col gap-2 pointer-events-none"
  aria-live="polite"
  aria-atomic="false"
>
  {#each $toasts as toast (toast.id)}
    <!-- A bordered overlay surface with one tinted status icon, rather than a
         saturated block of colour shouting from the corner of the screen. -->
    <div
      role="status"
      class="pointer-events-auto flex min-w-64 max-w-sm items-start gap-2.5 rounded-card border border-line bg-surface-overlay p-3 text-body text-fg shadow-popover"
    >
      {#if toast.type === 'success'}
        <CheckCircle2 size={16} class="mt-0.5 shrink-0 text-success-fg" aria-hidden="true" />
      {:else}
        <AlertCircle size={16} class="mt-0.5 shrink-0 text-danger-fg" aria-hidden="true" />
      {/if}
      <span class="flex-1">{toast.message}</span>
      <button
        onclick={() => removeToast(toast.id)}
        class="-mr-0.5 -mt-0.5 shrink-0 rounded-control p-0.5 text-fg-subtle transition-colors duration-150 ease-standard hover:bg-surface-hover hover:text-fg"
        aria-label="Dismiss notification"
      >
        <X size={14} />
      </button>
    </div>
  {/each}
</div>
