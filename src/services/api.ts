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
import { normalizeAcademicSemesterCode } from '../utils/semester';

function fallbackMeta(): ApiMeta {
  return {
    source: 'unknown',
    timestamp: Math.floor(Date.now() / 1000),
  };
}

function normalizeEnvelope<T>(raw: any): ApiEnvelope<T> {
  if (raw && typeof raw === 'object' && 'data' in raw && '_meta' in raw) {
    return raw as ApiEnvelope<T>;
  }

  // Backward-compatible object: { ...payload, _meta }
  if (raw && typeof raw === 'object' && '_meta' in raw && !('data' in raw)) {
    const { _meta, ...payload } = raw;
    return {
      data: payload as T,
      _meta,
    };
  }

  return {
    data: raw as T,
    _meta: fallbackMeta(),
  };
}

async function callEnvelope<T>(command: string, args?: Record<string, unknown>): Promise<ApiEnvelope<T>> {
  const raw = await invoke(command, args);
  return normalizeEnvelope<T>(raw);
}

export async function fetchScholarData(): Promise<ApiEnvelope<ScholarPayload>> {
  const env = await callEnvelope<ScholarPayload>('fetch_scholar_data');
  const data: any = env.data || {};

  // Backward compatibility: if old shape without gpaByPolicy
  if (!data.gpaByPolicy) {
    data.gpaByPolicy = {
      first: data.gpa || {
        fivePoint: 0,
        fourPoint: 0,
        fourPointLegacy: 0,
        hundredPoint: 0,
        totalCredits: 0,
        majorGpa: 0,
        majorGpaLegacy: 0,
        majorCredits: 0,
      },
      highest: data.gpa || {
        fivePoint: 0,
        fourPoint: 0,
        fourPointLegacy: 0,
        hundredPoint: 0,
        totalCredits: 0,
        majorGpa: 0,
        majorGpaLegacy: 0,
        majorCredits: 0,
      },
    };
  }

  env.data = data;
  return env;
}

export async function fetchTimetable(args: {
  year: string;
  semester: string;
}): Promise<ApiEnvelope<TimetablePayload>> {
  const env = await callEnvelope<TimetablePayload | any[]>('fetch_timetable', args);

  if (Array.isArray(env.data)) {
    env.data = { timetable: env.data } as any;
  } else {
    const payload: any = env.data || {};
    if (!Array.isArray(payload.timetable)) {
      payload.timetable = Array.isArray(payload.kbList) ? payload.kbList : [];
    }
    payload.year = String(payload.year || args.year);
    payload.semester =
      normalizeAcademicSemesterCode(payload.semester || args.semester) ||
      normalizeAcademicSemesterCode(args.semester) ||
      '1';
    env.data = payload;
  }

  return env as ApiEnvelope<TimetablePayload>;
}

export async function fetchTodos(): Promise<ApiEnvelope<TodosPayload>> {
  const env = await callEnvelope<TodosPayload>('fetch_todos');
  const data: any = env.data || {};

  if (!Array.isArray(data.todo_list)) {
    if (Array.isArray((data as any).data)) {
      data.todo_list = (data as any).data;
    } else {
      data.todo_list = [];
    }
  }

  env.data = data;
  return env;
}

export async function calculateGpaPreview(input: GpaPreviewInput): Promise<GpaSummary> {
  const result = await invoke('calculate_gpa_preview', { input });
  return result as GpaSummary;
}
