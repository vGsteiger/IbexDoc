<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    padding = 'md',
    interactive = false,
    href = undefined,
    class: className = '',
    children,
    ...rest
  }: {
    padding?: 'none' | 'sm' | 'md';
    interactive?: boolean;
    href?: string;
    class?: string;
    children?: Snippet;
    [key: string]: unknown;
  } = $props();

  const paddings = { none: '', sm: 'p-3', md: 'p-4' };

  /* A hairline border is the separation mechanism — no shadow on inline
   * containers, only on genuine overlays (see Dialog). */
  let classes = $derived(
    [
      'rounded-card border border-line bg-surface-raised',
      paddings[padding],
      interactive
        ? 'block transition-colors duration-150 ease-standard hover:border-line-strong hover:bg-surface-hover'
        : '',
      className,
    ]
      .filter(Boolean)
      .join(' ')
  );
</script>

{#if href}
  <a {href} class={classes} {...rest}>{@render children?.()}</a>
{:else}
  <div class={classes} {...rest}>{@render children?.()}</div>
{/if}
