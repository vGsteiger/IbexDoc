<script lang="ts">
  import type { Patient } from '$lib/api';
  import { t } from '$lib/translations';

  interface Props {
    patient: Patient;
    onclick?: () => void;
  }

  let { patient, onclick }: Props = $props();

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

  function calculateAge(dateOfBirth: string): number {
    const birth = new Date(dateOfBirth);
    const today = new Date();
    let age = today.getFullYear() - birth.getFullYear();
    const monthDiff = today.getMonth() - birth.getMonth();
    if (monthDiff < 0 || (monthDiff === 0 && today.getDate() < birth.getDate())) {
      age--;
    }
    return age;
  }
</script>

<button
  {onclick}
  class="w-full text-left p-4 bg-surface-raised border border-line rounded-control hover:bg-surface-hover hover:border-line-strong transition-colors"
>
  <div class="flex justify-between items-start mb-2">
    <div>
      <h3 class="text-heading font-semibold text-fg">
        {patient.last_name}, {patient.first_name}
      </h3>
      <p class="text-body text-fg-muted">
        AHV: {patient.ahv_number}
      </p>
    </div>
    <div class="text-right">
      <p class="text-body text-fg-muted">
        {formatDate(patient.date_of_birth)}
      </p>
      <p class="text-caption text-fg-subtle">
        {$t('patients.age')}: {calculateAge(patient.date_of_birth)}
      </p>
    </div>
  </div>

  {#if patient.gender}
    <div class="flex gap-2 items-center">
      <span class="text-caption px-2 py-1 bg-surface-hover text-fg-muted rounded-card">
        {patient.gender}
      </span>
    </div>
  {/if}

  {#if patient.insurance}
    <div class="mt-2 text-body text-fg-muted">
      {$t('patients.insurance')}: {patient.insurance}
    </div>
  {/if}
</button>
