export type AcademicSemesterCode = '1' | '2';
export type TimetableSemesterCode = '3' | '12';

export interface AcademicTerm {
  year: string;
  academicSemester: AcademicSemesterCode;
}

export interface TimetableTerm extends AcademicTerm {
  timetableSemester: TimetableSemesterCode;
}

export function normalizeAcademicSemesterCode(input: unknown): AcademicSemesterCode | null {
  const code = String(input ?? '').trim();
  if (code === '1' || code === '3') return '1';
  if (code === '2' || code === '12') return '2';
  return null;
}

export function toTimetableSemesterCode(academicSemester: AcademicSemesterCode): TimetableSemesterCode {
  return academicSemester === '1' ? '3' : '12';
}

export function resolveCurrentTimetableTerm(now: Date = new Date()): TimetableTerm {
  const month = now.getMonth() + 1;
  const year = now.getFullYear();

  if (month >= 2 && month <= 8) {
    return {
      year: String(year - 1),
      academicSemester: '2',
      timetableSemester: '12',
    };
  }

  return {
    year: String(month === 1 ? year - 1 : year),
    academicSemester: '1',
    timetableSemester: '3',
  };
}

export function parseAcademicTermFromSemesterName(name: string): AcademicTerm | null {
  const match = /^(\d{4})-(\d{4})-(\d+)$/.exec((name || '').trim());
  if (!match) return null;

  const academicSemester = normalizeAcademicSemesterCode(match[3]);
  if (!academicSemester) return null;

  return {
    year: match[1],
    academicSemester,
  };
}

export function toTimetableTerm(term: AcademicTerm): TimetableTerm {
  return {
    ...term,
    timetableSemester: toTimetableSemesterCode(term.academicSemester),
  };
}

export function buildXkkhPrefix(year: string, academicSemester: AcademicSemesterCode): string {
  const parsedYear = Number.parseInt(year, 10);
  const nextYear = Number.isFinite(parsedYear) ? String(parsedYear + 1) : year;
  return `(${year}-${nextYear}-${academicSemester})`;
}
