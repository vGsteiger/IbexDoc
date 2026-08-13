<script lang="ts">
  import type { Snippet } from 'svelte';

  let { class: className = '', children }: { class?: string; children?: Snippet } = $props();
</script>

<kbd
  class="inline-flex h-5 min-w-5 items-center justify-center rounded-control border border-line bg-surface-hover px-1 font-sans text-caption font-medium text-fg-subtle {className}"
>
  {@render children?.()}
</kbd>
