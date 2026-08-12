<script lang="ts">
  import { onDestroy } from 'svelte';
  import { listen, type UnlistenFn } from '@tauri-apps/api/event';
  import { getOrCreatePatientChatSession, runAgentTurn, type SubstanceDetail } from '$lib/api';
  import MedicationComparisonPanel from './MedicationComparisonPanel.svelte';

  interface Props {
    patientId: string;
    currentSubstance: SubstanceDetail;
    replacementSubstance: SubstanceDetail;
  }

  let { patientId, currentSubstance, replacementSubstance }: Props = $props();

  let aiGuidance = $state('');
  let isGenerating = $state(false);
  let error = $state<string | null>(null);
  let sessionId = $state<string | null>(null);

  // Track active listeners so they can be cleaned up on unmount.
  let activeUnlisteners: UnlistenFn[] = [];

  function cleanupListeners() {
    activeUnlisteners.forEach((fn) => fn());
    activeUnlisteners = [];
  }

  onDestroy(cleanupListeners);

  async function generateGuidance() {
    try {
      isGenerating = true;
      error = null;
      aiGuidance = '';

      // Clean up any listeners from a previous run.
      cleanupListeners();

      // Create or get the chat session for this patient
      const session = await getOrCreatePatientChatSession(patientId);
      sessionId = session.id;

      // Set up event listeners for streaming
      const chunkUnlisten = await listen<string>('agent-chunk', (event) => {
        aiGuidance += event.payload;
      });

      const doneUnlisten = await listen<void>('agent-done', () => {
        isGenerating = false;
        cleanupListeners();
      });

      activeUnlisteners = [chunkUnlisten, doneUnlisten];

      // Construct a prompt that asks the agent to compare the medications
      const prompt = `Bitte vergleiche die folgenden beiden Medikamente und gib eine Entscheidungshilfe für den Medikamentenwechsel:

Aktuelles Medikament: ${currentSubstance.name_de} (ID: ${currentSubstance.id})
Neues Medikament: ${replacementSubstance.name_de} (ID: ${replacementSubstance.id})

Nutze das compare_medications Tool, um detaillierte Informationen zu beiden Medikamenten abzurufen, und erstelle dann eine Zusammenfassung mit folgenden Punkten:

1. Gemeinsamkeiten und Unterschiede in der Indikation
2. Vergleich der Nebenwirkungen (überlappend und unterschiedlich)
3. Kontraindikationen, die beachtet werden müssen
4. Empfehlung für den Medikamentenwechsel
5. Wichtige Punkte für das Monitoring nach dem Wechsel`;

      // Send the message to the agent
      await runAgentTurn(sessionId, prompt);
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
      isGenerating = false;
      cleanupListeners();
    }
  }
</script>

<div class="space-y-6">
  <!-- Comparison Panel -->
  <MedicationComparisonPanel current={currentSubstance} replacement={replacementSubstance} />

  <!-- AI Guidance Section -->
  <div class="bg-surface-raised border border-line rounded-card p-6">
    <div class="flex items-center justify-between mb-4">
      <h3 class="text-heading font-semibold text-fg">KI-gestützte Entscheidungshilfe</h3>
      <button
        onclick={generateGuidance}
        disabled={isGenerating}
        class="h-8 px-3 bg-accent text-on-accent rounded-control hover:bg-accent-hover transition-colors disabled:bg-surface-selected disabled:cursor-not-allowed"
      >
        {isGenerating ? 'Generiert...' : 'Entscheidungshilfe generieren'}
      </button>
    </div>

    {#if error}
      <div class="mb-4 p-3 bg-danger-subtle border border-danger-line rounded-card">
        <p class="text-body text-danger-fg">{error}</p>
      </div>
    {/if}

    {#if aiGuidance || isGenerating}
      <div class="prose dark:prose-invert max-w-none">
        <div class="whitespace-pre-wrap text-body text-fg-muted bg-surface-sunken rounded-card p-4">
          {aiGuidance || 'Generiere Entscheidungshilfe...'}
        </div>
      </div>
    {:else}
      <p class="text-body text-fg-muted">
        Klicken Sie auf "Entscheidungshilfe generieren", um eine KI-gestützte Analyse des
        Medikamentenwechsels zu erhalten. Die KI wird die Kompendiumdaten beider Medikamente
        analysieren und eine fundierte Empfehlung geben.
      </p>
    {/if}
  </div>
</div>
