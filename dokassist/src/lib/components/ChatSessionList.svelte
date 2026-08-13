<script lang="ts">
  import { t } from '$lib/translations';
  import { renameChatSession, deleteChatSession, type ChatSession } from '$lib/api';
  import { Check, X, Pencil, Trash2 } from 'lucide-svelte';

  interface Props {
    sessions: ChatSession[];
    activeSessionId: string | null;
    onsessionselect: (sessionId: string) => void;
    onsessionnew: () => void;
    onlistchange?: () => void;
  }

  let {
    sessions = $bindable(),
    activeSessionId,
    onsessionselect,
    onsessionnew,
    onlistchange,
  }: Props = $props();

  let renamingId = $state<string | null>(null);
  let renameValue = $state('');
  let renameInputEl = $state<HTMLInputElement | null>(null);

  $effect(() => {
    if (renamingId && renameInputEl) {
      renameInputEl.focus();
    }
  });

  function startRename(session: ChatSession) {
    renamingId = session.id;
    renameValue = session.title;
  }

  async function confirmRename(sessionId: string) {
    if (!renameValue.trim()) return;
    try {
      const updated = await renameChatSession(sessionId, renameValue.trim());
      sessions = sessions.map((s) => (s.id === sessionId ? updated : s));
      renamingId = null;
      onlistchange?.();
    } catch (e) {
      console.error('Rename failed:', e);
    }
  }

  async function handleDelete(sessionId: string) {
    try {
      await deleteChatSession(sessionId);
      sessions = sessions.filter((s) => s.id !== sessionId);
      onlistchange?.();
      if (activeSessionId === sessionId && sessions.length > 0) {
        onsessionselect(sessions[0].id);
      }
    } catch (e) {
      console.error('Delete failed:', e);
    }
  }

  function formatDate(iso: string): string {
    return new Date(iso).toLocaleDateString('de-CH', {
      day: '2-digit',
      month: '2-digit',
      year: 'numeric',
    });
  }
</script>

<div class="flex flex-col h-full">
  <div class="p-3 border-b border-line">
    <button
      onclick={onsessionnew}
      class="w-full flex items-center justify-center gap-2 h-8 px-3 bg-accent hover:bg-accent-hover
 text-on-accent text-body font-medium rounded-control transition-colors"
    >
      <span>+</span>
      <span>{$t('chat.newChat')}</span>
    </button>
  </div>

  <div class="flex-1 overflow-y-auto">
    {#if sessions.length === 0}
      <p class="text-center text-fg-subtle text-body p-4">{$t('chat.noChats')}</p>
    {:else}
      <ul class="py-2">
        {#each sessions as session (session.id)}
          <li
            class="group px-3 py-2 hover:bg-surface-hover transition-colors cursor-pointer
 {activeSessionId === session.id ? 'bg-surface-hover' : ''}"
          >
            {#if renamingId === session.id}
              <div class="flex gap-1">
                <input
                  bind:this={renameInputEl}
                  bind:value={renameValue}
                  onkeydown={(e) => {
                    if (e.key === 'Enter') confirmRename(session.id);
                    if (e.key === 'Escape') renamingId = null;
                  }}
                  class="flex-1 text-body bg-surface-hover border border-line rounded-control px-2 py-0.5
 text-fg focus:outline-none focus:border-accent"
                />
                <button
                  onclick={() => confirmRename(session.id)}
                  class="text-caption text-success-fg hover:text-success-fg px-1"
                  aria-label={$t('common.confirm')}><Check size={14} /></button
                >
                <button
                  onclick={() => (renamingId = null)}
                  class="text-caption text-fg-muted hover:text-fg-muted px-1"
                  aria-label={$t('common.cancel')}><X size={14} /></button
                >
              </div>
            {:else}
              <div
                class="flex items-start gap-2"
                role="button"
                tabindex="0"
                onclick={() => onsessionselect(session.id)}
                onkeydown={(e) => e.key === 'Enter' && onsessionselect(session.id)}
              >
                <div class="flex-1 min-w-0">
                  <p class="text-body text-fg truncate">{session.title}</p>
                  <p class="text-caption text-fg-subtle">
                    {formatDate(session.updated_at)}
                  </p>
                </div>
                <div class="hidden group-hover:flex items-center gap-1 shrink-0">
                  <button
                    onclick={(e) => {
                      e.stopPropagation();
                      startRename(session);
                    }}
                    class="text-caption text-fg-muted hover:text-fg p-0.5"
                    title={$t('common.rename')}><Pencil size={14} /></button
                  >
                  <button
                    onclick={(e) => {
                      e.stopPropagation();
                      handleDelete(session.id);
                    }}
                    class="text-caption text-fg-muted hover:text-danger-fg p-0.5"
                    title={$t('common.delete')}><Trash2 size={14} /></button
                  >
                </div>
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</div>
