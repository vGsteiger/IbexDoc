<script lang="ts">
  import { t } from '$lib/translations';
  import { untrack } from 'svelte';
  import type { CreateOutcomeScore, UpdateOutcomeScore, OutcomeScore } from '$lib/api';

  interface Props {
    outcomeScore?: OutcomeScore;
    sessionId?: string;
    onSave: (input: CreateOutcomeScore | { id: string; update: UpdateOutcomeScore }) => void;
    onCancel: () => void;
  }

  let { outcomeScore, sessionId, onSave, onCancel }: Props = $props();

  // The form intentionally snapshots the score it was opened with; a different
  // score is edited by remounting the form, not by swapping the prop.
  const initial = untrack(() => ({
    scaleType: outcomeScore?.scale_type || 'PHQ-9',
    score: outcomeScore?.score?.toString() || '',
    administeredAt: outcomeScore?.administered_at || new Date().toISOString().split('T')[0],
    notes: outcomeScore?.notes || '',
  }));

  let scaleType = $state(initial.scaleType);
  let score = $state(initial.score);
  let administeredAt = $state(initial.administeredAt);
  let notes = $state(initial.notes);

  const scaleOptions = [
    { value: 'PHQ-9', label: 'PHQ-9 (Depression)', max: 27 },
    { value: 'GAD-7', label: 'GAD-7 (Anxiety)', max: 21 },
    { value: 'BDI-II', label: 'BDI-II (Depression)', max: 63 },
  ];

  let maxScore = $derived(scaleOptions.find((s) => s.value === scaleType)?.max || 27);

  function handleSubmit(event: Event) {
    event.preventDefault();

    const scoreValue = parseInt(score, 10);
    if (isNaN(scoreValue) || scoreValue < 0 || scoreValue > maxScore) {
      return;
    }

    if (outcomeScore) {
      // Update existing score
      const update: UpdateOutcomeScore = {
        scale_type: scaleType !== outcomeScore.scale_type ? scaleType : undefined,
        score: scoreValue !== outcomeScore.score ? scoreValue : undefined,
        administered_at:
          administeredAt !== outcomeScore.administered_at ? administeredAt : undefined,
        notes: notes !== (outcomeScore.notes || '') ? notes || undefined : undefined,
      };
      onSave({ id: outcomeScore.id, update });
    } else if (sessionId) {
      // Create new score
      const input: CreateOutcomeScore = {
        session_id: sessionId,
        scale_type: scaleType,
        score: scoreValue,
        administered_at: administeredAt,
        notes: notes || undefined,
      };
      onSave(input);
    }
  }
</script>

<form onsubmit={handleSubmit} class="space-y-4">
  <div>
    <label for="scale-type" class="block text-body font-medium text-fg mb-1"> Fragebogen * </label>
    <select
      id="scale-type"
      bind:value={scaleType}
      required
      class="w-full px-3 py-2 bg-surface-raised border border-line rounded-control text-fg focus:outline-none focus:ring-2 focus:ring-accent/30"
    >
      {#each scaleOptions as option}
        <option value={option.value}>{option.label}</option>
      {/each}
    </select>
  </div>

  <div>
    <label for="score" class="block text-body font-medium text-fg mb-1">
      Gesamtpunktzahl * (0-{maxScore})
    </label>
    <input
      id="score"
      type="number"
      bind:value={score}
      required
      min="0"
      max={maxScore}
      placeholder="z.B. 12"
      class="w-full px-3 py-2 bg-surface-raised border border-line rounded-control text-fg focus:outline-none focus:ring-2 focus:ring-accent/30"
    />
  </div>

  <div>
    <label for="administered-at" class="block text-body font-medium text-fg mb-1">
      {$t('outcomeScores.administeredOnLabel')} *
    </label>
    <input
      id="administered-at"
      type="date"
      bind:value={administeredAt}
      required
      class="w-full px-3 py-2 bg-surface-raised border border-line rounded-control text-fg focus:outline-none focus:ring-2 focus:ring-accent/30"
    />
  </div>

  <div>
    <label for="notes" class="block text-body font-medium text-fg mb-1">
      {$t('common.notes')}
    </label>
    <textarea
      id="notes"
      bind:value={notes}
      rows="3"
      placeholder={$t('outcomeScores.notesPlaceholder')}
      class="w-full px-3 py-2 bg-surface-raised border border-line rounded-control text-fg focus:outline-none focus:ring-2 focus:ring-accent/30 resize-none"
    ></textarea>
  </div>

  <div class="flex justify-end gap-3 pt-4">
    <button
      type="button"
      onclick={onCancel}
      class="h-8 px-3 bg-surface-selected text-fg rounded-control hover:bg-surface-selected transition-colors"
    >
      {$t('common.cancel')}
    </button>
    <button
      type="submit"
      class="h-8 px-3 bg-accent text-on-accent rounded-control hover:bg-accent-hover transition-colors"
    >
      {outcomeScore ? $t('common.update') : $t('common.add')}
    </button>
  </div>
</form>
