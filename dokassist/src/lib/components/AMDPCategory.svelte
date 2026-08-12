<script lang="ts">
  import type { AMDPCategory } from '$lib/amdp';
  import AMDPItem from './AMDPItem.svelte';

  interface Props {
    category: AMDPCategory;
    onScoreChange: (code: string, score: 0 | 1 | 2 | 3) => void;
  }

  let { category, onScoreChange }: Props = $props();
</script>

<div class="bg-surface-raised rounded-card p-4 border border-line">
  <h3 class="text-heading font-semibold text-fg mb-3">{category.name}</h3>
  <div class="space-y-1">
    {#each category.items as item (item.code)}
      <AMDPItem {item} {onScoreChange} />
    {/each}
  </div>
</div>
