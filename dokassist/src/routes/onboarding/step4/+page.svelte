<script lang="ts">
  import { goto } from '$app/navigation';
  import { completeOnboarding, parseError } from '$lib/api';
  import { ChevronLeft, Users, Mic, Search, FileText, Calendar, CheckCircle } from 'lucide-svelte';

  let isCompleting = $state(false);
  let error = $state<string | null>(null);

  const colorClasses: Record<string, { bg: string; text: string }> = {
    blue: { bg: 'bg-accent-subtle/20', text: 'text-accent-fg' },
    green: { bg: 'bg-success-subtle/20', text: 'text-success-fg' },
    purple: { bg: 'bg-accent-subtle/20', text: 'text-accent-fg' },
    yellow: { bg: 'bg-warning-subtle/20', text: 'text-warning-fg' },
    red: { bg: 'bg-danger-subtle/20', text: 'text-danger-fg' },
  };

  async function handleComplete() {
    isCompleting = true;
    error = null;

    try {
      await completeOnboarding();
      goto('/dashboard');
    } catch (err) {
      error = parseError(err).message;
      isCompleting = false;
    }
  }

  function handleBack() {
    goto('/onboarding/step3');
  }

  const features = [
    {
      icon: Users,
      title: 'Patient Management',
      description:
        'Create and manage patient records with comprehensive demographic information, diagnoses, medications, and treatment plans.',
      color: 'blue',
    },
    {
      icon: Calendar,
      title: 'Session Scheduling',
      description:
        'Schedule therapy sessions with calendar integration. Track session notes, AMDP data, and clinical observations.',
      color: 'green',
    },
    {
      icon: Mic,
      title: 'Session Recording',
      description:
        'Record therapy sessions and generate AI-powered summaries. All processing happens locally for maximum privacy.',
      color: 'purple',
    },
    {
      icon: Search,
      title: 'Global Search',
      description:
        'Quickly find patients, sessions, and notes using powerful full-text search. Press Cmd+K to open search from anywhere.',
      color: 'yellow',
    },
    {
      icon: FileText,
      title: 'Report Generation',
      description:
        'Generate clinical reports, letters, and documentation with AI assistance. All data stays on your device.',
      color: 'red',
    },
  ];
</script>

<div class="min-h-screen bg-surface flex items-center justify-center p-8">
  <div class="max-w-4xl w-full">
    <div class="mb-8 text-center">
      <h1 class="text-display font-semibold text-fg mb-2">Welcome to RamDoc!</h1>
      <p class="text-fg-muted">
        Here's a quick overview of the key features to help you get started.
      </p>
      <div class="flex items-center justify-center gap-2 mt-4">
        <div class="h-2 w-16 bg-accent rounded-full"></div>
        <div class="h-2 w-16 bg-accent rounded-full"></div>
        <div class="h-2 w-16 bg-accent rounded-full"></div>
        <div class="h-2 w-16 bg-accent rounded-full"></div>
      </div>
    </div>

    {#if error}
      <div class="bg-danger-subtle border border-danger-line rounded-card p-4 mb-6">
        <p class="text-danger-fg text-body">{error}</p>
      </div>
    {/if}

    <div class="bg-surface-raised border border-line-subtle rounded-card p-8 space-y-6">
      <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
        {#each features as feature}
          {@const FeatureIcon = feature.icon}
          <div class="bg-surface-hover rounded-card p-6 border border-line">
            <div class="inline-block p-3 {colorClasses[feature.color].bg} rounded-card mb-4">
              <FeatureIcon size={28} class={colorClasses[feature.color].text} />
            </div>
            <h3 class="text-fg font-semibold mb-2">{feature.title}</h3>
            <p class="text-fg-muted text-body">{feature.description}</p>
          </div>
        {/each}
      </div>

      <div class="bg-accent-subtle border border-accent-line rounded-card p-6">
        <div class="flex items-start gap-4">
          <div class="flex-shrink-0">
            <CheckCircle size={24} class="text-accent-fg" />
          </div>
          <div>
            <h3 class="text-accent-fg font-semibold mb-2">Privacy & Security</h3>
            <p class="text-fg-muted text-body leading-relaxed">
              All your data is encrypted at rest using SQLCipher with AES-256 encryption. Patient
              files are stored in an encrypted vault. The AI model runs locally on your machine, so
              patient data never leaves your device. Audit logs track all data access for nDSG
              compliance.
            </p>
          </div>
        </div>
      </div>

      <div class="bg-surface-hover rounded-card p-6 border border-line">
        <h3 class="text-fg font-semibold mb-3">Quick Keyboard Shortcuts</h3>
        <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
          <div class="flex items-center justify-between">
            <span class="text-fg-muted text-body">Open command palette</span>
            <kbd
              class="px-2 py-1 bg-surface-raised border border-line rounded-control text-fg-muted text-caption font-mono"
            >
              Cmd+K
            </kbd>
          </div>
          <div class="flex items-center justify-between">
            <span class="text-fg-muted text-body">Create new patient</span>
            <kbd
              class="px-2 py-1 bg-surface-raised border border-line rounded-control text-fg-muted text-caption font-mono"
            >
              Cmd+N
            </kbd>
          </div>
          <div class="flex items-center justify-between">
            <span class="text-fg-muted text-body">Create new session</span>
            <kbd
              class="px-2 py-1 bg-surface-raised border border-line rounded-control text-fg-muted text-caption font-mono"
            >
              Cmd+Shift+S
            </kbd>
          </div>
          <div class="flex items-center justify-between">
            <span class="text-fg-muted text-body">Close command palette</span>
            <kbd
              class="px-2 py-1 bg-surface-raised border border-line rounded-control text-fg-muted text-caption font-mono"
            >
              Esc
            </kbd>
          </div>
        </div>
      </div>
    </div>

    <div class="flex justify-between items-center mt-8">
      <button
        onclick={handleBack}
        disabled={isCompleting}
        class="px-6 py-3 border border-line bg-surface-raised hover:bg-surface-hover disabled:bg-surface-hover disabled:cursor-not-allowed text-fg font-medium rounded-control transition-colors flex items-center gap-2"
      >
        <ChevronLeft size={20} />
        Back
      </button>

      <button
        onclick={handleComplete}
        disabled={isCompleting}
        class="px-8 py-3 bg-accent hover:bg-accent-hover disabled:bg-surface-selected disabled:cursor-not-allowed text-on-accent font-semibold rounded-control transition-colors flex items-center gap-2 text-heading"
      >
        {#if isCompleting}
          Completing...
        {:else}
          Get Started
          <CheckCircle size={24} />
        {/if}
      </button>
    </div>
  </div>
</div>
