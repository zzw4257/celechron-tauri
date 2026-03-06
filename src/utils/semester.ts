export type AcademicSemesterCode = '1' | '2';
export type TimetableSemesterCode = '3' | '12';

export interface AcademicTerm {
  year: string;
  academicSemester: AcademicSemesterCode;
}

export interface TimetableTerm extends AcademicTerm {
  timetableSemester: TimetableSemesterCode;
}

export interface TermDescriptor extends TimetableTerm {
  name: string;
  displayName: string;
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

export function buildTermDescriptor(term: AcademicTerm): TermDescriptor {
  const parsedYear = Number.parseInt(term.year, 10);
  const nextYear = Number.isFinite(parsedYear) ? String(parsedYear + 1) : term.year;
  const shortStart = term.year.slice(-2);
  const shortEnd = nextYear.slice(-2);
  const semesterLabel = term.academicSemester === '2' ? '春夏' : '秋冬';

  return {
    ...term,
    timetableSemester: toTimetableSemesterCode(term.academicSemester),
    name: `${term.year}-${nextYear}-${term.academicSemester}`,
    displayName: `${shortStart}-${shortEnd} ${semesterLabel}`,
  };
}

export function resolveCurrentTimetableTerm(now: Date = new Date()): TermDescriptor {
  const month = now.getMonth() + 1;
  const year = now.getFullYear();

  if (month >= 2 && month <= 8) {
    return buildTermDescriptor({
      year: String(year - 1),
      academicSemester: '2',
    });
  }

  return buildTermDescriptor({
    year: String(month === 1 ? year - 1 : year),
    academicSemester: '1',
  });
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

export function parseTermDescriptor(input: unknown): TermDescriptor | null {
  if (!input || typeof input !== 'object') return null;
  const value = input as Partial<TermDescriptor>;
  const year = String(value.year || '').trim();
  const academicSemester = normalizeAcademicSemesterCode(value.academicSemester);
  if (!year || !academicSemester) {
    return null;
  }

  const descriptor = buildTermDescriptor({
    year,
    academicSemester,
  });

  return {
    ...descriptor,
    name: String(value.name || descriptor.name),
    displayName: String(value.displayName || descriptor.displayName),
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

export function formatTermDisplayName(term?: Partial<TermDescriptor> | null, fallbackName = ''): string {
  const descriptor = parseTermDescriptor(term) || (fallbackName ? parseTermDescriptor(parseAcademicTermFromSemesterName(fallbackName) || null) : null);
  if (descriptor) {
    return descriptor.displayName;
  }

  if (!fallbackName || !fallbackName.includes('-')) return fallbackName;
  const parsed = parseAcademicTermFromSemesterName(fallbackName);
  return parsed ? buildTermDescriptor(parsed).displayName : fallbackName;
}
