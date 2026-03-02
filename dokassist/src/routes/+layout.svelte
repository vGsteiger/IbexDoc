<script lang="ts">
  import { page } from '$app/stores';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import TopBar from '$lib/components/TopBar.svelte';
  import '../app.css';

  let currentPath = $derived($page.url.pathname);

  const authPaths = ['/', '/setup', '/unlock', '/recover'];
  let showLayout = $derived(!authPaths.includes(currentPath));
</script>

{#if showLayout}
  <div class="flex h-screen bg-gray-950">
    <Sidebar />
    <div class="flex-1 flex flex-col overflow-hidden">
      <TopBar />
      <main class="flex-1 overflow-auto">
        <slot />
      </main>
    </div>
  </div>
{:else}
  <slot />
{/if}
