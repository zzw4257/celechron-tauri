export type MetaSource = 'network' | 'cache' | 'unknown';

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
  grades: any[];
  gpaByPolicy?: {
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

export interface TimetablePayload {
  timetable: any[];
  year?: string;
  semester?: '1' | '2';
  xqm?: '3' | '12';
}

export interface TodosPayload {
  [key: string]: any;
  todo_list?: any[];
}

export interface GpaPreviewInput {
  grades: any[];
  selectedIds?: string[];
  simulatedScores?: Record<string, number>;
  retakePolicy?: 'first' | 'highest';
  majorCourseIds?: string[];
}
