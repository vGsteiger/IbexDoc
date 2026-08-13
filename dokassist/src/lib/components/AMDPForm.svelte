<script lang="ts">
  import { t } from '$lib/translations';
  import type { AMDPCategory } from '$lib/amdp';
  import AMDPCategoryComponent from './AMDPCategory.svelte';

  interface Props {
    categories: AMDPCategory[];
    onScoreChange: (code: string, score: 0 | 1 | 2 | 3) => void;
  }

  let { categories, onScoreChange }: Props = $props();

  let activeCategoryIndex = $state(0);
</script>

<div class="space-y-4">
  <!-- Category tabs -->
  <div class="flex gap-2 flex-wrap">
    {#each categories as category, index}
      <button
        type="button"
        class="px-4 py-2 rounded-control text-body font-medium transition-colors"
        class:bg-accent={activeCategoryIndex === index}
        class:text-on-accent={activeCategoryIndex === index}
        class:bg-surface-hover={activeCategoryIndex !== index}
        class:text-fg-muted={activeCategoryIndex !== index}
        onclick={() => (activeCategoryIndex = index)}
      >
        {category.name}
      </button>
    {/each}
  </div>

  <!-- Active category content -->
  {#if categories[activeCategoryIndex]}
    <AMDPCategoryComponent category={categories[activeCategoryIndex]} {onScoreChange} />
  {/if}

  <!-- Navigation buttons -->
  <div class="flex justify-between pt-4">
    <button
      type="button"
      class="h-8 px-3 bg-surface-selected text-fg-muted rounded-control hover:bg-surface-selected disabled:opacity-50 disabled:cursor-not-allowed"
      disabled={activeCategoryIndex === 0}
      onclick={() => (activeCategoryIndex = Math.max(0, activeCategoryIndex - 1))}
    >
      ← {$t('common.back')}
    </button>
    <button
      type="button"
      class="h-8 px-3 bg-surface-selected text-fg-muted rounded-control hover:bg-surface-selected disabled:opacity-50 disabled:cursor-not-allowed"
      disabled={activeCategoryIndex === categories.length - 1}
      onclick={() =>
        (activeCategoryIndex = Math.min(categories.length - 1, activeCategoryIndex + 1))}
    >
      {$t('common.next')} →
    </button>
  </div>
</div>
