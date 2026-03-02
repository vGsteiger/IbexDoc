import { invoke } from '@tauri-apps/api/core';
import type { AuthStatus } from './stores/auth';

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

export interface LlmEngineStatus {
  is_loaded: boolean;
  model_name: string | null;
  model_path: string | null;
  total_ram_bytes: number;
}

export async function getEngineStatus(): Promise<LlmEngineStatus> {
  return await invoke<LlmEngineStatus>('get_engine_status');
}

// Session types and commands
export interface Session {
  id: string;
  patient_id: string;
  session_date: string;
  session_type: string;
  duration_minutes: number | null;
  notes: string | null;
  amdp_data: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateSession {
  patient_id: string;
  session_date: string;
  session_type: string;
  duration_minutes?: number;
  notes?: string;
  amdp_data?: string;
}

export interface UpdateSession {
  session_date?: string;
  session_type?: string;
  duration_minutes?: number;
  notes?: string;
  amdp_data?: string;
}

export async function createSession(input: CreateSession): Promise<Session> {
  return await invoke<Session>('create_session', { input });
}

export async function getSession(id: string): Promise<Session> {
  return await invoke<Session>('get_session', { id });
}

export async function listSessionsForPatient(
  patientId: string,
  limit?: number,
  offset?: number
): Promise<Session[]> {
  return await invoke<Session[]>('list_sessions_for_patient', {
    patientId,
    limit,
    offset
  });
}

export async function updateSession(id: string, input: UpdateSession): Promise<Session> {
  return await invoke<Session>('update_session', { id, input });
}

export async function deleteSession(id: string): Promise<void> {
  return await invoke<void>('delete_session', { id });
}

// Diagnosis types and commands
export interface Diagnosis {
  id: string;
  patient_id: string;
  icd10_code: string;
  description: string;
  status: string;
  diagnosed_date: string;
  resolved_date: string | null;
  notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateDiagnosis {
  patient_id: string;
  icd10_code: string;
  description: string;
  status?: string;
  diagnosed_date: string;
  resolved_date?: string;
  notes?: string;
}

export interface UpdateDiagnosis {
  icd10_code?: string;
  description?: string;
  status?: string;
  diagnosed_date?: string;
  resolved_date?: string;
  notes?: string;
}

export async function createDiagnosis(input: CreateDiagnosis): Promise<Diagnosis> {
  return await invoke<Diagnosis>('create_diagnosis', { input });
}

export async function getDiagnosis(id: string): Promise<Diagnosis> {
  return await invoke<Diagnosis>('get_diagnosis', { id });
}

export async function listDiagnosesForPatient(
  patientId: string,
  limit?: number,
  offset?: number
): Promise<Diagnosis[]> {
  return await invoke<Diagnosis[]>('list_diagnoses_for_patient', {
    patientId,
    limit,
    offset
  });
}

export async function updateDiagnosis(id: string, input: UpdateDiagnosis): Promise<Diagnosis> {
  return await invoke<Diagnosis>('update_diagnosis', { id, input });
}

export async function deleteDiagnosis(id: string): Promise<void> {
  return await invoke<void>('delete_diagnosis', { id });
}

// Medication types and commands
export interface Medication {
  id: string;
  patient_id: string;
  substance: string;
  dosage: string;
  frequency: string;
  start_date: string;
  end_date: string | null;
  notes: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateMedication {
  patient_id: string;
  substance: string;
  dosage: string;
  frequency: string;
  start_date: string;
  end_date?: string;
  notes?: string;
}

export interface UpdateMedication {
  substance?: string;
  dosage?: string;
  frequency?: string;
  start_date?: string;
  end_date?: string;
  notes?: string;
}

export async function createMedication(input: CreateMedication): Promise<Medication> {
  return await invoke<Medication>('create_medication', { input });
}

export async function getMedication(id: string): Promise<Medication> {
  return await invoke<Medication>('get_medication', { id });
}

export async function listMedicationsForPatient(
  patientId: string,
  limit?: number,
  offset?: number
): Promise<Medication[]> {
  return await invoke<Medication[]>('list_medications_for_patient', {
    patientId,
    limit,
    offset
  });
}

export async function updateMedication(id: string, input: UpdateMedication): Promise<Medication> {
  return await invoke<Medication>('update_medication', { id, input });
}

export async function deleteMedication(id: string): Promise<void> {
  return await invoke<void>('delete_medication', { id });
}
