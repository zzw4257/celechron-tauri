import type {
  NormalizedTimetableSession,
  SessionTimeSlot,
  TermTimeConfig,
  TimetablePayload,
} from '../types/api';
import type { TermDescriptor } from './semester';

const DAY_MS = 24 * 60 * 60 * 1000;
const WEEK_MS = 7 * DAY_MS;

export interface ResolvedTermAnchor {
  date: Date;
  key: string;
  source: 'manual' | 'remote' | 'fallback';
}

export interface TimetableOccurrence {
  id: string;
  dateKey: string;
  date: Date;
  weekNumber: number;
  dayIdx: number;
  session: NormalizedTimetableSession;
  startSlot: SessionTimeSlot | null;
  endSlot: SessionTimeSlot | null;
  startDateTime: Date | null;
  endDateTime: Date | null;
}

export function startOfLocalDay(date: Date): Date {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

export function formatDateKey(date: Date): string {
  const normalized = startOfLocalDay(date);
  const month = String(normalized.getMonth() + 1).padStart(2, '0');
  const day = String(normalized.getDate()).padStart(2, '0');
  return `${normalized.getFullYear()}-${month}-${day}`;
}

export function parseDateKey(input?: string | null): Date | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(String(input || '').trim());
  if (!match) {
    return null;
  }

  const year = Number.parseInt(match[1], 10);
  const month = Number.parseInt(match[2], 10);
  const day = Number.parseInt(match[3], 10);
  if (!Number.isFinite(year) || !Number.isFinite(month) || !Number.isFinite(day)) {
    return null;
  }

  return new Date(year, month - 1, day);
}

export function addDays(base: Date, days: number): Date {
  const result = startOfLocalDay(base);
  result.setDate(result.getDate() + days);
  return result;
}

function normalizeToMonday(date: Date): Date {
  const normalized = startOfLocalDay(date);
  const weekday = normalized.getDay() || 7;
  normalized.setDate(normalized.getDate() - weekday + 1);
  return normalized;
}

function fallbackTermStartDate(term: TermDescriptor): Date {
  const year = Number.parseInt(term.year, 10);
  if (term.academicSemester === '2') {
    const calendarYear = Number.isFinite(year) ? year + 1 : new Date().getFullYear();
    return normalizeToMonday(new Date(calendarYear, 1, 24));
  }

  const calendarYear = Number.isFinite(year) ? year : new Date().getFullYear();
  return normalizeToMonday(new Date(calendarYear, 8, 9));
}

export function resolveTermAnchor(
  payload: Pick<TimetablePayload, 'term' | 'timeConfig'>,
  options?: {
    manualAnchors?: Record<string, string>;
    timeConfigMode?: 'remote' | 'manual';
  },
): ResolvedTermAnchor {
  const manualKey = options?.manualAnchors?.[payload.term.name];
  const manualDate = parseDateKey(manualKey);
  if (options?.timeConfigMode === 'manual' && manualDate) {
    return {
      date: normalizeToMonday(manualDate),
      key: formatDateKey(manualDate),
      source: 'manual',
    };
  }

  const remoteDate = parseDateKey(payload.timeConfig?.startDate);
  if (remoteDate) {
    return {
      date: normalizeToMonday(remoteDate),
      key: formatDateKey(remoteDate),
      source: 'remote',
    };
  }

  if (manualDate) {
    return {
      date: normalizeToMonday(manualDate),
      key: formatDateKey(manualDate),
      source: 'manual',
    };
  }

  const fallbackDate = fallbackTermStartDate(payload.term);
  return {
    date: fallbackDate,
    key: formatDateKey(fallbackDate),
    source: 'fallback',
  };
}

export function getWeekNumberForDate(date: Date, anchorDate: Date): number {
  const diff = startOfLocalDay(date).getTime() - startOfLocalDay(anchorDate).getTime();
  return Math.floor(diff / WEEK_MS) + 1;
}

export function clampWeekNumber(weekNumber: number, totalWeeks: number): number {
  if (!Number.isFinite(weekNumber)) {
    return 1;
  }
  return Math.min(Math.max(Math.trunc(weekNumber), 1), Math.max(totalWeeks, 1));
}

export function getWeekMonday(anchorDate: Date, weekNumber: number): Date {
  return addDays(anchorDate, (weekNumber - 1) * 7);
}

export function buildSessionTimeMap(timeConfig: TermTimeConfig): Map<number, SessionTimeSlot> {
  const map = new Map<number, SessionTimeSlot>();
  for (const slot of timeConfig.sessionTimes || []) {
    map.set(slot.index, slot);
  }
  return map;
}

function withTime(date: Date, hhmm?: string): Date | null {
  const match = /^(\d{2}):(\d{2})$/.exec(String(hhmm || '').trim());
  if (!match) {
    return null;
  }

  const value = startOfLocalDay(date);
  value.setHours(Number.parseInt(match[1], 10), Number.parseInt(match[2], 10), 0, 0);
  return value;
}

export function buildCourseOccurrences(
  payload: Pick<TimetablePayload, 'sessions' | 'timeConfig' | 'term'>,
  options?: {
    manualAnchors?: Record<string, string>;
    timeConfigMode?: 'remote' | 'manual';
  },
): TimetableOccurrence[] {
  const anchor = resolveTermAnchor(payload as Pick<TimetablePayload, 'term' | 'timeConfig'>, options);
  const slotMap = buildSessionTimeMap(payload.timeConfig);
  const holidays = payload.timeConfig.holidays || {};
  const exchanges = payload.timeConfig.exchanges || {};
  const occurrences: TimetableOccurrence[] = [];

  for (const session of payload.sessions || []) {
    for (const weekNumber of session.weekNumbers || []) {
      const originalDate = addDays(anchor.date, (weekNumber - 1) * 7 + (session.dayOfWeek - 1));
      const originalKey = formatDateKey(originalDate);
      if (holidays[originalKey]) {
        continue;
      }

      const exchangedDate = parseDateKey(exchanges[originalKey]);
      const actualDate = exchangedDate || originalDate;
      const startSlot = slotMap.get(session.startPeriod) || null;
      const endSlot = slotMap.get(session.endPeriod) || null;
      const startDateTime = withTime(actualDate, startSlot?.start);
      const endDateTime = withTime(actualDate, endSlot?.end);
      const actualDay = actualDate.getDay() || 7;

      occurrences.push({
        id: `${session.id}-${weekNumber}-${formatDateKey(actualDate)}`,
        dateKey: formatDateKey(actualDate),
        date: actualDate,
        weekNumber,
        dayIdx: actualDay - 1,
        session,
        startSlot,
        endSlot,
        startDateTime,
        endDateTime,
      });
    }
  }

  occurrences.sort((left, right) => {
    const leftTime = left.startDateTime?.getTime() ?? left.date.getTime();
    const rightTime = right.startDateTime?.getTime() ?? right.date.getTime();
    return leftTime - rightTime || left.session.courseName.localeCompare(right.session.courseName);
  });
  return occurrences;
}

export function groupOccurrencesByDate(occurrences: TimetableOccurrence[]): Map<string, TimetableOccurrence[]> {
  const grouped = new Map<string, TimetableOccurrence[]>();
  for (const occurrence of occurrences) {
    const bucket = grouped.get(occurrence.dateKey) || [];
    bucket.push(occurrence);
    grouped.set(occurrence.dateKey, bucket);
  }
  return grouped;
}

export function getTotalWeeks(occurrences: TimetableOccurrence[], sessions: NormalizedTimetableSession[] = []): number {
  const allWeeks = [
    ...occurrences.map((item) => item.weekNumber),
    ...sessions.flatMap((session) => session.weekNumbers || []),
  ];
  return Math.max(...allWeeks, 1);
}
