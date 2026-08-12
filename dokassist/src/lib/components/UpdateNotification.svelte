<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { checkForUpdates, installUpdate, parseError, type UpdateInfo } from '$lib/api';
  import { t } from '$lib/translations';

  function renderMarkdown(text: string): string {
    function escape(s: string) {
      return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    }
    function inline(s: string) {
      return s
        .replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>')
        .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1');
    }
    return text
      .split('\n')
      .map((line) => {
        if (/^## /.test(line))
          return `<p class="font-semibold mt-2 mb-0.5">${inline(escape(line.slice(3)))}</p>`;
        if (/^### /.test(line))
          return `<p class="font-medium mt-1 mb-0.5">${inline(escape(line.slice(4)))}</p>`;
        if (/^- /.test(line)) return `<p class="ml-3">&bull; ${inline(escape(line.slice(2)))}</p>`;
        if (line.trim() === '') return '<div class="mt-1"></div>';
        return `<p>${inline(escape(line))}</p>`;
      })
      .join('');
  }

  let updateInfo = $state<UpdateInfo | null>(null);
  let installing = $state(false);
  let downloadProgress = $state<number>(0);
  let errorMsg = $state('');
  let showNotification = $state(false);
  let unlisten: UnlistenFn | null = null;

  let { autoCheck = true }: { autoCheck?: boolean } = $props();

  onMount(async () => {
    if (autoCheck) {
      await handleCheckForUpdates();
    }
  });

  onDestroy(() => {
    unlisten?.();
  });

  async function handleCheckForUpdates() {
    errorMsg = '';
    try {
      updateInfo = await checkForUpdates();
      if (updateInfo.update_available) {
        showNotification = true;
      }
    } catch (e) {
      errorMsg = parseError(e).message;
    }
  }

  async function handleInstallUpdate() {
    if (!updateInfo?.update_available) return;

    installing = true;
    errorMsg = '';
    downloadProgress = 0;

    // Listen for download progress events
    unlisten = await listen<number>('updater-download-progress', (e) => {
      downloadProgress = Math.round(e.payload * 100);
    });

    const completeUnsub = await listen('updater-download-complete', () => {
      completeUnsub();
    });

    try {
      await installUpdate();
      // After successful install, the app will restart automatically
    } catch (e) {
      unlisten?.();
      unlisten = null;
      installing = false;
      errorMsg = parseError(e).message;
    }
  }

  function dismiss() {
    showNotification = false;
  }
</script>

{#if showNotification && updateInfo?.update_available}
  <div
    class="fixed top-4 right-4 z-50 bg-surface-raised border border-line rounded-card shadow-popover p-4 max-w-md"
  >
    <div class="flex items-start justify-between gap-4">
      <div class="flex-1">
        <h3 class="text-body font-semibold text-fg mb-1">Update Available</h3>
        <p class="text-caption text-fg-muted mb-2">
          Version {updateInfo.latest_version} is now available. You are currently on version {updateInfo.current_version}.
        </p>
        {#if updateInfo.body}
          <div class="text-caption text-fg-muted mb-3 max-h-24 overflow-y-auto">
            <p class="font-medium mb-1">What's new:</p>
            <div>{@html renderMarkdown(updateInfo.body)}</div>
          </div>
        {/if}

        {#if installing}
          <div class="mb-3">
            <div class="flex justify-between text-caption text-fg-muted mb-1">
              <span>Downloading update...</span>
              <span>{downloadProgress}%</span>
            </div>
            <div class="w-full bg-surface-selected rounded-full h-2">
              <div
                class="bg-accent h-2 rounded-full transition-colors"
                style="width: {downloadProgress}%"
              ></div>
            </div>
          </div>
        {/if}

        {#if errorMsg}
          <p class="text-caption text-danger-fg mb-3">{errorMsg}</p>
        {/if}

        <div class="flex gap-2">
          <button
            onclick={handleInstallUpdate}
            disabled={installing}
            class="h-7 px-2.5 text-caption rounded-control bg-accent hover:bg-accent-hover disabled:opacity-50 disabled:cursor-not-allowed text-on-accent transition-colors"
          >
            {installing ? 'Installing...' : 'Install Update'}
          </button>
          <button
            onclick={dismiss}
            disabled={installing}
            class="h-7 px-2.5 text-caption rounded-control bg-surface-selected hover:bg-surface-selected disabled:opacity-50 disabled:cursor-not-allowed text-fg transition-colors"
          >
            Later
          </button>
        </div>
      </div>
      <button
        onclick={dismiss}
        disabled={installing}
        aria-label={$t('common.close')}
        class="text-fg-muted hover:text-fg transition-colors disabled:opacity-50"
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M6 18L18 6M6 6l12 12"
          />
        </svg>
      </button>
    </div>
  </div>
{/if}
