<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { listPatients, globalSearch, type Patient } from '$lib/api';
  import PatientCard from '$lib/components/PatientCard.svelte';
  import { t } from '$lib/translations';

  let patients = $state<Patient[]>([]);
  let filteredPatients = $state<Patient[]>([]);
  let searchQuery = $state('');
  let isLoading = $state(true);
  let error = $state('');
  let sortBy = $state<'name' | 'created'>('name');

  // Debounced search
  let searchTimeout: ReturnType<typeof setTimeout> | null = null;

  onMount(async () => {
    await loadPatients();
  });

  async function loadPatients() {
    try {
      isLoading = true;
      error = '';
      patients = await listPatients(100, 0);
      filteredPatients = patients;
    } catch (e) {
      error = e instanceof Error ? e.message : 'Failed to load patients';
      console.error('Error loading patients:', e);
    } finally {
      isLoading = false;
    }
  }

  async function handleSearch(query: string) {
    searchQuery = query;

    // Clear existing timeout
    if (searchTimeout) {
      clearTimeout(searchTimeout);
    }

    // Debounce search by 300ms
    searchTimeout = setTimeout(async () => {
      if (!query.trim()) {
        filteredPatients = patients;
        return;
      }

      try {
        const results = await globalSearch(query, 50);
        // Filter to only patient results
        const patientResults = results.filter((r) => r.result_type === 'patient');

        // Get patient IDs from search results
        const patientIds = new Set(patientResults.map((r) => r.entity_id));

        // Filter patients by search results
        filteredPatients = patients.filter((p) => patientIds.has(p.id));
      } catch (e) {
        console.error('Search error:', e);
        // On search error, fall back to client-side filtering
        const lowerQuery = query.toLowerCase();
        filteredPatients = patients.filter(
          (p) =>
            p.first_name.toLowerCase().includes(lowerQuery) ||
            p.last_name.toLowerCase().includes(lowerQuery) ||
            (p.ahv_number?.includes(query) ?? false)
        );
      }
    }, 300);
  }

  function sortPatients(pats: Patient[]): Patient[] {
    if (sortBy === 'name') {
      return [...pats].sort((a, b) => {
        const nameA = `${a.last_name} ${a.first_name}`.toLowerCase();
        const nameB = `${b.last_name} ${b.first_name}`.toLowerCase();
        return nameA.localeCompare(nameB);
      });
    } else {
      return [...pats].sort((a, b) => {
        return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
      });
    }
  }

  let sortedPatients = $derived(sortPatients(filteredPatients));

  function handlePatientClick(patientId: string) {
    goto(`/patients/${patientId}`);
  }

  function handleNewPatient() {
    goto('/patients/new');
  }
</script>

<div class="p-8">
  <div class="max-w-7xl mx-auto">
    <!-- Header -->
    <div class="flex justify-between items-center mb-6">
      <h1 class="text-display font-semibold text-fg">{$t('patients.title')}</h1>
      <button
        onclick={handleNewPatient}
        class="h-8 px-3 bg-accent text-on-accent rounded-control hover:bg-accent-hover transition-colors"
      >
        + {$t('patients.newPatient')}
      </button>
    </div>

    <!-- Search and Sort -->
    <div class="flex gap-4 mb-6">
      <div class="flex-1">
        <input
          type="search"
          placeholder={$t('patients.search')}
          bind:value={searchQuery}
          oninput={(e) => handleSearch(e.currentTarget.value)}
          class="w-full px-4 py-2 bg-surface-raised border border-line rounded-control text-fg focus:outline-none focus:border-accent"
        />
      </div>
      <select
        bind:value={sortBy}
        class="px-4 py-2 bg-surface-raised border border-line rounded-control text-fg focus:outline-none focus:border-accent"
      >
        <option value="name">{$t('patients.sortByName')}</option>
        <option value="created">{$t('patients.sortByCreated')}</option>
      </select>
    </div>

    <!-- Patient Count -->
    {#if !isLoading}
      <div class="mb-4 text-body text-fg-muted">
        {sortedPatients.length}
        {sortedPatients.length === 1 ? $t('patients.patient') : $t('patients.patients')}
        {#if searchQuery}
          {$t('patients.matching')} "{searchQuery}"
        {/if}
      </div>
    {/if}

    <!-- Loading State -->
    {#if isLoading}
      <div class="flex justify-center items-center py-12">
        <div class="text-fg-muted">{$t('common.loading')}</div>
      </div>
    {:else if error}
      <div class="bg-danger-subtle border border-danger-line rounded-card p-4 text-danger-fg">
        {error}
      </div>
    {:else if sortedPatients.length === 0}
      <div class="text-center py-12">
        <p class="text-fg-muted mb-4">
          {searchQuery ? $t('patients.noSearchResults') : $t('patients.noPatients')}
        </p>
        {#if !searchQuery}
          <button
            onclick={handleNewPatient}
            class="h-8 px-3 bg-accent text-on-accent rounded-control hover:bg-accent-hover transition-colors"
          >
            {$t('patients.createFirst')}
          </button>
        {/if}
      </div>
    {:else}
      <!-- Patient List -->
      <div class="grid gap-4">
        {#each sortedPatients as patient (patient.id)}
          <PatientCard {patient} onclick={() => handlePatientClick(patient.id)} />
        {/each}
      </div>
    {/if}
  </div>
</div>
