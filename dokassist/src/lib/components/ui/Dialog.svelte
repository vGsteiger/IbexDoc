<script lang="ts">
  import type { Snippet } from 'svelte';
  import { X } from 'lucide-svelte';

  let {
    open = $bindable(false),
    title = undefined,
    description = undefined,
    size = 'md',
    onClose = undefined,
    class: className = '',
    children,
    footer,
  }: {
    open?: boolean;
    title?: string;
    description?: string;
    size?: 'sm' | 'md' | 'lg';
    onClose?: () => void;
    class?: string;
    children?: Snippet;
    footer?: Snippet;
  } = $props();

  const sizes = { sm: 'max-w-sm', md: 'max-w-lg', lg: 'max-w-3xl' };

  let panel = $state<HTMLElement | null>(null);
  let titleId = $props.id();

  function close() {
    open = false;
    onClose?.();
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.stopPropagation();
      close();
    }
  }

  /* Move focus into the panel when it opens so keyboard users are not left
   * behind on the trigger. */
  $effect(() => {
    if (open && panel) {
      const target = panel.querySelector<HTMLElement>(
        'input, textarea, select, button:not([data-dialog-dismiss])'
      );
      (target ?? panel).focus();
    }
  });
</script>

<svelte:window onkeydown={open ? onKeydown : undefined} />

{#if open}
  <div class="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto p-4 sm:p-8">
    <!-- The scrim is a plain black wash in both themes; tinting it with a
         token would make it read as a surface. -->
    <button
      type="button"
      class="fixed inset-0 bg-black/25 dark:bg-black/55"
      aria-label="Close dialog"
      data-dialog-dismiss
      onclick={close}
    ></button>

    <div
      bind:this={panel}
      role="dialog"
      aria-modal="true"
      aria-labelledby={title ? titleId : undefined}
      tabindex="-1"
      class="relative z-10 my-auto w-full {sizes[
        size
      ]} rounded-card border border-line bg-surface-overlay shadow-modal focus:outline-none {className}"
    >
      {#if title}
        <header class="flex items-start gap-3 border-b border-line-subtle px-4 py-3">
          <div class="min-w-0 flex-1">
            <h2 id={titleId} class="text-heading text-fg">{title}</h2>
            {#if description}
              <p class="mt-0.5 text-caption text-fg-muted">{description}</p>
            {/if}
          </div>
          <button
            type="button"
            data-dialog-dismiss
            onclick={close}
            class="-mr-1 -mt-0.5 rounded-control p-1 text-fg-subtle transition-colors duration-150 ease-standard hover:bg-surface-hover hover:text-fg"
            aria-label="Close"
          >
            <X size={16} />
          </button>
        </header>
      {/if}

      <div class="px-4 py-3">{@render children?.()}</div>

      {#if footer}
        <footer class="flex justify-end gap-2 border-t border-line-subtle px-4 py-3">
          {@render footer()}
        </footer>
      {/if}
    </div>
  </div>
{/if}
