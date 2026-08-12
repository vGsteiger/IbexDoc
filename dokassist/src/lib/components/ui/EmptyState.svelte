<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { ComponentType } from 'svelte';

  let {
    icon = undefined,
    title,
    description = undefined,
    class: className = '',
    action,
  }: {
    icon?: ComponentType;
    title: string;
    description?: string;
    class?: string;
    action?: Snippet;
  } = $props();
</script>

<div
  class="flex flex-col items-center justify-center rounded-card border border-dashed border-line px-6 py-12 text-center {className}"
>
  {#if icon}
    {@const Icon = icon}
    <Icon size={20} class="mb-3 text-fg-subtle" aria-hidden="true" />
  {/if}
  <p class="text-heading text-fg">{title}</p>
  {#if description}
    <p class="mt-1 max-w-sm text-body text-fg-muted">{description}</p>
  {/if}
  {#if action}
    <div class="mt-4">{@render action()}</div>
  {/if}
</div>
