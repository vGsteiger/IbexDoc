<script lang="ts">
  import { reportTypeLabel } from '$lib/translations/labels';
  export let selectedType: string = '';
  import { t } from '$lib/translations';

  interface ReportTypeInfo {
    value: string;
    descriptionKey: string;
  }

  // `value` is the stored form and is matched against the Rust ReportType
  // enum, so it stays as-is; the label comes from the bundle.
  const reportTypes: ReportTypeInfo[] = [
    { value: 'Befundbericht', descriptionKey: 'reports.types.befundbericht' },
    { value: 'Verlaufsbericht', descriptionKey: 'reports.types.verlaufsbericht' },
    { value: 'Ueberweisungsschreiben', descriptionKey: 'reports.types.ueberweisungsschreiben' },
  ];

  function selectType(type: string) {
    selectedType = type;
  }
</script>

<div class="space-y-4">
  <h3 class="text-heading font-semibold text-fg">{$t('reports.selectType')}</h3>
  <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
    {#each reportTypes as type}
      <button
        on:click={() => selectType(type.value)}
        class="p-4 rounded-control border-2 transition-colors text-left {selectedType === type.value
          ? 'border-accent bg-accent-subtle'
          : 'border-line bg-surface-raised hover:border-line-strong'}"
      >
        <div class="font-semibold text-fg mb-2">{$reportTypeLabel(type.value)}</div>
        <div class="text-body text-fg-muted">{$t(type.descriptionKey)}</div>
      </button>
    {/each}
  </div>
</div>
