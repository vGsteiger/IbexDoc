<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import {
    getEngineStatus,
    loadModel,
    globalSearch,
    parseError,
    type LlmEngineStatus,
    type SearchResult,
  } from '$lib/api';
  import { t } from '$lib/translations';
  import { Search } from 'lucide-svelte';

  let searchInput = $state<HTMLInputElement | null>(null);
  let engineStatus = $state<LlmEngineStatus | null>(null);
  let isLoadingModel = $state(false);
  let searchQuery = $state('');
  let searchResults = $state<SearchResult[]>([]);
  let showDropdown = $state(false);
  let isSearching = $state(false);
  let searchTimeout: ReturnType<typeof setTimeout> | null = null;

  let isLoaded = $derived(engineStatus?.is_loaded ?? false);
  let isDownloaded = $derived(engineStatus?.is_downloaded ?? false);

  onMount(() => {
    const handleKeydown = (e: KeyboardEvent) => {
      // Cmd+K is now handled globally for command palette
      // Only handle Escape here
      if (e.key === 'Escape') {
        closeDropdown();
      }
    };

    window.addEventListener('keydown', handleKeydown);
    updateLlmStatus();
    const interval = setInterval(updateLlmStatus, 5000);

    return () => {
      window.removeEventListener('keydown', handleKeydown);
      clearInterval(interval);
    };
  });

  async function updateLlmStatus() {
    try {
      engineStatus = await getEngineStatus();
    } catch (error) {
      console.error('Failed to get LLM status:', error);
    }
  }

  async function handleDotClick() {
    if (isLoaded || isLoadingModel) return;
    if (isDownloaded && engineStatus?.downloaded_filename) {
      isLoadingModel = true;
      try {
        await loadModel(engineStatus.downloaded_filename);
        engineStatus = await getEngineStatus();
      } catch (e) {
        console.error('Failed to load model:', parseError(e).message);
      } finally {
        isLoadingModel = false;
      }
    } else {
      goto('/settings');
    }
  }

  function handleSearch(e: Event) {
    const query = (e.target as HTMLInputElement).value;
    searchQuery = query;

    if (searchTimeout) clearTimeout(searchTimeout);

    if (!query.trim()) {
      searchResults = [];
      showDropdown = false;
      return;
    }

    searchTimeout = setTimeout(async () => {
      isSearching = true;
      try {
        searchResults = await globalSearch(query, 20);
        showDropdown = true;
      } catch (err) {
        console.error('Search error:', err);
        searchResults = [];
      } finally {
        isSearching = false;
      }
    }, 300);
  }

  function handleBlur() {
    setTimeout(() => {
      showDropdown = false;
    }, 150);
  }

  function closeDropdown() {
    showDropdown = false;
    searchQuery = '';
    searchResults = [];
    searchInput?.blur();
  }

  function navigateTo(result: SearchResult) {
    closeDropdown();
    switch (result.result_type) {
      case 'patient':
        goto(`/patients/${result.entity_id}`);
        break;
      case 'file':
        goto(`/patients/${result.patient_id}/files`);
        break;
      case 'session':
        goto(`/patients/${result.patient_id}/sessions`);
        break;
      case 'diagnosis':
        goto(`/patients/${result.patient_id}/diagnoses`);
        break;
      case 'medication':
        goto(`/patients/${result.patient_id}/medications`);
        break;
      case 'report':
        goto(`/patients/${result.patient_id}/reports/${result.entity_id}`);
        break;
      default:
        goto(`/patients/${result.patient_id}`);
    }
  }

  let typeLabel = $derived<Record<string, string>>({
    patient: $t('topbar.typePatient'),
    file: $t('topbar.typeFile'),
    session: $t('topbar.typeSession'),
    diagnosis: $t('topbar.typeDiagnosis'),
    medication: $t('topbar.typeMedication'),
    report: $t('topbar.typeReport'),
  });
</script>

<!-- The bar sits on the page surface with a single hairline under it; the old
     sunken slab read as a second chrome layer above the content. -->
<header class="flex h-14 shrink-0 items-center gap-3 border-b border-line-subtle px-4">
  <div class="relative w-full max-w-md">
    <Search
      size={14}
      class="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-fg-subtle"
      aria-hidden="true"
    />
    <input
      bind:this={searchInput}
      type="search"
      placeholder={$t('topbar.searchPlaceholder')}
      class="h-8 w-full rounded-control border border-line bg-surface-raised pl-8 pr-2.5 text-body text-fg transition-colors duration-150 ease-standard focus:border-accent focus:outline-none focus:ring-2 focus:ring-accent/25"
      value={searchQuery}
      oninput={handleSearch}
      onblur={handleBlur}
    />

    {#if showDropdown && searchQuery.trim()}
      <div
        class="absolute left-0 right-0 top-full z-50 mt-1 max-h-96 overflow-y-auto rounded-card border border-line bg-surface-overlay py-1 shadow-popover"
      >
        {#if isSearching}
          <div class="px-3 py-2 text-body text-fg-muted">
            {$t('topbar.searching')}
          </div>
        {:else if searchResults.length === 0}
          <div class="px-3 py-2 text-body text-fg-muted">
            {$t('topbar.noResults').replace('{query}', searchQuery)}
          </div>
        {:else}
          {#each searchResults as result (result.entity_id)}
            <button
              class="w-full px-3 py-2 text-left transition-colors duration-150 ease-standard hover:bg-surface-hover"
              onclick={() => navigateTo(result)}
            >
              <div class="flex items-baseline gap-2">
                <span class="truncate text-body text-fg">{result.title}</span>
                <span class="ml-auto shrink-0 text-caption text-fg-subtle">
                  {typeLabel[result.result_type] ?? result.result_type}
                </span>
              </div>
              <div class="mt-0.5 flex items-baseline gap-2 text-caption text-fg-muted">
                <span class="shrink-0">{result.patient_name}</span>
                {#if result.snippet}
                  <span class="line-clamp-1 text-fg-subtle">{result.snippet}</span>
                {/if}
              </div>
            </button>
          {/each}
        {/if}
      </div>
    {/if}
  </div>

  <div class="ml-auto flex items-center gap-2">
    <span class="text-caption text-fg-subtle">LLM</span>
    <button
      onclick={handleDotClick}
      disabled={isLoaded || isLoadingModel}
      class="h-2 w-2 rounded-full transition-colors duration-150 ease-standard {isLoadingModel
        ? 'animate-pulse cursor-wait bg-warning'
        : isLoaded
          ? 'cursor-default bg-success'
          : isDownloaded
            ? 'cursor-pointer bg-warning'
            : 'cursor-pointer bg-danger'}"
      aria-label={isLoadingModel
        ? $t('topbar.loadingModel')
        : isLoaded
          ? $t('topbar.modelLoaded')
          : isDownloaded
            ? $t('topbar.modelDownloaded')
            : $t('topbar.noModelDownloaded')}
      title={isLoadingModel
        ? $t('topbar.loadingModel')
        : isLoaded
          ? $t('topbar.modelLoaded')
          : isDownloaded
            ? $t('topbar.modelDownloaded')
            : $t('topbar.noModelDownloaded')}
    ></button>
  </div>
</header>
