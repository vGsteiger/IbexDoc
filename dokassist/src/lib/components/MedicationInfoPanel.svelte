<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import type { SubstanceDetail } from '$lib/api';
  import { t } from '$lib/translations';

  interface Props {
    substanceId: string | null;
  }

  let { substanceId }: Props = $props();

  let detail = $state<SubstanceDetail | null>(null);
  let loading = $state(false);
  let collapsed = $state(false);

  $effect(() => {
    if (!substanceId) {
      detail = null;
      return;
    }

    loading = true;
    collapsed = false;
    const currentSubstanceId = substanceId;
    invoke<SubstanceDetail>('get_medication_reference_detail', { id: substanceId })
      .then((d) => {
        if (currentSubstanceId === substanceId) {
          detail = d;
        }
      })
      .catch(() => {
        if (currentSubstanceId === substanceId) {
          detail = null;
        }
      })
      .finally(() => {
        if (currentSubstanceId === substanceId) {
          loading = false;
        }
      });
  });
</script>

{#if loading}
  <div
    class="mt-2 px-3 py-2 bg-surface-selected rounded-card text-caption text-fg-muted animate-pulse"
  >
    {$t('medications.loadingReference')}
  </div>
{:else if detail}
  <div
    class="mt-2 bg-surface-selected border border-line rounded-card overflow-hidden text-caption"
  >
    <!-- Header -->
    <button
      type="button"
      class="w-full flex items-center justify-between px-3 py-2 text-left hover:bg-surface-selected transition-colors"
      onclick={() => (collapsed = !collapsed)}
    >
      <div class="flex items-center gap-2">
        <span class="font-semibold text-fg">{detail.name_de}</span>
        {#if detail.atc_code}
          <span class="font-mono bg-accent-subtle text-accent-fg px-1.5 py-0.5 rounded-card">
            {detail.atc_code}
          </span>
        {/if}
        {#if detail.trade_names.length > 0}
          <span class="text-fg-muted">{detail.trade_names.slice(0, 3).join(' · ')}</span>
        {/if}
      </div>
      <span class="text-fg-muted text-caption">{collapsed ? '▸' : '▾'}</span>
    </button>

    {#if !collapsed}
      <div class="px-3 pb-3 space-y-2 border-t border-line">
        {#if detail.indication}
          <div class="pt-2">
            <p class="font-semibold text-fg-muted mb-0.5">{$t('medications.indication')}</p>
            <p class="text-fg-muted leading-relaxed">{detail.indication}</p>
          </div>
        {/if}

        {#if detail.side_effects}
          <div>
            <p class="font-semibold text-warning-fg mb-0.5">{$t('medications.sideEffects')}</p>
            <p class="text-fg-muted leading-relaxed">{detail.side_effects}</p>
          </div>
        {/if}

        {#if detail.contraindications}
          <div>
            <p class="font-semibold text-danger-fg mb-0.5">{$t('medications.contraindications')}</p>
            <p class="text-fg-muted leading-relaxed">{detail.contraindications}</p>
          </div>
        {/if}

        {#if detail.source_version}
          <p class="text-fg-muted pt-1">
            {$t('common.source')}: Swissmedic AIPS {detail.source_version}
          </p>
        {/if}
      </div>
    {/if}
  </div>
{/if}
