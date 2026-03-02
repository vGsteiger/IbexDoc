import { invoke } from '@tauri-apps/api/core';

export type AuthStatus = 'first_run' | 'locked' | 'unlocked' | 'recovery_required';

export async function checkAuth(): Promise<AuthStatus> {
  return await invoke<AuthStatus>('check_auth');
}

export async function initializeApp(): Promise<string[]> {
  return await invoke<string[]>('initialize_app');
}

export async function unlockApp(): Promise<boolean> {
  return await invoke<boolean>('unlock_app');
}

export async function recoverApp(words: string[]): Promise<boolean> {
  return await invoke<boolean>('recover_app', { words });
}

export async function lockApp(): Promise<void> {
  return await invoke<void>('lock_app');
}
