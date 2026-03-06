<script lang="ts">
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { onMount } from "svelte";
  import {
    getPatient,
    createEmail,
    parseError,
    type CreateEmail,
    type Patient,
    type AppError,
  } from "$lib/api";
  import ErrorDisplay from "$lib/components/ErrorDisplay.svelte";

  $: patientId = $page.params.id;

  let patient: Patient | null = null;
  let recipientEmail = "";
  let subject = "";
  let body = "";
  let error: AppError | null = null;
  let isSaving = false;

  async function loadPatient() {
    try {
      patient = await getPatient(patientId);
      // Pre-fill recipient email if patient has one
      if (patient.email) {
        recipientEmail = patient.email;
      }
    } catch (e) {
      error = parseError(e);
    }
  }

  async function handleSaveDraft() {
    if (!recipientEmail.trim() || !subject.trim() || !body.trim()) {
      error = {
        code: "VALIDATION_ERROR",
        message: "Please fill in all fields",
        ref: "VALIDATION",
      };
      return;
    }

    try {
      isSaving = true;
      error = null;

      const input: CreateEmail = {
        patient_id: patientId,
        recipient_email: recipientEmail,
        subject: subject,
        body: body,
      };

      const email = await createEmail(input);

      // Navigate to the email list
      await goto(`/patients/${patientId}/email`);
    } catch (e) {
      error = parseError(e);
    } finally {
      isSaving = false;
    }
  }

  async function handleSendEmail() {
    if (!recipientEmail.trim() || !subject.trim() || !body.trim()) {
      error = {
        code: "VALIDATION_ERROR",
        message: "Please fill in all fields",
        ref: "VALIDATION",
      };
      return;
    }

    try {
      isSaving = true;
      error = null;

      // Create the email draft first
      const input: CreateEmail = {
        patient_id: patientId,
        recipient_email: recipientEmail,
        subject: subject,
        body: body,
      };

      await createEmail(input);

      // Open the email in the local mail program using mailto: link
      const mailtoLink = `mailto:${encodeURIComponent(recipientEmail)}?subject=${encodeURIComponent(subject)}&body=${encodeURIComponent(body)}`;
      window.location.href = mailtoLink;

      // Navigate back to the email list after opening mail client
      setTimeout(() => {
        goto(`/patients/${patientId}/email`);
      }, 500);
    } catch (e) {
      error = parseError(e);
    } finally {
      isSaving = false;
    }
  }

  onMount(() => {
    loadPatient();
  });
</script>

<div class="p-8 max-w-4xl mx-auto">
  <div class="mb-6">
    <h2 class="text-2xl font-bold text-gray-100 mb-2">Compose Email</h2>
    {#if patient}
      <p class="text-gray-400">
        For patient: {patient.first_name} {patient.last_name}
      </p>
    {/if}
  </div>

  {#if error}
    <div class="mb-6">
      <ErrorDisplay {error} showDetails={true} />
    </div>
  {/if}

  <div class="bg-gray-800 rounded-lg p-6 border border-gray-700 space-y-4">
    <div>
      <label for="recipient" class="block text-sm font-medium text-gray-300 mb-2">
        To:
      </label>
      <input
        id="recipient"
        type="email"
        bind:value={recipientEmail}
        placeholder="recipient@example.com"
        class="w-full px-3 py-2 bg-gray-900 border border-gray-700 rounded text-gray-100 placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
      />
    </div>

    <div>
      <label for="subject" class="block text-sm font-medium text-gray-300 mb-2">
        Subject:
      </label>
      <input
        id="subject"
        type="text"
        bind:value={subject}
        placeholder="Email subject"
        class="w-full px-3 py-2 bg-gray-900 border border-gray-700 rounded text-gray-100 placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
      />
    </div>

    <div>
      <label for="body" class="block text-sm font-medium text-gray-300 mb-2">
        Message:
      </label>
      <textarea
        id="body"
        bind:value={body}
        placeholder="Email body"
        rows="15"
        class="w-full px-3 py-2 bg-gray-900 border border-gray-700 rounded text-gray-100 placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500 font-mono"
      ></textarea>
    </div>

    <div class="flex justify-between items-center pt-4 border-t border-gray-700">
      <a
        href={`/patients/${patientId}/email`}
        class="px-4 py-2 text-gray-400 hover:text-gray-300 transition-colors"
      >
        Cancel
      </a>
      <div class="flex space-x-3">
        <button
          on:click={handleSaveDraft}
          disabled={isSaving}
          class="px-4 py-2 bg-gray-700 text-gray-300 rounded hover:bg-gray-600 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {isSaving ? "Saving..." : "Save Draft"}
        </button>
        <button
          on:click={handleSendEmail}
          disabled={isSaving}
          class="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {isSaving ? "Opening..." : "Open in Mail Client"}
        </button>
      </div>
    </div>
  </div>

  <div class="mt-4 text-sm text-gray-500">
    <p>
      Clicking "Open in Mail Client" will save the draft and open your default email
      application with the email pre-filled. You can then review and send it from there.
    </p>
  </div>
</div>
