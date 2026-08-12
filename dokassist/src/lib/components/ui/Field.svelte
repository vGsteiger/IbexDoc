<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    label,
    for: forId = undefined,
    hint = undefined,
    error = undefined,
    required = false,
    class: className = '',
    children,
  }: {
    label?: string;
    for?: string;
    hint?: string;
    error?: string;
    required?: boolean;
    class?: string;
    children?: Snippet;
  } = $props();
</script>

<div class={className}>
  {#if label}
    <label class="field-label" for={forId}>
      {label}{#if required}<span class="text-danger-fg" aria-hidden="true">&nbsp;*</span>{/if}
    </label>
  {/if}
  {@render children?.()}
  {#if error}
    <p class="mt-1 text-caption text-danger-fg">{error}</p>
  {:else if hint}
    <p class="mt-1 text-caption text-fg-subtle">{hint}</p>
  {/if}
</div>
