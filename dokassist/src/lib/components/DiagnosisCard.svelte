<script lang="ts">
  import { t } from '$lib/translations';
  import type { Diagnosis } from '$lib/api';

  interface Props {
    diagnosis: Diagnosis;
    onEdit?: () => void;
    onDelete?: () => void;
  }

  let { diagnosis, onEdit, onDelete }: Props = $props();

  function formatDate(dateStr: string): string {
    try {
      const date = new Date(dateStr);
      return date.toLocaleDateString('de-CH', {
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
      });
    } catch {
      return dateStr;
    }
  }

  function getStatusColor(status: string): string {
    switch (status) {
      case 'active':
        return 'bg-success-subtle/20 text-success-fg border-success-line/30';
      case 'remission':
        return 'bg-warning-subtle/20 text-warning-fg border-warning-line/30';
      case 'resolved':
        return 'bg-surface-selected/20 text-fg-muted border-line-strong/30';
      default:
        return 'bg-accent-subtle/20 text-accent-fg border-accent-line/30';
    }
  }

  function getStatusLabel(status: string): string {
    switch (status) {
      case 'active':
        return $t('diagnoses.active');
      case 'remission':
        return $t('diagnoses.remission');
      case 'resolved':
        return $t('diagnoses.resolved');
      default:
        return status;
    }
  }
</script>

<div class="p-4 bg-surface-raised rounded-card border border-line">
  <div class="flex justify-between items-start mb-2">
    <div class="flex-1">
      <div class="flex items-center gap-2 mb-1">
        <span class="font-mono text-body text-accent-fg">{diagnosis.icd10_code}</span>
        <span
          class="px-2 py-0.5 rounded-full text-caption border {getStatusColor(diagnosis.status)}"
        >
          {getStatusLabel(diagnosis.status)}
        </span>
      </div>
      <h3 class="text-body font-medium text-fg">
        {diagnosis.description}
      </h3>
      <p class="text-body text-fg-muted mt-1">
        Diagnostiziert: {formatDate(diagnosis.diagnosed_date)}
        {#if diagnosis.resolved_date}
          • {$t('diagnoses.resolved')}: {formatDate(diagnosis.resolved_date)}
        {/if}
      </p>
    </div>
    <div class="flex gap-2 ml-2">
      {#if onEdit}
        <button
          type="button"
          class="p-2 text-fg-muted hover:text-accent-fg hover:bg-surface-hover rounded-control transition-colors"
          onclick={onEdit}
          title={$t('common.edit')}
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"
            />
          </svg>
        </button>
      {/if}
      {#if onDelete}
        <button
          type="button"
          class="p-2 text-fg-muted hover:text-danger-fg hover:bg-surface-hover rounded-control transition-colors"
          onclick={onDelete}
          title={$t('common.delete')}
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path
              stroke-linecap="round"
              stroke-linejoin="round"
              stroke-width="2"
              d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
            />
          </svg>
        </button>
      {/if}
    </div>
  </div>
  {#if diagnosis.notes}
    <p class="text-body text-fg-muted mt-2">{diagnosis.notes}</p>
  {/if}
</div>
