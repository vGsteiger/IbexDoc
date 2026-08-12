<script lang="ts">
  import { page } from '$app/stores';
  import { lockApp } from '$lib/api';
  import { authStatus } from '$lib/stores/auth';
  import { goto } from '$app/navigation';
  import {
    Users,
    Calendar,
    BookOpen,
    MessageSquare,
    Settings,
    Lock,
    LayoutDashboard,
  } from 'lucide-svelte';
  import { t } from '$lib/translations';

  const navItems = [
    { path: '/dashboard', labelKey: 'nav.dashboard', icon: LayoutDashboard },
    { path: '/patients', labelKey: 'nav.patients', icon: Users },
    { path: '/calendar', labelKey: 'nav.calendar', icon: Calendar },
    { path: '/literature', labelKey: 'nav.literature', icon: BookOpen },
    { path: '/chat', labelKey: 'nav.chat', icon: MessageSquare },
    { path: '/settings', labelKey: 'nav.settings', icon: Settings },
  ];

  async function handleLock() {
    try {
      await lockApp();
      authStatus.set('locked');
      goto('/unlock');
    } catch (error) {
      console.error('Failed to lock app:', error);
    }
  }

  let currentPath = $derived($page.url.pathname);
</script>

<aside class="flex h-screen w-56 flex-col border-r border-line-subtle bg-surface-sunken">
  <div class="flex h-14 shrink-0 items-center px-3">
    <span class="text-heading text-fg">RamDoc</span>
  </div>

  <nav class="flex-1 px-2 pb-2">
    <ul class="space-y-0.5">
      {#each navItems as item}
        {@const Icon = item.icon}
        {@const active = currentPath === item.path}
        <li>
          <!-- Selection is a quiet raised surface plus a 2px accent bar, not a
               saturated fill: the accent stays meaningful because it is scarce. -->
          <a
            href={item.path}
            aria-current={active ? 'page' : undefined}
            class="relative flex h-8 items-center gap-2.5 rounded-control px-2.5 text-body transition-colors duration-150 ease-standard {active
              ? 'bg-surface-selected font-medium text-fg'
              : 'text-fg-muted hover:bg-surface-hover hover:text-fg'}"
          >
            {#if active}
              <span
                class="absolute left-0 top-1/2 h-4 w-0.5 -translate-y-1/2 rounded-full bg-accent"
                aria-hidden="true"
              ></span>
            {/if}
            <Icon size={16} class={active ? 'text-fg' : 'text-fg-subtle'} />
            <span class="truncate">{$t(item.labelKey)}</span>
          </a>
        </li>
      {/each}
    </ul>
  </nav>

  <div class="shrink-0 border-t border-line-subtle p-2">
    <button
      onclick={handleLock}
      class="flex h-8 w-full items-center gap-2.5 rounded-control px-2.5 text-body text-fg-muted transition-colors duration-150 ease-standard hover:bg-surface-hover hover:text-fg"
    >
      <Lock size={16} class="text-fg-subtle" />
      <span>{$t('nav.lock')}</span>
    </button>
  </div>
</aside>
