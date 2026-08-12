<script lang="ts">
  import { goto } from '$app/navigation';
  import { createPatient, parseError, type CreatePatient, type UpdatePatient } from '$lib/api';
  import PatientForm from '$lib/components/PatientForm.svelte';
  import { addToast } from '$lib/stores/toast';

  let isSubmitting = $state(false);
  let error = $state('');

  async function handleSubmit(
    event: CustomEvent<CreatePatient | { id: string; data: UpdatePatient }>
  ) {
    if ('id' in event.detail) return;
    try {
      isSubmitting = true;
      error = '';
      const patient = await createPatient(event.detail);
      addToast('Patient created');
      goto(`/patients/${patient.id}`);
    } catch (e) {
      const { code } = parseError(e);
      if (code === 'DB_UNIQUE_CONSTRAINT') {
        error = 'A patient with this AHV number already exists.';
      } else {
        error = e instanceof Error ? e.message : 'Failed to create patient';
      }
      console.error('Error creating patient:', e);
      isSubmitting = false;
    }
  }

  function handleCancel() {
    goto('/patients');
  }
</script>

<div class="p-8">
  <div class="max-w-3xl mx-auto">
    <h1 class="text-display font-semibold text-fg mb-6">New Patient</h1>

    {#if error}
      <div class="mb-6 bg-danger-subtle border border-danger-line rounded-card p-4 text-danger-fg">
        {error}
      </div>
    {/if}

    <div class="bg-surface-raised rounded-card p-6">
      <PatientForm on:submit={handleSubmit} on:cancel={handleCancel} {isSubmitting} />
    </div>
  </div>
</div>
