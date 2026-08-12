<script lang="ts">
  import { downloadFile, type FileRecord } from '$lib/api';
  import { onDestroy } from 'svelte';
  import { Hourglass, FileText } from 'lucide-svelte';

  interface Props {
    file: FileRecord | null;
    onClose?: () => void;
  }

  let { file, onClose }: Props = $props();

  let blobUrl = $state<string | null>(null);
  let isLoading = $state(false);
  let errorMessage = $state('');

  async function loadFile() {
    if (!file) return;

    try {
      isLoading = true;
      errorMessage = '';

      const data = await downloadFile(file.id);
      const blob = new Blob([new Uint8Array(data)], { type: file.mime_type });

      // Revoke previous blob URL after the await so blobUrl is not read
      // synchronously inside $effect (which would make it a tracked dependency
      // and cause an infinite reload loop in Svelte 5).
      if (blobUrl) {
        URL.revokeObjectURL(blobUrl);
      }
      blobUrl = URL.createObjectURL(blob);
    } catch (error) {
      console.error('Failed to load file:', error);
      errorMessage = `Failed to load file: ${error}`;
    } finally {
      isLoading = false;
    }
  }

  function handleDownload() {
    if (!blobUrl || !file) return;

    const a = document.createElement('a');
    a.href = blobUrl;
    a.download = file.filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
  }

  function handleClose() {
    if (blobUrl) {
      URL.revokeObjectURL(blobUrl);
      blobUrl = null;
    }
    onClose?.();
  }

  onDestroy(() => {
    if (blobUrl) {
      URL.revokeObjectURL(blobUrl);
    }
  });

  $effect(() => {
    if (file) {
      loadFile();
    }
  });
</script>

{#if file}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/80"
    role="presentation"
    onclick={handleClose}
    onkeydown={(e) => e.key === 'Escape' && handleClose()}
  >
    <div
      class="relative w-full h-full max-w-6xl max-h-[90vh] m-4"
      role="dialog"
      aria-modal="true"
      aria-label={file.filename}
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <div
        class="absolute top-0 left-0 right-0 bg-surface-sunken border-b border-line-subtle p-4 flex items-center justify-between rounded-t-lg"
      >
        <div class="flex-1 min-w-0">
          <h2 class="text-fg font-medium truncate" title={file.filename}>
            {file.filename}
          </h2>
        </div>

        <div class="flex items-center gap-2 ml-4">
          <button
            onclick={handleDownload}
            class="h-8 px-3 bg-accent hover:bg-accent-hover text-on-accent rounded-control transition-colors"
            disabled={!blobUrl}
          >
            Download
          </button>

          <button
            onclick={handleClose}
            class="h-8 px-3 bg-surface-selected hover:bg-surface-selected text-fg-muted rounded-control transition-colors"
          >
            Close
          </button>
        </div>
      </div>

      <div
        class="absolute top-16 bottom-0 left-0 right-0 bg-surface-sunken rounded-b-lg overflow-hidden"
      >
        {#if isLoading}
          <div class="flex items-center justify-center h-full">
            <div class="text-center">
              <div class="mb-4 flex justify-center text-fg-muted">
                <Hourglass size={48} />
              </div>
              <p class="text-fg-muted">Loading file...</p>
            </div>
          </div>
        {:else if errorMessage}
          <div class="flex items-center justify-center h-full">
            <div class="bg-danger-subtle border border-danger-line rounded-card p-6 max-w-md">
              <p class="text-danger-fg">{errorMessage}</p>
            </div>
          </div>
        {:else if blobUrl}
          {#if file.mime_type === 'application/pdf'}
            <iframe src={blobUrl} title={file.filename} class="w-full h-full"></iframe>
          {:else if file.mime_type.startsWith('image/')}
            <div class="flex items-center justify-center h-full p-4 overflow-auto">
              <img src={blobUrl} alt={file.filename} class="max-w-full max-h-full object-contain" />
            </div>
          {:else}
            <div class="flex items-center justify-center h-full">
              <div class="text-center">
                <div class="mb-4 flex justify-center text-fg-muted">
                  <FileText size={48} />
                </div>
                <p class="text-fg-muted mb-4">Preview not available for this file type</p>
                <button
                  onclick={handleDownload}
                  class="h-8 px-3 bg-accent hover:bg-accent-hover text-on-accent rounded-control transition-colors"
                >
                  Download to View
                </button>
              </div>
            </div>
          {/if}
        {/if}
      </div>
    </div>
  </div>
{/if}
