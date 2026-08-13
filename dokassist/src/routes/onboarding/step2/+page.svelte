<script lang="ts">
  import { errorText } from '$lib/translations/labels';
  import { t } from '$lib/translations';
  import { goto } from '$app/navigation';
  import { onMount, onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getRecommendedModel, downloadAndRegisterModel, type ModelChoice } from '$lib/api';
  import {
    ChevronRight,
    ChevronLeft,
    Download,
    Check,
    Cpu,
    Gauge,
    HardDrive,
    Target,
  } from 'lucide-svelte';

  let recommended = $state<ModelChoice | null>(null);
  let error = $state<string | null>(null);
  let isLoading = $state(true);
  let isDownloading = $state(false);
  let downloadProgress = $state<number>(0);
  let isComplete = $state(false);

  let unlisten: UnlistenFn | null = null;
  let doneUnsubscribe: UnlistenFn | null = null;

  onMount(async () => {
    try {
      recommended = await getRecommendedModel();
    } catch (err) {
      error = $errorText(err);
    } finally {
      isLoading = false;
    }
  });

  onDestroy(() => {
    unlisten?.();
    doneUnsubscribe?.();
  });

  async function handleDownload() {
    if (!recommended) return;

    isDownloading = true;
    downloadProgress = 0;
    error = null;

    try {
      unlisten = await listen<number>('model-download-progress', (e) => {
        downloadProgress = Math.round(e.payload * 100);
      });

      doneUnsubscribe = await listen('model-download-done', () => {
        isComplete = true;
      });

      await downloadAndRegisterModel(recommended);
    } catch (err) {
      error = $errorText(err);
    } finally {
      doneUnsubscribe?.();
      unlisten?.();
      doneUnsubscribe = null;
      unlisten = null;
      isDownloading = false;
    }
  }

  function handleContinue() {
    goto('/onboarding/step3');
  }

  function handleBack() {
    goto('/onboarding/step1');
  }

  function formatBytes(bytes: number): string {
    const gb = bytes / 1024 ** 3;
    return `${gb.toFixed(1)} GB`;
  }
</script>

<div class="min-h-screen bg-surface flex items-center justify-center p-8">
  <div class="max-w-3xl w-full">
    <div class="mb-8 text-center">
      <h1 class="text-display font-semibold text-fg mb-2">{$t('onboarding.step2.title')}</h1>
      <p class="text-fg-muted">{$t('onboarding.step2.subtitle')}</p>
      <div class="flex items-center justify-center gap-2 mt-4">
        <div class="h-2 w-16 bg-accent rounded-full"></div>
        <div class="h-2 w-16 bg-accent rounded-full"></div>
        <div class="h-2 w-16 bg-surface-selected rounded-full"></div>
        <div class="h-2 w-16 bg-surface-selected rounded-full"></div>
      </div>
    </div>

    {#if error}
      <div class="bg-danger-subtle border border-danger-line rounded-card p-4 mb-6">
        <p class="text-danger-fg text-body">{error}</p>
      </div>
    {/if}

    {#if isLoading}
      <div class="text-center py-12">
        <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-accent mx-auto"></div>
        <p class="mt-4 text-fg-muted">{$t('onboarding.step2.loading')}</p>
      </div>
    {:else if recommended}
      <div class="bg-surface-raised border border-line-subtle rounded-card p-8 space-y-6">
        <div class="text-center">
          <div class="inline-block p-4 bg-surface-hover rounded-card mb-4">
            <Cpu size={48} class="text-fg-subtle" />
          </div>
          <h2 class="text-display font-semibold text-fg mb-2">{recommended.name}</h2>
          <p class="text-fg-muted">{$t('onboarding.step2.recommendedForSystem')}</p>
        </div>

        <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div class="bg-surface-hover rounded-card p-4 text-center">
            <HardDrive size={24} class="text-fg-muted mx-auto mb-2" />
            <p class="text-fg-muted text-body mb-1">{$t('onboarding.step2.size')}</p>
            <p class="text-fg font-semibold">{formatBytes(recommended.size_bytes)}</p>
          </div>

          <div class="bg-surface-hover rounded-card p-4 text-center">
            <Gauge size={24} class="text-fg-muted mx-auto mb-2" />
            <p class="text-fg-muted text-body mb-1">{$t('onboarding.step2.speed')}</p>
            <p class="text-fg font-semibold">
              {recommended.name.includes('30B')
                ? $t('onboarding.step2.speedGood')
                : $t('onboarding.step2.speedFast')}
            </p>
          </div>

          <div class="bg-surface-hover rounded-card p-4 text-center">
            <Target size={24} class="text-fg-muted mx-auto mb-2" />
            <p class="text-fg-muted text-body mb-1">{$t('onboarding.step2.quality')}</p>
            <p class="text-fg font-semibold">
              {recommended.name.includes('30B')
                ? $t('onboarding.step2.qualityExcellent')
                : $t('onboarding.step2.qualityVeryGood')}
            </p>
          </div>
        </div>

        <div class="bg-accent-subtle border border-accent-line rounded-card p-4">
          <p class="text-accent-fg text-body">
            <strong>{$t('onboarding.step2.aboutModelLabel')}</strong>
            {$t('onboarding.step2.aboutModel')}
          </p>
        </div>

        {#if isDownloading}
          <div class="space-y-4">
            <div class="flex items-center justify-between text-body text-fg-muted">
              <span>{$t('onboarding.step2.downloading')}</span>
              <span>{downloadProgress}%</span>
            </div>
            <div class="w-full bg-surface-hover rounded-full h-3 overflow-hidden">
              <div
                class="bg-accent h-full transition-colors duration-300 ease-out"
                style="width: {downloadProgress}%"
              ></div>
            </div>
            <p class="text-center text-fg-muted text-body">
              {$t('onboarding.step2.downloadHint')}
            </p>
          </div>
        {:else if isComplete}
          <div
            class="bg-success-subtle border border-success-line rounded-card p-4 flex items-center gap-3"
          >
            <div
              class="flex-shrink-0 w-10 h-10 bg-success rounded-full flex items-center justify-center"
            >
              <Check size={24} class="text-on-success" />
            </div>
            <div>
              <p class="text-success-fg font-semibold">{$t('onboarding.step2.downloadComplete')}</p>
              <p class="text-fg-muted text-body">{$t('onboarding.step2.modelReady')}</p>
            </div>
          </div>
        {/if}
      </div>

      <div class="flex justify-between items-center mt-8">
        <button
          onclick={handleBack}
          disabled={isDownloading}
          class="px-6 py-3 border border-line bg-surface-raised hover:bg-surface-hover disabled:bg-surface-hover disabled:cursor-not-allowed text-fg font-medium rounded-control transition-colors flex items-center gap-2"
        >
          <ChevronLeft size={20} />
          {$t('common.back')}
        </button>

        <div class="flex gap-3">
          {#if !isComplete && !isDownloading}
            <button
              onclick={handleContinue}
              class="px-6 py-3 text-fg-muted hover:text-fg font-medium transition-colors"
            >
              {$t('onboarding.skipForNow')}
            </button>

            <button
              onclick={handleDownload}
              class="px-6 py-3 bg-accent hover:bg-accent-hover text-on-accent font-medium rounded-control transition-colors flex items-center gap-2"
            >
              <Download size={20} />
              {$t('onboarding.step2.downloadModel')}
            </button>
          {:else if isComplete}
            <button
              onclick={handleContinue}
              class="px-6 py-3 bg-accent hover:bg-accent-hover text-on-accent font-medium rounded-control transition-colors flex items-center gap-2"
            >
              {$t('common.continue')}
              <ChevronRight size={20} />
            </button>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</div>
