<script lang="ts">
  import type { Patient } from '$lib/api';
  import { Badge, Card } from '$lib/components/ui';
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

<Card padding="sm" {onclick}>
  <div class="flex items-start justify-between gap-4">
    <div class="min-w-0">
      <h3 class="truncate text-heading text-fg">
        {patient.last_name}, {patient.first_name}
      </h3>
      <p class="mt-0.5 text-caption text-fg-muted" data-numeric>
        AHV: {patient.ahv_number}
      </p>
    </div>
    <div class="shrink-0 text-right">
      <p class="text-body text-fg-muted" data-numeric>
        {formatDate(patient.date_of_birth)}
      </p>
      <p class="text-caption text-fg-subtle">
        {$t('patients.age')}: {calculateAge(patient.date_of_birth)}
      </p>
    </div>
  </div>

  {#if patient.gender || patient.insurance}
    <div class="mt-2 flex items-center gap-2">
      {#if patient.gender}
        <Badge>{patient.gender}</Badge>
      {/if}
      {#if patient.insurance}
        <span class="truncate text-caption text-fg-subtle">
          {$t('patients.insurance')}: {patient.insurance}
        </span>
      {/if}
    </div>
  {/if}
</Card>
