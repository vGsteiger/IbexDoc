<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { initializeApp, parseError } from '$lib/api';
  import { authStatus } from '$lib/stores/auth';
  import { t } from '$lib/translations';
  import MnemonicDisplay from '$lib/components/MnemonicDisplay.svelte';
  import { AlertTriangle } from 'lucide-svelte';

  let words = $state<string[]>([]);
  let isLoading = $state(true);
  let error = $state<string | null>(null);
  let showConfirmation = $state(false);
  let confirmIndices = $state<number[]>([]);
  let userInputs = $state<{ [key: number]: string }>({});
  let confirmError = $state<string | null>(null);

  async function createRecoveryPhrase() {
    isLoading = true;
    error = null;
    try {
      const mnemonic = await initializeApp();
      words = mnemonic;
    } catch (err) {
      const { code, message } = parseError(err);
      error = code === 'KEYCHAIN_ERROR' ? $t('auth.setupKeychainError') : message;
    } finally {
      isLoading = false;
    }
  }

  onMount(() => {
    void createRecoveryPhrase();
  });

  function startConfirmation() {
    const indices: number[] = [];
    while (indices.length < 3) {
      const randomIndex = Math.floor(Math.random() * 24);
      if (!indices.includes(randomIndex)) {
        indices.push(randomIndex);
      }
    }
    confirmIndices = indices.sort((a, b) => a - b);
    userInputs = {};
    confirmError = null;
    showConfirmation = true;
  }

  function validateConfirmation() {
    for (const index of confirmIndices) {
      if (userInputs[index]?.toLowerCase().trim() !== words[index]?.toLowerCase()) {
        confirmError = $t('auth.confirmWordsError');
        return;
      }
    }
    authStatus.set('unlocked');
    goto('/onboarding/step1');
  }
</script>

<div class="min-h-screen bg-gray-50 text-gray-900 dark:bg-gray-950 dark:text-gray-100 flex items-center justify-center p-8">
  <div class="max-w-4xl w-full">
    {#if isLoading}
      <div class="text-center">
        <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-500 mx-auto"></div>
        <p class="mt-4 text-gray-600 dark:text-gray-400">{$t('auth.setupGenerating')}</p>
      </div>
    {:else if error}
      <div class="bg-red-50 border border-red-300 rounded-lg p-6 text-center dark:bg-red-900/20 dark:border-red-500">
        <h2 class="text-xl font-bold text-red-500 mb-2">{$t('auth.setupFailed')}</h2>
        <p class="text-gray-700 dark:text-gray-300">{error}</p>
        <button
          onclick={createRecoveryPhrase}
          class="mt-5 px-5 py-2 bg-blue-600 hover:bg-blue-500 text-white font-medium rounded-lg transition-colors"
        >
          {$t('auth.retry')}
        </button>
      </div>
    {:else if !showConfirmation}
      <div class="space-y-6">
        <div class="text-center">
          <h1 class="text-3xl font-bold text-gray-900 dark:text-gray-100 mb-2">{$t('auth.welcomeToRamDoc')}</h1>
          <p class="text-gray-600 dark:text-gray-400">
            {$t('auth.setupIntro')}
          </p>
        </div>

        <div class="bg-yellow-50 border border-yellow-400 rounded-lg p-4 dark:bg-yellow-900/20 dark:border-yellow-600">
          <p class="text-yellow-700 dark:text-yellow-500 text-sm font-medium flex items-center gap-2">
            <AlertTriangle size={16} />
            {$t('auth.recoveryPhraseDesc')}
          </p>
        </div>

        <MnemonicDisplay {words} />

        <div class="flex justify-center">
          <button
            onclick={startConfirmation}
            class="px-6 py-3 bg-blue-600 hover:bg-blue-700 text-white font-medium rounded-lg transition-colors"
          >
            {$t('auth.writtenDown')}
          </button>
        </div>
      </div>
    {:else}
      <div class="space-y-6">
        <div class="text-center">
          <h2 class="text-2xl font-bold text-gray-900 dark:text-gray-100 mb-2">{$t('auth.confirmRecoveryPhrase')}</h2>
          <p class="text-gray-600 dark:text-gray-400">{$t('auth.confirmWordsPrompt')}</p>
        </div>

        {#if confirmError}
          <div class="bg-red-50 border border-red-500 rounded-lg p-4 dark:bg-red-900/20">
            <p class="text-red-500 text-sm">{confirmError}</p>
          </div>
        {/if}

        <div class="space-y-4 max-w-md mx-auto">
          {#each confirmIndices as index}
            <div>
              <label for={`confirm-word-${index}`} class="block text-gray-700 dark:text-gray-400 mb-2">
                {$t('auth.wordPlaceholder').replace('{number}', String(index + 1))}
              </label>
              <input
                id={`confirm-word-${index}`}
                type="text"
                bind:value={userInputs[index]}
                class="w-full px-4 py-3 bg-white border border-gray-300 rounded-lg text-gray-900 focus:outline-none focus:ring-2 focus:ring-blue-500 dark:bg-gray-800 dark:border-gray-700 dark:text-gray-100"
                placeholder={$t('auth.enterWord')}
              />
            </div>
          {/each}
        </div>

        <div class="flex justify-center gap-4">
          <button
            onclick={() => (showConfirmation = false)}
            class="px-6 py-3 bg-gray-700 hover:bg-gray-600 text-white font-medium rounded-lg transition-colors"
          >
            Back
          </button>
          <button
            onclick={validateConfirmation}
            class="px-6 py-3 bg-blue-600 hover:bg-blue-700 text-white font-medium rounded-lg transition-colors"
          >
            Continue
          </button>
        </div>
      </div>
    {/if}
  </div>
</div>
