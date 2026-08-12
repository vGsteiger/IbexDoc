<script lang="ts">
  import { untrack } from 'svelte';
  import type {
    CreateMedication,
    UpdateMedication,
    Medication,
    SubstanceSummary,
    SubstanceDetail,
  } from '$lib/api';
  import { getMedicationReferenceDetail, searchMedicationReference } from '$lib/api';
  import MedicationAutocomplete from './MedicationAutocomplete.svelte';
  import MedicationInfoPanel from './MedicationInfoPanel.svelte';
  import MedicationChangeAssistant from './MedicationChangeAssistant.svelte';

  interface Props {
    medication?: Medication;
    patientId?: string;
    activeMedications?: Medication[];
    onSave: (
      input: CreateMedication | { id: string; update: UpdateMedication },
      replacingMedicationId?: string | null
    ) => void;
    onCancel: () => void;
  }

  let { medication, patientId, activeMedications = [], onSave, onCancel }: Props = $props();

  let substance = $state(untrack(() => medication?.substance || ''));
  let dosage = $state(untrack(() => medication?.dosage || ''));
  let frequency = $state(untrack(() => medication?.frequency || ''));
  let startDate = $state(
    untrack(() => medication?.start_date || new Date().toISOString().split('T')[0])
  );
  let endDate = $state(untrack(() => medication?.end_date || ''));
  let notes = $state(untrack(() => medication?.notes || ''));
  let selectedSubstanceId = $state<string | null>(null);
  let selectedSubstanceDetail = $state<SubstanceDetail | null>(null);

  // Replacement medication state
  let isReplacement = $state(false);
  let replacingMedicationId = $state<string | null>(null);
  let replacingMedication = $state<Medication | null>(null);
  let replacingSubstanceDetail = $state<SubstanceDetail | null>(null);
  let showComparisonAssistant = $state(false);

  $effect(() => {
    if (medication) {
      substance = medication.substance || '';
      dosage = medication.dosage || '';
      frequency = medication.frequency || '';
      startDate = medication.start_date || new Date().toISOString().split('T')[0];
      endDate = medication.end_date || '';
      notes = medication.notes || '';
      // Clear any reference panel when editing an existing record
      selectedSubstanceId = null;
      selectedSubstanceDetail = null;
      isReplacement = false;
      replacingMedicationId = null;
    }
  });

  // Load substance detail when selected
  $effect(() => {
    if (selectedSubstanceId) {
      const currentId = selectedSubstanceId;
      getMedicationReferenceDetail(currentId)
        .then((detail) => {
          if (selectedSubstanceId === currentId) {
            selectedSubstanceDetail = detail;
          }
        })
        .catch((err) => {
          console.error('Failed to load substance detail:', err);
          if (selectedSubstanceId === currentId) {
            selectedSubstanceDetail = null;
          }
        });
    } else {
      selectedSubstanceDetail = null;
    }
  });

  // Load replacing medication substance detail
  $effect(() => {
    if (replacingMedicationId) {
      const currentReplacingId = replacingMedicationId;
      const med = activeMedications.find((m) => m.id === currentReplacingId);
      replacingMedication = med ?? null;
      replacingSubstanceDetail = null;
      showComparisonAssistant = false;
      if (med) {
        searchMedicationReference(med.substance)
          .then((results) => {
            if (results.length > 0) {
              return getMedicationReferenceDetail(results[0].id);
            }
            return null;
          })
          .then((detail) => {
            if (replacingMedicationId === currentReplacingId) {
              replacingSubstanceDetail = detail;
            }
          })
          .catch((err) => {
            console.error('Failed to load replacing substance detail:', err);
          });
      }
    } else {
      replacingMedication = null;
      replacingSubstanceDetail = null;
    }
  });

  function handleSubstanceSelect(summary: SubstanceSummary) {
    substance = summary.name_de;
    selectedSubstanceId = summary.id;
  }

  function handleReplacementToggle() {
    isReplacement = !isReplacement;
    if (!isReplacement) {
      replacingMedicationId = null;
      showComparisonAssistant = false;
    }
  }

  function handleShowComparison() {
    if (selectedSubstanceDetail && replacingSubstanceDetail) {
      showComparisonAssistant = true;
    }
  }

  function handleSubmit(event: Event) {
    event.preventDefault();

    if (!substance.trim() || !dosage.trim() || !frequency.trim()) {
      return;
    }

    if (medication) {
      const update: UpdateMedication = {
        substance: substance !== medication.substance ? substance : undefined,
        dosage: dosage !== medication.dosage ? dosage : undefined,
        frequency: frequency !== medication.frequency ? frequency : undefined,
        start_date: startDate !== medication.start_date ? startDate : undefined,
        end_date: endDate !== (medication.end_date || '') ? endDate || undefined : undefined,
        notes: notes !== (medication.notes || '') ? notes || undefined : undefined,
      };
      onSave({ id: medication.id, update });
    } else if (patientId) {
      const input: CreateMedication = {
        patient_id: patientId,
        substance,
        dosage,
        frequency,
        start_date: startDate,
        end_date: endDate || undefined,
        notes: notes || undefined,
      };
      onSave(input, isReplacement ? replacingMedicationId : null);
    }
  }
</script>

<form onsubmit={handleSubmit} class="space-y-4">
  <!-- Replacement Option -->
  {#if !medication && activeMedications.length > 0}
    <div class="bg-surface-sunken border border-line rounded-card p-4">
      <label class="flex items-center gap-2 cursor-pointer">
        <input
          type="checkbox"
          checked={isReplacement}
          onchange={handleReplacementToggle}
          class="w-4 h-4 text-accent-fg border-line rounded-control focus:ring-accent/30"
        />
        <span class="text-body font-medium text-fg-muted">
          Dieses Medikament ersetzt ein bestehendes Medikament
        </span>
      </label>

      {#if isReplacement}
        <div class="mt-3">
          <label for="replacing-medication" class="block text-body font-medium text-fg-muted mb-1">
            Zu ersetzendes Medikament *
          </label>
          <select
            id="replacing-medication"
            bind:value={replacingMedicationId}
            required={isReplacement}
            class="w-full px-3 py-2 bg-surface-raised border border-line rounded-control text-fg focus:outline-none focus:ring-2 focus:ring-accent/30"
          >
            <option value={null}>-- Bitte wählen --</option>
            {#each activeMedications as med (med.id)}
              <option value={med.id}>
                {med.substance} ({med.dosage}, {med.frequency})
              </option>
            {/each}
          </select>

          {#if replacingMedicationId && selectedSubstanceDetail && replacingSubstanceDetail && patientId}
            <button
              type="button"
              onclick={handleShowComparison}
              class="mt-2 w-full h-8 px-3 bg-accent text-on-accent rounded-control hover:bg-accent-hover transition-colors text-body"
            >
              🤖 Medikamentenvergleich & KI-Entscheidungshilfe anzeigen
            </button>
          {/if}
        </div>
      {/if}
    </div>
  {/if}

  <!-- Show Comparison Assistant -->
  {#if showComparisonAssistant && selectedSubstanceDetail && replacingSubstanceDetail && patientId}
    <div class="border-t border-line pt-4">
      <MedicationChangeAssistant
        {patientId}
        currentSubstance={replacingSubstanceDetail}
        replacementSubstance={selectedSubstanceDetail}
      />
    </div>
  {/if}

  <div>
    <label for="substance" class="block text-body font-medium text-fg-muted mb-1">
      Wirkstoff *
    </label>
    <MedicationAutocomplete
      id="substance"
      value={substance}
      onInput={(v) => {
        substance = v;
        selectedSubstanceId = null;
      }}
      onSelect={handleSubstanceSelect}
      required
      placeholder="z.B. Sertralin"
    />
    <MedicationInfoPanel substanceId={selectedSubstanceId} />
  </div>

  <div>
    <label for="dosage" class="block text-body font-medium text-fg-muted mb-1"> Dosierung * </label>
    <input
      id="dosage"
      type="text"
      bind:value={dosage}
      required
      placeholder="z.B. 50 mg"
      class="w-full px-3 py-2 bg-surface-raised border border-line rounded-control text-fg focus:outline-none focus:ring-2 focus:ring-accent/30"
    />
  </div>

  <div>
    <label for="frequency" class="block text-body font-medium text-fg-muted mb-1">
      Häufigkeit *
    </label>
    <input
      id="frequency"
      type="text"
      bind:value={frequency}
      required
      placeholder="z.B. 1x täglich"
      class="w-full px-3 py-2 bg-surface-raised border border-line rounded-control text-fg focus:outline-none focus:ring-2 focus:ring-accent/30"
    />
  </div>

  <div class="grid grid-cols-2 gap-4">
    <div>
      <label for="start-date" class="block text-body font-medium text-fg-muted mb-1">
        Startdatum *
      </label>
      <input
        id="start-date"
        type="date"
        bind:value={startDate}
        required
        class="w-full px-3 py-2 bg-surface-raised border border-line rounded-control text-fg focus:outline-none focus:ring-2 focus:ring-accent/30"
      />
    </div>

    <div>
      <label for="end-date" class="block text-body font-medium text-fg-muted mb-1">
        Enddatum
      </label>
      <input
        id="end-date"
        type="date"
        bind:value={endDate}
        class="w-full px-3 py-2 bg-surface-raised border border-line rounded-control text-fg focus:outline-none focus:ring-2 focus:ring-accent/30"
      />
    </div>
  </div>

  <div>
    <label for="notes" class="block text-body font-medium text-fg-muted mb-1"> Notizen </label>
    <textarea
      id="notes"
      bind:value={notes}
      rows="3"
      placeholder="Zusätzliche Informationen..."
      class="w-full px-3 py-2 bg-surface-raised border border-line rounded-control text-fg focus:outline-none focus:ring-2 focus:ring-accent/30 resize-none"
    ></textarea>
  </div>

  <div class="flex justify-end gap-3 pt-4">
    <button
      type="button"
      onclick={onCancel}
      class="h-8 px-3 bg-surface-hover text-fg-muted rounded-control hover:bg-surface-selected transition-colors"
    >
      Abbrechen
    </button>
    <button
      type="submit"
      class="h-8 px-3 bg-accent text-on-accent rounded-control hover:bg-accent-hover transition-colors"
    >
      {medication ? 'Aktualisieren' : 'Hinzufügen'}
    </button>
  </div>
</form>
