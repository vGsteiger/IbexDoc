<script lang="ts">
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { checkAuth } from '$lib/api';
  import { authStatus, isLoading } from '$lib/stores/auth';

  onMount(async () => {
    try {
      const status = await checkAuth();
      authStatus.set(status);

      if (status === 'first_run') {
        goto('/setup');
      } else if (status === 'locked') {
        goto('/unlock');
      } else if (status === 'recovery_required') {
        goto('/recover');
      } else if (status === 'unlocked') {
        goto('/patients');
      }
    } catch (error) {
      console.error('Failed to check auth:', error);
    } finally {
      isLoading.set(false);
    }
  });
</script>

<div class="min-h-screen bg-gray-950 flex items-center justify-center">
  <div class="text-center">
    <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-500 mx-auto"></div>
    <p class="mt-4 text-gray-400">Loading...</p>
  </div>
</div>
