import { invoke } from '@tauri-apps/api/core';
import type {
  ApiEnvelope,
  ApiMeta,
  GpaPreviewInput,
  GpaSummary,
  ScholarPayload,
  TimetablePayload,
  TodosPayload,
} from '../types/api';

function fallbackMeta(): ApiMeta {
  return {
    source: 'unknown',
    timestamp: Math.floor(Date.now() / 1000),
  };
}

function normalizeEnvelope<T>(raw: unknown): ApiEnvelope<T> {
  if (raw && typeof raw === 'object' && 'data' in raw && '_meta' in raw) {
    return raw as ApiEnvelope<T>;
  }

  throw new Error('Invalid API response: expected envelope payload');
}

async function callEnvelope<T>(command: string, args?: Record<string, unknown>): Promise<ApiEnvelope<T>> {
  const raw = await invoke(command, args);
  const env = normalizeEnvelope<T>(raw);
  env._meta = env._meta || fallbackMeta();
  return env;
}

export async function fetchScholarData(): Promise<ApiEnvelope<ScholarPayload>> {
  const env = await callEnvelope<ScholarPayload>('fetch_scholar_data');
  if (!env.data?.gpaByPolicy?.first || !env.data?.gpaByPolicy?.highest) {
    throw new Error('Invalid scholar payload: gpaByPolicy missing');
  }
  env.data.semesters = Array.isArray(env.data.semesters) ? env.data.semesters : [];
  env.data.retakePolicySupported = Array.isArray(env.data.retakePolicySupported)
    ? env.data.retakePolicySupported
    : ['first', 'highest'];
  return env;
}

export async function fetchTimetable(args: {
  year: string;
  semester: string;
}): Promise<ApiEnvelope<TimetablePayload>> {
  const env = await callEnvelope<TimetablePayload>('fetch_timetable', args);
  if (!env.data?.term || !Array.isArray(env.data?.sessions)) {
    throw new Error('Invalid timetable payload: term/sessions missing');
  }
  return env;
}

export async function fetchTodos(): Promise<ApiEnvelope<TodosPayload>> {
  const env = await callEnvelope<TodosPayload>('fetch_todos');
  if (!Array.isArray(env.data?.todo_list)) {
    env.data.todo_list = [];
  }
  return env;
}

export async function calculateGpaPreview(input: GpaPreviewInput): Promise<GpaSummary> {
  const result = await invoke('calculate_gpa_preview', { input });
  return result as GpaSummary;
}
