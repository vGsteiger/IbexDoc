<script lang="ts">
  import type { AMDPItem } from '$lib/amdp';

  interface Props {
    item: AMDPItem;
    onScoreChange: (code: string, score: 0 | 1 | 2 | 3) => void;
  }

  let { item, onScoreChange }: Props = $props();

  const scores = [
    { value: 0, label: '0', title: 'Nicht vorhanden' },
    { value: 1, label: '1', title: 'Leicht' },
    { value: 2, label: '2', title: 'Mittel' },
    { value: 3, label: '3', title: 'Schwer' },
  ] as const;

  function handleScoreClick(score: 0 | 1 | 2 | 3) {
    onScoreChange(item.code, score);
  }
</script>

<div class="flex items-center justify-between py-2 px-3 hover:bg-surface-selected/30 rounded-card">
  <div class="flex-1">
    <span class="text-body text-fg-muted">
      <span class="text-fg-muted font-mono text-caption mr-2">{item.code}</span>
      {item.label}
    </span>
  </div>
  <div class="flex gap-1 ml-4">
    {#each scores as { value, label, title }}
      <button
        type="button"
        class="w-10 h-10 rounded-control transition-colors font-medium text-body"
        class:bg-surface-hover={item.score !== value}
        class:text-fg-muted={item.score !== value}
        class:bg-accent={item.score === value}
        class:text-on-accent={item.score === value}
        onclick={() => handleScoreClick(value)}
        {title}
      >
        {label}
      </button>
    {/each}
  </div>
</div>
