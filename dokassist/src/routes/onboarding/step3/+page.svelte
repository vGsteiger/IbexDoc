<script lang="ts">
  import { t } from '$lib/translations';
  import { goto } from '$app/navigation';
  import { ChevronRight, ChevronLeft, FileText, Database } from 'lucide-svelte';

  function handleBack() {
    goto('/onboarding/step2');
  }

  function handleImportLater() {
    goto('/onboarding/step4');
  }
</script>

<div class="min-h-screen bg-surface flex items-center justify-center p-8">
  <div class="max-w-3xl w-full">
    <div class="mb-8 text-center">
      <h1 class="text-display font-semibold text-fg mb-2">{$t('onboarding.step3.title')}</h1>
      <p class="text-fg-muted">{$t('onboarding.step3.subtitle')}</p>
      <div class="flex items-center justify-center gap-2 mt-4">
        <div class="h-2 w-16 bg-accent rounded-full"></div>
        <div class="h-2 w-16 bg-accent rounded-full"></div>
        <div class="h-2 w-16 bg-accent rounded-full"></div>
        <div class="h-2 w-16 bg-surface-selected rounded-full"></div>
      </div>
    </div>

    <div class="bg-surface-raised border border-line-subtle rounded-card p-8 space-y-6">
      <div class="text-center">
        <div class="inline-block p-4 bg-surface-hover rounded-full mb-4">
          <Database size={48} class="text-fg-muted" />
        </div>
        <h2 class="text-title font-semibold text-fg mb-2">{$t('onboarding.step3.optionsTitle')}</h2>
        <p class="text-fg-muted">{$t('onboarding.step3.optionsSubtitle')}</p>
      </div>

      <div class="space-y-4">
        <button
          onclick={() => goto('/settings')}
          class="w-full bg-surface-hover hover:bg-surface-hover border border-line rounded-control p-6 text-left transition-colors group"
        >
          <div class="flex items-start gap-4">
            <div
              class="flex-shrink-0 w-12 h-12 bg-accent-subtle rounded-card flex items-center justify-center group-hover:bg-accent-subtle/30 transition-colors"
            >
              <FileText size={24} class="text-accent-fg" />
            </div>
            <div class="flex-1">
              <h3 class="text-fg font-semibold mb-1">{$t('onboarding.step3.importCsv')}</h3>
              <p class="text-fg-muted text-body">{$t('onboarding.step3.importCsvDesc')}</p>
            </div>
            <ChevronRight
              size={20}
              class="text-fg-muted group-hover:text-fg-muted transition-colors"
            />
          </div>
        </button>

        <button
          onclick={() => goto('/settings')}
          class="w-full bg-surface-hover hover:bg-surface-hover border border-line rounded-control p-6 text-left transition-colors group"
        >
          <div class="flex items-start gap-4">
            <div
              class="flex-shrink-0 w-12 h-12 bg-success-subtle rounded-card flex items-center justify-center group-hover:bg-success-subtle/30 transition-colors"
            >
              <Database size={24} class="text-success-fg" />
            </div>
            <div class="flex-1">
              <h3 class="text-fg font-semibold mb-1">{$t('onboarding.step3.restoreBackup')}</h3>
              <p class="text-fg-muted text-body">{$t('onboarding.step3.restoreBackupDesc')}</p>
            </div>
            <ChevronRight
              size={20}
              class="text-fg-muted group-hover:text-fg-muted transition-colors"
            />
          </div>
        </button>
      </div>

      <div class="bg-warning-subtle border border-warning-line rounded-card p-4">
        <p class="text-warning-fg text-body">
          <strong>{$t('onboarding.step3.noteLabel')}</strong>
          {$t('onboarding.step3.note')}
        </p>
      </div>
    </div>

    <div class="flex justify-between items-center mt-8">
      <button
        onclick={handleBack}
        class="px-6 py-3 border border-line bg-surface-raised hover:bg-surface-hover text-fg font-medium rounded-control transition-colors flex items-center gap-2"
      >
        <ChevronLeft size={20} />
        {$t('common.back')}
      </button>

      <button
        onclick={handleImportLater}
        class="px-6 py-3 bg-accent hover:bg-accent-hover text-on-accent font-medium rounded-control transition-colors flex items-center gap-2"
      >
        {$t('onboarding.skipForNow')}
        <ChevronRight size={20} />
      </button>
    </div>
  </div>
</div>
