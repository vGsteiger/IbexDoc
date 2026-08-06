<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { checkAuth, unlockApp, parseError } from '$lib/api';
  import { authStatus } from '$lib/stores/auth';
  import { t } from '$lib/translations';
  import { Fingerprint, KeyRound } from 'lucide-svelte';

  let isUnlocking = $state(false);
  let error = $state<string | null>(null);

  // The browser history (or a manually entered URL) can reach this page even
  // though recovery is required. Do not offer a locked-only unlock action in
  // that state.
  onMount(() => {
    void (async () => {
      if (await checkAuth() === 'recovery_required') {
        authStatus.set('recovery_required');
        await goto('/recover', { replaceState: true });
      }
    })();
  });

  function friendlyError(err: unknown): string {
    const { code, message } = parseError(err);
    switch (code) {
      case 'KEYCHAIN_ERROR':
        return $t('auth.keychainAccessError');
      case 'DATABASE_ERROR':
        return $t('auth.databaseAccessError');
      case 'FILESYSTEM_ERROR':
        return $t('auth.filesystemAccessError');
      case 'AUTH_REQUIRED':
        return $t('auth.unlockFailed');
      default:
        return message || $t('auth.unlockFailed');
    }
  }

  async function handleUnlock() {
    if (isUnlocking) return;
    isUnlocking = true;
    error = null;

    try {
      const unlocked = await unlockApp();
      if (!unlocked) {
        error = $t('auth.unlockFailed');
        return;
      }
      authStatus.set('unlocked');
      goto('/dashboard');
    } catch (err) {
      const { code } = parseError(err);
      // User dismissed the Touch ID sheet — not an error, just stay on screen.
      if (code === 'BIOMETRIC_CANCELLED') return;
      // A missing or invalidated master-key item cannot be restored by trying
      // Touch ID again. The backend has already moved this session to recovery.
      if (code === 'RECOVERY_REQUIRED') {
        authStatus.set('recovery_required');
        await goto('/recover', { replaceState: true });
        return;
      }
      error = friendlyError(err);
    } finally {
      isUnlocking = false;
    }
  }
</script>

<div class="min-h-screen bg-gray-50 text-gray-900 dark:bg-gray-950 dark:text-gray-100 flex items-center justify-center p-8">
  <main class="max-w-md w-full rounded-2xl border border-gray-200 bg-white p-8 text-center shadow-xl space-y-8 dark:border-gray-800 dark:bg-gray-900/70 dark:shadow-2xl">
    <div class="space-y-3">
      <div class="mx-auto flex h-12 w-12 items-center justify-center rounded-full bg-blue-500/15 text-blue-300">
        <KeyRound size={24} aria-hidden="true" />
      </div>
      <h1 class="text-3xl font-bold text-gray-900 dark:text-gray-100">{$t('auth.welcomeBack')}</h1>
      <p class="text-gray-600 dark:text-gray-400">{$t('auth.unlockSubtitle')}</p>
    </div>

    {#if error}
      <div class="rounded-xl border border-red-500/50 bg-red-950/40 p-4 text-left" role="alert">
        <p class="text-sm text-red-200">{error}</p>
      </div>
    {/if}

    <div class="space-y-4">
      <button
        onclick={handleUnlock}
        disabled={isUnlocking}
        class="w-full px-6 py-4 bg-blue-600 hover:bg-blue-500 focus:outline-none focus:ring-2 focus:ring-blue-400 focus:ring-offset-2 focus:ring-offset-gray-900 disabled:bg-gray-700 disabled:cursor-not-allowed text-white font-medium rounded-xl transition-colors flex items-center justify-center gap-3"
      >
        {#if isUnlocking}
          <div class="animate-spin rounded-full h-5 w-5 border-b-2 border-white"></div>
          <span>{$t('auth.unlocking')}</span>
        {:else}
          <Fingerprint size={22} aria-hidden="true" />
          <span>{$t('auth.unlockWithTouchID')}</span>
        {/if}
      </button>

      <a href="/recover" class="block text-sm text-blue-400 hover:text-blue-300 transition-colors">
        {$t('auth.recoveryLink')}
      </a>
    </div>

    <div class="border-t border-gray-200 pt-6 dark:border-gray-800">
      <a href="/reset" class="text-xs text-gray-500 hover:text-red-600 transition-colors dark:text-gray-500 dark:hover:text-red-300">
        {$t('auth.resetLink')}
      </a>
    </div>
  </main>
</div>
