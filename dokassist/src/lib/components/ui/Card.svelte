<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    padding = 'md',
    interactive = false,
    href = undefined,
    onclick = undefined,
    class: className = '',
    children,
    ...rest
  }: {
    padding?: 'none' | 'sm' | 'md';
    interactive?: boolean;
    href?: string;
    onclick?: (event: MouseEvent) => void;
    class?: string;
    children?: Snippet;
    [key: string]: unknown;
  } = $props();

  const paddings = { none: '', sm: 'p-3', md: 'p-4' };

  // A card that navigates or acts is interactive whether or not the caller
  // says so, so the hover affordance is never accidentally omitted.
  let isInteractive = $derived(interactive || Boolean(href) || Boolean(onclick));

  /* A hairline border is the separation mechanism — no shadow on inline
   * containers, only on genuine overlays (see Dialog). */
  let classes = $derived(
    [
      'rounded-card border border-line bg-surface-raised',
      paddings[padding],
      isInteractive
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
{:else if onclick}
  <button type="button" {onclick} class="{classes} w-full text-left" {...rest}>
    {@render children?.()}
  </button>
{:else}
  <div class={classes} {...rest}>{@render children?.()}</div>
{/if}
