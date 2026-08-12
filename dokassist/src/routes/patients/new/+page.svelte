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
  import { Alert, Card, PageHeader } from '$lib/components/ui';
</script>

<div class="p-8">
  <div class="max-w-3xl mx-auto">
    <PageHeader title="New Patient" />

    {#if error}
      <Alert tone="danger" class="mb-4">{error}</Alert>
    {/if}

    <Card>
      <PatientForm on:submit={handleSubmit} on:cancel={handleCancel} {isSubmitting} />
    </Card>
  </div>
</div>
