<script lang="ts">
  interface Props {
    words: string[];
  }

  let { words }: Props = $props();
</script>

<div class="grid grid-cols-4 gap-3 max-w-2xl">
  {#each words as word, i}
    <div class="flex items-center gap-2 p-3 bg-surface-raised rounded-card border border-line">
      <span class="text-fg-muted text-body font-mono w-6">{i + 1}.</span>
      <span class="text-fg font-medium">{word}</span>
    </div>
  {/each}
</div>
