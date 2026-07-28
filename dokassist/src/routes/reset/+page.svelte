<script lang="ts">
  import { goto } from '$app/navigation';
  import { resetApp, parseError } from '$lib/api';
  import { t } from '$lib/translations';
  import { AlertTriangle, RotateCcw } from 'lucide-svelte';

  let confirmation = $state('');
  let isResetting = $state(false);
  let error = $state<string | null>(null);

  async function handleReset() {
    if (confirmation !== 'RESET' || isResetting) return;

    isResetting = true;
    error = null;
    try {
      await resetApp();
      await goto('/setup', { replaceState: true });
    } catch (err) {
      error = parseError(err).message || $t('auth.resetFailed');
    } finally {
      isResetting = false;
    }
  }
</script>

<div class="min-h-screen bg-gray-50 text-gray-900 dark:bg-gray-950 dark:text-gray-100 flex items-center justify-center p-8">
  <main class="max-w-md w-full rounded-2xl border border-red-200 bg-white p-8 shadow-xl space-y-6 dark:border-red-950 dark:bg-gray-900/70">
    <div class="text-center space-y-3">
      <div class="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-red-100 text-red-600 dark:bg-red-500/15 dark:text-red-300">
        <AlertTriangle size={24} aria-hidden="true" />
      </div>
      <h1 class="text-2xl font-bold">{$t('auth.resetTitle')}</h1>
      <p class="text-sm text-gray-600 dark:text-gray-400">{$t('auth.resetDescription')}</p>
    </div>

    <div class="rounded-xl border border-red-200 bg-red-50 p-4 text-sm text-red-900 dark:border-red-500/30 dark:bg-red-950/30 dark:text-red-100">
      {$t('auth.resetWarning')}
    </div>

    {#if error}
      <p class="rounded-xl border border-red-500/50 bg-red-50 p-4 text-sm text-red-800 dark:bg-red-950/40 dark:text-red-200" role="alert">{error}</p>
    {/if}

    <label for="reset-confirmation" class="block text-sm font-medium text-gray-700 dark:text-gray-300">
      {$t('auth.resetPrompt')}
    </label>
    <input
      id="reset-confirmation"
      bind:value={confirmation}
      autocomplete="off"
      class="mt-2 w-full rounded-xl border border-gray-300 bg-white px-4 py-3 text-gray-900 outline-none focus:ring-2 focus:ring-red-500 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-100"
      placeholder="RESET"
    />

    <div class="flex gap-3">
      <a href="/unlock" class="flex-1 rounded-xl border border-gray-300 px-4 py-3 text-center text-sm font-medium text-gray-700 hover:bg-gray-100 dark:border-gray-700 dark:text-gray-200 dark:hover:bg-gray-800">
        {$t('common.cancel')}
      </a>
      <button
        onclick={handleReset}
        disabled={confirmation !== 'RESET' || isResetting}
        class="flex-1 rounded-xl bg-red-600 px-4 py-3 text-sm font-medium text-white hover:bg-red-500 disabled:cursor-not-allowed disabled:bg-gray-400 dark:disabled:bg-gray-700"
      >
        {#if isResetting}
          {$t('auth.resetting')}
        {:else}
          <span class="inline-flex items-center gap-2"><RotateCcw size={16} aria-hidden="true" />{$t('auth.resetAction')}</span>
        {/if}
      </button>
    </div>
  </main>
</div>
