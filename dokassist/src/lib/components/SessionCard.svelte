<script lang="ts">
  import { sessionTypeLabel } from '$lib/translations/labels';
  import { t } from '$lib/translations';
  import type { Session } from '$lib/api';
  import { Card } from '$lib/components/ui';

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
    if (!notes) return $t('sessions.noNotes');
    return notes.length > 100 ? notes.substring(0, 100) + '...' : notes;
  }
</script>

<Card padding="sm" {onclick}>
  <div class="flex items-start justify-between gap-4">
    <div class="min-w-0 flex-1">
      <h3 class="truncate text-heading text-fg">{$sessionTypeLabel(session.session_type)}</h3>
      <p class="mt-0.5 text-caption text-fg-muted" data-numeric>
        {formatDate(session.session_date)}
      </p>
    </div>
    {#if session.duration_minutes}
      <span class="shrink-0 text-caption text-fg-subtle" data-numeric>
        {session.duration_minutes} Min.
      </span>
    {/if}
  </div>
  <p class="mt-1.5 line-clamp-2 text-body text-fg-muted">{getSnippet(session.notes)}</p>
</Card>
