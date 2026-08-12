<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    title,
    description = undefined,
    class: className = '',
    actions,
    breadcrumb,
  }: {
    title: string;
    description?: string;
    class?: string;
    actions?: Snippet;
    breadcrumb?: Snippet;
  } = $props();
</script>

<!-- One display-size heading per page. Sub-sections use text-title. -->
<header class="mb-6 {className}">
  {#if breadcrumb}
    <div class="mb-1.5 flex items-center gap-1 text-caption text-fg-subtle">
      {@render breadcrumb()}
    </div>
  {/if}
  <div class="flex flex-wrap items-center justify-between gap-3">
    <div class="min-w-0">
      <h1 class="text-display text-fg">{title}</h1>
      {#if description}
        <p class="mt-1 text-body text-fg-muted">{description}</p>
      {/if}
    </div>
    {#if actions}
      <div class="flex shrink-0 items-center gap-2">{@render actions()}</div>
    {/if}
  </div>
</header>
