<script lang="ts">
  import type { Session } from '$lib/api';

  interface Props {
    session: Session;
    onclick?: () => void;
  }

  let { session, onclick }: Props = $props();

  function formatDate(dateStr: string): string {
    try {
      const date = new Date(dateStr);
      return date.toLocaleDateString('de-CH', {
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
      });
    } catch {
      return dateStr;
    }
  }

  function getSnippet(notes: string | null): string {
    if (!notes) return 'Keine Notizen';
    return notes.length > 100 ? notes.substring(0, 100) + '...' : notes;
  }
</script>

<button
  type="button"
  class="w-full text-left p-4 bg-surface-raised rounded-control border border-line hover:border-accent hover:bg-surface-hover transition-colors"
  {onclick}
>
  <div class="flex justify-between items-start mb-2">
    <div class="flex-1">
      <h3 class="text-heading font-semibold text-fg">{session.session_type}</h3>
      <p class="text-body text-fg-muted">{formatDate(session.session_date)}</p>
    </div>
    {#if session.duration_minutes}
      <span class="text-body text-fg-muted">{session.duration_minutes} Min.</span>
    {/if}
  </div>
  <p class="text-body text-fg-muted line-clamp-2">{getSnippet(session.notes)}</p>
</button>
