<script lang="ts">
  import { page } from '$app/stores';
  import { onMount } from 'svelte';
  import { listEmails, deleteEmail, parseError, type Email, type AppError } from '$lib/api';
  import ErrorDisplay from '$lib/components/ErrorDisplay.svelte';

  $: patientId = $page.params.id;
  let emails: Email[] = [];
  let loading = true;
  let error: AppError | null = null;

  async function loadEmails() {
    try {
      loading = true;
      error = null;
      emails = await listEmails(patientId);
    } catch (e) {
      error = parseError(e);
    } finally {
      loading = false;
    }
  }

  async function handleDeleteEmail(emailId: string, status: string) {
    const confirmMessage = status === 'draft'
      ? 'Are you sure you want to delete this email draft?'
      : 'Are you sure you want to delete this email?';

    if (!confirm(confirmMessage)) {
      return;
    }
    try {
      await deleteEmail(emailId);
      await loadEmails();
    } catch (e) {
      error = parseError(e);
    }
  }

  function formatDate(dateStr: string): string {
    return new Date(dateStr).toLocaleDateString('de-DE', {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit'
    });
  }

  function formatStatus(status: string): string {
    switch (status) {
      case 'draft':
        return 'Draft';
      case 'sent':
        return 'Sent';
      default:
        return status;
    }
  }

  onMount(() => {
    loadEmails();
  });
</script>

<div class="p-8">
  <div class="flex justify-between items-center mb-6">
    <h2 class="text-2xl font-bold text-gray-100">Emails</h2>
    <a
      href={`/patients/${patientId}/email/new`}
      class="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 transition-colors"
    >
      Compose New Email
    </a>
  </div>

  {#if loading}
    <div class="text-gray-400">Loading emails...</div>
  {:else if error}
    <ErrorDisplay {error} showDetails={true} />
  {:else if emails.length === 0}
    <div class="text-center py-12">
      <p class="text-gray-400 mb-4">No emails yet.</p>
      <a
        href={`/patients/${patientId}/email/new`}
        class="inline-block px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 transition-colors"
      >
        Compose Your First Email
      </a>
    </div>
  {:else}
    <div class="space-y-4">
      {#each emails as email}
        <div class="bg-gray-800 rounded-lg p-6 border border-gray-700">
          <div class="flex justify-between items-start mb-3">
            <div class="flex-1">
              <div class="flex items-center gap-3 mb-2">
                <h3 class="text-lg font-semibold text-gray-100">
                  {email.subject}
                </h3>
                <span class="px-2 py-1 text-xs rounded {email.status === 'sent' ? 'bg-green-900/30 text-green-400' : 'bg-yellow-900/30 text-yellow-400'}">
                  {formatStatus(email.status)}
                </span>
              </div>
              <p class="text-sm text-gray-400">
                To: {email.recipient_email}
              </p>
              <p class="text-xs text-gray-500 mt-1">
                {#if email.status === 'sent' && email.sent_at}
                  Sent: {formatDate(email.sent_at)}
                {:else}
                  Created: {formatDate(email.created_at)}
                {/if}
              </p>
            </div>
            <div class="flex space-x-2">
              <a
                href={`/patients/${patientId}/email/${email.id}`}
                class="px-3 py-1 text-sm bg-gray-700 text-gray-300 rounded hover:bg-gray-600 transition-colors"
              >
                {email.status === 'draft' ? 'Edit' : 'View'}
              </a>
              {#if email.status === 'draft'}
                <button
                  on:click={() => handleDeleteEmail(email.id, email.status)}
                  class="px-3 py-1 text-sm bg-red-900/20 text-red-400 rounded hover:bg-red-900/40 transition-colors"
                >
                  Delete
                </button>
              {/if}
            </div>
          </div>
          <div class="text-sm text-gray-400 line-clamp-3">
            {email.body.substring(0, 300)}{email.body.length > 300 ? '...' : ''}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
