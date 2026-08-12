<script lang="ts">
  import type { Snippet } from 'svelte';
  import { AlertCircle, AlertTriangle, CheckCircle2, Info } from 'lucide-svelte';

  type Tone = 'info' | 'success' | 'warning' | 'danger';

  let {
    tone = 'info',
    title = undefined,
    icon = true,
    class: className = '',
    children,
    ...rest
  }: {
    tone?: Tone;
    title?: string;
    icon?: boolean;
    class?: string;
    children?: Snippet;
    [key: string]: unknown;
  } = $props();

  const tones: Record<Tone, string> = {
    info: 'bg-info-subtle border-info-line text-info-fg',
    success: 'bg-success-subtle border-success-line text-success-fg',
    warning: 'bg-warning-subtle border-warning-line text-warning-fg',
    danger: 'bg-danger-subtle border-danger-line text-danger-fg',
  };

  const icons = {
    info: Info,
    success: CheckCircle2,
    warning: AlertTriangle,
    danger: AlertCircle,
  };

  let Icon = $derived(icons[tone]);
</script>

<div
  class="flex gap-2.5 rounded-card border p-3 text-body {tones[tone]} {className}"
  role={tone === 'danger' ? 'alert' : 'status'}
  {...rest}
>
  {#if icon}
    <Icon size={16} class="mt-0.5 shrink-0" aria-hidden="true" />
  {/if}
  <div class="min-w-0 flex-1">
    {#if title}
      <p class="font-medium">{title}</p>
    {/if}
    <div class={title ? 'mt-0.5 text-fg-muted' : ''}>{@render children?.()}</div>
  </div>
</div>
