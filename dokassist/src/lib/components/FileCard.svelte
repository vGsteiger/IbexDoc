<script lang="ts">
  import type { FileRecord } from '$lib/api';
  import { ImageIcon, FileText, FileType, Paperclip } from 'lucide-svelte';
  import type { Component } from 'svelte';
  import { t } from '$lib/translations';

  interface Props {
    file: FileRecord;
    onView?: (file: FileRecord) => void;
    onDownload?: (file: FileRecord) => void;
    onDelete?: (file: FileRecord) => void;
  }

  let { file, onView, onDownload, onDelete }: Props = $props();

  function formatFileSize(bytes: number): string {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
    return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
  }

  function formatDate(dateString: string): string {
    let normalized = dateString.replace(' ', 'T');
    if (!normalized.endsWith('Z')) {
      normalized += 'Z';
    }

    const date = new Date(normalized);
    if (isNaN(date.getTime())) {
      return dateString;
    }

    return (
      date.toLocaleDateString() +
      ' ' +
      date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
    );
  }

  function getFileIcon(mimeType: string): Component<Record<string, unknown>> {
    if (mimeType.startsWith('image/'))
      return ImageIcon as unknown as Component<Record<string, unknown>>;
    if (mimeType === 'application/pdf')
      return FileText as unknown as Component<Record<string, unknown>>;
    if (mimeType.includes('word')) return FileType as unknown as Component<Record<string, unknown>>;
    return Paperclip as unknown as Component<Record<string, unknown>>;
  }

  function getFileExtension(filename: string): string {
    const parts = filename.split('.');
    return parts.length > 1 ? parts[parts.length - 1].toUpperCase() : '';
  }

  let Icon = $derived(getFileIcon(file.mime_type));
</script>

<div
  class="bg-surface-raised border border-line-subtle rounded-card p-4 hover:border-line-strong transition-colors"
>
  <div class="flex items-start gap-4">
    <div class="flex-shrink-0">
      <Icon size={32} class="text-fg-muted" />
    </div>

    <div class="flex-1 min-w-0">
      <div class="flex items-start justify-between gap-2">
        <div class="flex-1 min-w-0">
          <h3 class="text-fg font-medium truncate" title={file.filename}>
            {file.filename}
          </h3>
          <div class="flex items-center gap-3 mt-1 text-body text-fg-muted">
            <span>{formatFileSize(file.size_bytes)}</span>
            <span>•</span>
            <span>{getFileExtension(file.filename)}</span>
            <span>•</span>
            <span>{formatDate(file.created_at)}</span>
          </div>
        </div>
      </div>

      <div class="flex items-center gap-2 mt-3">
        {#if onView}
          <button
            onclick={() => onView?.(file)}
            class="h-7 px-2.5 bg-accent hover:bg-accent-hover text-on-accent text-body rounded-control transition-colors"
          >
            {$t('files.view')}
          </button>
        {/if}

        {#if onDownload}
          <button
            onclick={() => onDownload?.(file)}
            class="h-7 px-2.5 bg-surface-hover hover:bg-surface-hover text-fg-muted text-body rounded-control transition-colors"
          >
            {$t('files.download')}
          </button>
        {/if}

        {#if onDelete}
          <button
            onclick={() => onDelete?.(file)}
            class="h-7 px-2.5 bg-danger-subtle hover:bg-danger-subtle text-danger-fg text-body rounded-control transition-colors ml-auto"
          >
            {$t('common.delete')}
          </button>
        {/if}
      </div>
    </div>
  </div>
</div>
