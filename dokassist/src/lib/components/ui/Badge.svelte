<script lang="ts">
  import type { Snippet } from 'svelte';

  type Tone = 'neutral' | 'accent' | 'success' | 'warning' | 'danger' | 'info';

  let {
    tone = 'neutral',
    class: className = '',
    children,
    ...rest
  }: {
    tone?: Tone;
    class?: string;
    children?: Snippet;
    [key: string]: unknown;
  } = $props();

  const tones: Record<Tone, string> = {
    neutral: 'bg-surface-hover text-fg-muted border-line',
    accent: 'bg-accent-subtle text-accent-fg border-accent-line',
    info: 'bg-info-subtle text-info-fg border-info-line',
    success: 'bg-success-subtle text-success-fg border-success-line',
    warning: 'bg-warning-subtle text-warning-fg border-warning-line',
    danger: 'bg-danger-subtle text-danger-fg border-danger-line',
  };

  let classes = $derived(
    [
      'inline-flex items-center gap-1 rounded-control border px-1.5 py-0.5',
      'text-caption font-medium leading-none',
      tones[tone],
      className,
    ].join(' ')
  );
</script>

<span class={classes} {...rest}>{@render children?.()}</span>
