<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { getDashboardData, type DashboardData } from '$lib/api';
  import { t } from '$lib/translations';
  import { language } from '$lib/stores/language';
  import { Calendar, Users, FileText, Plus } from 'lucide-svelte';

  let data = $state<DashboardData | null>(null);
  let isLoading = $state(true);
  let error = $state<string | null>(null);

  function getSessionTypeLabel(sessionType: string): string {
    const key = `sessions.types.${sessionType}`;
    const translated = $t(key);
    // If translation doesn't exist, $t returns the key itself
    return translated === key ? sessionType : translated;
  }

  function formatDate(isoDate: string): string {
    const d = new Date(isoDate + 'T00:00:00');
    const locale = $language === 'de' ? 'de-CH' : 'en-US';
    return d.toLocaleDateString(locale, {
      day: 'numeric',
      month: 'long',
      year: 'numeric',
    });
  }

  onMount(async () => {
    try {
      data = await getDashboardData();
    } catch (err) {
      console.error('Failed to load dashboard data:', err);
      error = err instanceof Error ? err.message : $t('dashboard.loadError');
    } finally {
      isLoading = false;
    }
  });
</script>

<div class="p-8 max-w-7xl mx-auto">
  <div class="mb-8">
    <h1 class="text-display font-semibold text-fg">{$t('dashboard.title')}</h1>
  </div>

  {#if isLoading}
    <div class="text-center py-12">
      <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-accent mx-auto"></div>
      <p class="mt-4 text-fg-muted">{$t('common.loading')}</p>
    </div>
  {:else if error}
    <div class="bg-danger-subtle border border-danger-line rounded-card p-6 text-center">
      <p class="text-danger-fg">{error}</p>
    </div>
  {:else if data}
    <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
      <!-- Today's Sessions -->
      <div class="bg-surface-raised border border-line rounded-card p-6">
        <div class="flex items-center gap-3 mb-4">
          <div class="p-2 bg-accent-subtle rounded-card">
            <Calendar size={20} class="text-accent-fg " />
          </div>
          <h2 class="text-heading font-semibold text-fg">{$t('dashboard.todaysSessions')}</h2>
        </div>

        {#if data.todays_sessions.length === 0}
          <p class="text-body text-fg-muted">{$t('dashboard.noSessionsToday')}</p>
        {:else}
          <div class="space-y-3">
            {#each data.todays_sessions as item}
              <button
                onclick={() => goto(`/patients/${item.session.patient_id}/sessions`)}
                class="w-full text-left bg-surface-sunken hover:bg-surface-hover rounded-control p-3 transition-colors"
              >
                <p class="text-body font-medium text-fg truncate">{item.patient_name}</p>
                <div class="flex items-center gap-2 mt-1">
                  <span
                    class="text-caption px-2 py-0.5 rounded-full bg-accent-subtle text-accent-fg"
                  >
                    {getSessionTypeLabel(item.session.session_type)}
                  </span>
                  {#if item.session.duration_minutes}
                    <span class="text-caption text-fg-muted"
                      >{item.session.duration_minutes} {$t('dashboard.minutes')}</span
                    >
                  {/if}
                </div>
              </button>
            {/each}
          </div>
        {/if}

        <button
          onclick={() => goto('/calendar')}
          class="w-full mt-4 h-8 px-3 text-body font-medium text-accent-fg hover:bg-accent-subtle rounded-control transition-colors"
        >
          {$t('dashboard.viewCalendar')}
        </button>
      </div>

      <!-- Recent Patients -->
      <div class="bg-surface-raised border border-line rounded-card p-6">
        <div class="flex items-center gap-3 mb-4">
          <div class="p-2 bg-success-subtle rounded-card">
            <Users size={20} class="text-success-fg " />
          </div>
          <h2 class="text-heading font-semibold text-fg">{$t('dashboard.recentPatients')}</h2>
        </div>

        {#if data.recent_patients.length === 0}
          <p class="text-body text-fg-muted">{$t('dashboard.noRecentPatients')}</p>
        {:else}
          <div class="space-y-3">
            {#each data.recent_patients as patient}
              <button
                onclick={() => goto(`/patients/${patient.id}`)}
                class="w-full text-left bg-surface-sunken hover:bg-surface-hover rounded-control p-3 transition-colors"
              >
                <p class="text-body font-medium text-fg">
                  {patient.first_name}
                  {patient.last_name}
                </p>
                <p class="text-caption text-fg-muted mt-1">
                  {formatDate(patient.date_of_birth)}
                </p>
              </button>
            {/each}
          </div>
        {/if}

        <div class="flex gap-2 mt-4">
          <button
            onclick={() => goto('/patients/new')}
            class="flex-1 h-8 px-3 text-body font-medium text-on-success bg-success hover:bg-success-hover rounded-control transition-colors flex items-center justify-center gap-2"
          >
            <Plus size={16} />
            {$t('dashboard.newPatient')}
          </button>
          <button
            onclick={() => goto('/patients')}
            class="flex-1 h-8 px-3 text-body font-medium text-success-fg hover:bg-success-subtle rounded-control transition-colors"
          >
            {$t('dashboard.viewAllPatients')}
          </button>
        </div>
      </div>

      <!-- Sessions with Incomplete Notes -->
      <div class="bg-surface-raised border border-line rounded-card p-6">
        <div class="flex items-center gap-3 mb-4">
          <div class="p-2 bg-warning-subtle rounded-card">
            <FileText size={20} class="text-warning-fg " />
          </div>
          <h2 class="text-heading font-semibold text-fg">{$t('dashboard.incompleteNotes')}</h2>
        </div>

        {#if data.sessions_with_incomplete_notes.length === 0}
          <p class="text-body text-fg-muted">{$t('dashboard.noIncompleteNotes')}</p>
        {:else}
          <div class="space-y-3 max-h-96 overflow-y-auto">
            {#each data.sessions_with_incomplete_notes as item}
              <button
                onclick={() =>
                  goto(`/patients/${item.session.patient_id}/sessions/${item.session.id}`)}
                class="w-full text-left bg-surface-sunken hover:bg-surface-hover rounded-control p-3 transition-colors"
              >
                <p class="text-body font-medium text-fg truncate">{item.patient_name}</p>
                <div class="flex items-center gap-2 mt-1">
                  <span class="text-caption text-fg-muted">
                    {formatDate(item.session.session_date)}
                  </span>
                  <span
                    class="text-caption px-2 py-0.5 rounded-full bg-warning-subtle text-warning-fg"
                  >
                    {getSessionTypeLabel(item.session.session_type)}
                  </span>
                </div>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>
