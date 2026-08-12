<script lang="ts">
  import type { Snippet } from 'svelte';

  type Tone = 'default' | 'danger';

  let {
    label,
    tone = 'default',
    href = undefined,
    disabled = false,
    class: className = '',
    children,
    ...rest
  }: {
    label: string;
    tone?: Tone;
    href?: string;
    disabled?: boolean;
    class?: string;
    children?: Snippet;
    [key: string]: unknown;
  } = $props();

  const tones: Record<Tone, string> = {
    default: 'text-fg-subtle hover:bg-surface-hover hover:text-fg',
    danger: 'text-fg-subtle hover:bg-danger-subtle hover:text-danger-fg',
  };

  let classes = $derived(
    [
      'inline-flex h-7 w-7 items-center justify-center rounded-control',
      'transition-colors duration-150 ease-standard',
      'disabled:pointer-events-none disabled:opacity-50',
      tones[tone],
      className,
    ].join(' ')
  );
</script>

{#if href}
  <a {href} class={classes} aria-label={label} title={label} {...rest}>{@render children?.()}</a>
{:else}
  <button type="button" class={classes} aria-label={label} title={label} {disabled} {...rest}>
    {@render children?.()}
  </button>
{/if}
