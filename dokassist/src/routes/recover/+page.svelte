<script lang="ts">
  import { goto } from '$app/navigation';
  import { recoverApp, parseError } from '$lib/api';
  import { authStatus } from '$lib/stores/auth';
  import { t } from '$lib/translations';
  import { ShieldCheck } from 'lucide-svelte';

  let words = $state<string[]>(Array(24).fill(''));
  let isRecovering = $state(false);
  let error = $state<string | null>(null);

  function handleInput(index: number, value: string) {
    words[index] = value.toLowerCase().trim();
  }

  async function handleRecover() {
    if (isRecovering) return;

    const filledWords = words.filter((w) => w.length > 0);
    if (filledWords.length !== 24) {
      error = $t('auth.recoveryMissingWords');
      return;
    }

    isRecovering = true;
    error = null;

    try {
      const recovered = await recoverApp(words);

      if (!recovered) {
        error = $t('auth.recoveryFailed');
        return;
      }

      authStatus.set('unlocked');
      goto('/dashboard');
    } catch (err) {
      error = parseError(err).message || $t('auth.recoveryFailed');
    } finally {
      isRecovering = false;
    }
  }
</script>

<div class="min-h-screen bg-gray-50 text-gray-900 dark:bg-gray-950 dark:text-gray-100 flex items-center justify-center p-8">
  <main class="max-w-4xl w-full rounded-2xl border border-gray-200 bg-white p-6 sm:p-8 space-y-6 shadow-xl dark:border-gray-800 dark:bg-gray-900/70 dark:shadow-2xl">
    <div class="text-center space-y-3">
      <div class="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-emerald-500/15 text-emerald-300">
        <ShieldCheck size={24} aria-hidden="true" />
      </div>
      <h1 class="text-3xl font-bold text-gray-900 dark:text-gray-100">{$t('auth.recoveryTitle')}</h1>
      <p class="mx-auto max-w-2xl text-gray-600 dark:text-gray-400">{$t('auth.recoverySubtitle')}</p>
    </div>

    {#if error}
      <div class="rounded-xl border border-red-500/50 bg-red-950/40 p-4" role="alert">
        <p class="text-sm text-red-200">{error}</p>
      </div>
    {/if}

    <div class="rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-4 text-sm text-emerald-100">
      <p class="font-medium">{$t('auth.recoveryNoticeTitle')}</p>
      <p class="mt-1 text-emerald-100/75">{$t('auth.recoveryNotice')}</p>
    </div>

    <div class="grid grid-cols-4 gap-3">
      {#each Array(24) as _, i}
        <div class="flex flex-col">
          <label for={`word-${i}`} class="text-gray-600 text-xs mb-1 dark:text-gray-400">{i + 1}.</label>
          <input
            id={`word-${i}`}
            type="text"
            value={words[i]}
            oninput={(e) => handleInput(i, (e.target as HTMLInputElement).value)}
            class="px-3 py-2 bg-white border border-gray-300 rounded-lg text-gray-900 focus:outline-none focus:ring-2 focus:ring-blue-500 dark:bg-gray-800 dark:border-gray-700 dark:text-gray-100"
            placeholder={$t('auth.wordPlaceholder').replace('{number}', String(i + 1))}
          />
        </div>
      {/each}
    </div>

    <div class="flex justify-center">
      <button
        onclick={handleRecover}
        disabled={isRecovering}
        class="px-6 py-3 bg-blue-600 hover:bg-blue-500 focus:outline-none focus:ring-2 focus:ring-blue-400 focus:ring-offset-2 focus:ring-offset-gray-900 disabled:bg-gray-700 disabled:cursor-not-allowed text-white font-medium rounded-xl transition-colors flex items-center gap-2"
      >
        {#if isRecovering}
          <div class="animate-spin rounded-full h-5 w-5 border-b-2 border-white"></div>
          <span>{$t('auth.recoveryInProgress')}</span>
        {:else}
          <span>{$t('auth.recoverAccount')}</span>
        {/if}
      </button>
    </div>
  </main>
</div>
