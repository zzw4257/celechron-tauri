import type { TermDescriptor } from '../utils/semester';

export type MetaSource = 'network' | 'cache' | 'unknown';
export type RetakePolicy = 'first' | 'highest';

export interface ApiMeta {
  source: MetaSource;
  timestamp: number;
}

export interface ApiEnvelope<T> {
  data: T;
  _meta: ApiMeta;
}

export interface GpaSummary {
  fivePoint: number;
  fourPoint: number;
  fourPointLegacy: number;
  hundredPoint: number;
  totalCredits: number;
  majorGpa: number;
  majorGpaLegacy: number;
  majorCredits: number;
}

export interface ScholarSemester {
  name: string;
  displayName: string;
  term: TermDescriptor | null;
  grades: any[];
  gpaByPolicy: {
    first: GpaSummary;
    highest: GpaSummary;
  };
  gpa?: number[];
  credits?: number;
}

export interface ScholarPayload {
  gpa?: GpaSummary;
  gpaByPolicy: {
    first: GpaSummary;
    highest: GpaSummary;
  };
  retakePolicySupported: RetakePolicy[];
  transcript: any[];
  majorGrades: any[];
  majorCourseIds: string[];
  exams: any[];
  practice: {
    pt2: number;
    pt3: number;
    pt4: number;
  };
  semesters: ScholarSemester[];
}

export interface SessionTimeSlot {
  index: number;
  start: string;
  end: string;
}

export interface TermTimeConfig {
  source: 'remote' | 'cache' | 'default' | string;
  startDate?: string;
  sessionTimes: SessionTimeSlot[];
  holidays: Record<string, string>;
  exchanges: Record<string, string>;
}

export interface NormalizedTimetableSession {
  id: string;
  xkkh: string;
  courseId: string;
  courseName: string;
  teacher: string;
  location: string;
  dayOfWeek: number;
  startPeriod: number;
  endPeriod: number;
  weekNumbers: number[];
  oddWeek: boolean;
  evenWeek: boolean;
  firstHalf: boolean;
  secondHalf: boolean;
}

export interface TimetablePayload {
  term: TermDescriptor;
  displayName: string;
  year: string;
  semester: '1' | '2';
  xqm: '3' | '12';
  timeConfig: TermTimeConfig;
  sessions: NormalizedTimetableSession[];
  timetable: any[];
}

export interface TodosPayload {
  [key: string]: any;
  todo_list: any[];
}

export interface GpaPreviewInput {
  grades: any[];
  selectedIds?: string[];
  simulatedScores?: Record<string, number>;
  retakePolicy?: RetakePolicy;
  majorCourseIds?: string[];
  courseIdMappings?: Record<string, string>;
}

export interface MaterialAsset {
  id: string;
  courseName: string;
  title: string;
  fileName: string;
  relativePath: string;
  absolutePath: string;
  sourceUrl?: string | null;
  mimeType?: string | null;
  sizeBytes: number;
  downloadedAt: number;
  updatedAt: number;
  exists: boolean;
}

export interface MaterialsPayload {
  items: MaterialAsset[];
}

export interface DownloadMaterialInput {
  url: string;
  courseName: string;
  title: string;
  fileName?: string;
  source?: string;
}

export interface AiAnalysisInput {
  baseUrl: string;
  apiKey?: string;
  prompt: string;
  context: Record<string, unknown>;
  requestBody?: Record<string, unknown>;
}

export interface AiAnalysisPayload {
  markdown: string;
  raw: Record<string, unknown> | string;
  provider: string;
}

export interface DingtalkTestInput {
  webhookUrl: string;
  secret?: string;
  title?: string;
  text?: string;
}
