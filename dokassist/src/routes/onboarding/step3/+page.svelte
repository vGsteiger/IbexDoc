<script lang="ts">
  import { goto } from '$app/navigation';
  import { ChevronRight, ChevronLeft, FileText, Database } from 'lucide-svelte';

  function handleBack() {
    goto('/onboarding/step2');
  }

  function handleImportLater() {
    goto('/onboarding/step4');
  }
</script>

<div class="min-h-screen bg-surface flex items-center justify-center p-8">
  <div class="max-w-3xl w-full">
    <div class="mb-8 text-center">
      <h1 class="text-display font-semibold text-fg mb-2">Import Existing Data</h1>
      <p class="text-fg-muted">
        Optionally import patient data from CSV files or a DokAssist backup. You can also do this
        later from Settings.
      </p>
      <div class="flex items-center justify-center gap-2 mt-4">
        <div class="h-2 w-16 bg-accent rounded-full"></div>
        <div class="h-2 w-16 bg-accent rounded-full"></div>
        <div class="h-2 w-16 bg-accent rounded-full"></div>
        <div class="h-2 w-16 bg-surface-selected rounded-full"></div>
      </div>
    </div>

    <div class="bg-surface-raised border border-line-subtle rounded-card p-8 space-y-6">
      <div class="text-center">
        <div class="inline-block p-4 bg-surface-hover rounded-full mb-4">
          <Database size={48} class="text-fg-muted" />
        </div>
        <h2 class="text-title font-semibold text-fg mb-2">Data Import Options</h2>
        <p class="text-fg-muted">Choose how you'd like to import your data</p>
      </div>

      <div class="space-y-4">
        <button
          onclick={() => goto('/settings')}
          class="w-full bg-surface-hover hover:bg-surface-hover border border-line rounded-control p-6 text-left transition-colors group"
        >
          <div class="flex items-start gap-4">
            <div
              class="flex-shrink-0 w-12 h-12 bg-accent-subtle rounded-card flex items-center justify-center group-hover:bg-accent-subtle/30 transition-colors"
            >
              <FileText size={24} class="text-accent-fg" />
            </div>
            <div class="flex-1">
              <h3 class="text-fg font-semibold mb-1">Import from CSV</h3>
              <p class="text-fg-muted text-body">
                Import patient records from a CSV file with automatic column detection for German,
                French, and English headers.
              </p>
            </div>
            <ChevronRight
              size={20}
              class="text-fg-muted group-hover:text-fg-muted transition-colors"
            />
          </div>
        </button>

        <button
          onclick={() => goto('/settings')}
          class="w-full bg-surface-hover hover:bg-surface-hover border border-line rounded-control p-6 text-left transition-colors group"
        >
          <div class="flex items-start gap-4">
            <div
              class="flex-shrink-0 w-12 h-12 bg-success-subtle rounded-card flex items-center justify-center group-hover:bg-success-subtle/30 transition-colors"
            >
              <Database size={24} class="text-success-fg" />
            </div>
            <div class="flex-1">
              <h3 class="text-fg font-semibold mb-1">Restore from Backup</h3>
              <p class="text-fg-muted text-body">
                Restore a complete DokAssist backup archive including patients, sessions, and
                encrypted files.
              </p>
            </div>
            <ChevronRight
              size={20}
              class="text-fg-muted group-hover:text-fg-muted transition-colors"
            />
          </div>
        </button>
      </div>

      <div class="bg-warning-subtle border border-warning-line rounded-card p-4">
        <p class="text-warning-fg text-body">
          <strong>Note:</strong> You can skip this step and import data later from the Settings page.
          The import features will always be available.
        </p>
      </div>
    </div>

    <div class="flex justify-between items-center mt-8">
      <button
        onclick={handleBack}
        class="px-6 py-3 border border-line bg-surface-raised hover:bg-surface-hover text-fg font-medium rounded-control transition-colors flex items-center gap-2"
      >
        <ChevronLeft size={20} />
        Back
      </button>

      <button
        onclick={handleImportLater}
        class="px-6 py-3 bg-accent hover:bg-accent-hover text-on-accent font-medium rounded-control transition-colors flex items-center gap-2"
      >
        Skip for now
        <ChevronRight size={20} />
      </button>
    </div>
  </div>
</div>
