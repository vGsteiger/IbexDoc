<script lang="ts">
  import { t } from '$lib/translations';
  import { LoaderCircle } from 'lucide-svelte';

  let {
    size = 16,
    label = undefined,
    class: className = '',
  }: { size?: number; label?: string; class?: string } = $props();
</script>

{#if label}
  <span class="inline-flex items-center gap-2 text-body text-fg-muted {className}" role="status">
    <LoaderCircle {size} class="animate-spin" aria-hidden="true" />
    {label}
  </span>
{:else}
  <LoaderCircle
    {size}
    class="animate-spin text-fg-subtle {className}"
    role="status"
    aria-label={$t('common.loading')}
  />
{/if}
