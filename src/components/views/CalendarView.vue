<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import ActionPill from '../ui/ActionPill.vue';
import SectionCard from '../ui/SectionCard.vue';
import StatusBanner from '../ui/StatusBanner.vue';
import { CalendarDays, ChevronLeft, ChevronRight, List } from 'lucide-vue-next';
import { fetchScholarData, fetchTimetable, fetchTodos } from '../../services/api';
import type { ScholarPayload, TodoItem, TimetablePayload } from '../../types/api';
import { usePreferences } from '../../composables/usePreferences';
import { parseTermDescriptor, resolveCurrentTimetableTerm, type TermDescriptor } from '../../utils/semester';
import {
  addDays,
  buildCourseOccurrences,
  clampWeekNumber,
  formatDateKey,
  getTotalWeeks,
  getWeekMonday,
  getWeekNumberForDate,
  groupOccurrencesByDate,
  resolveTermAnchor,
  startOfLocalDay,
  type TimetableOccurrence,
} from '../../utils/timetable';

interface NormalizedTodo {
  id: string;
  title: string;
  courseName: string;
  date: Date;
  dateKey: string;
  timeLabel: string;
  status: string;
}

interface NormalizedExam {
  id: string;
  title: string;
  location: string;
  date: Date;
  dateKey: string;
  timeLabel: string;
}

const WEEKDAY_LABELS = ['周一', '周二', '周三', '周四', '周五', '周六', '周日'];
const COURSE_PALETTES = [
  { accent: '#58AEE0', surface: '#DBF1FB', surfaceStrong: '#C9E8F8', border: '#8BC9EA', shadow: 'rgba(88, 174, 224, 0.18)', text: '#1F536B', muted: '#567D91' },
  { accent: '#61C49A', surface: '#DDF6EA', surfaceStrong: '#CBEDDE', border: '#97D8B9', shadow: 'rgba(97, 196, 154, 0.18)', text: '#245B4B', muted: '#5A8072' },
  { accent: '#F2A76D', surface: '#FCE8D8', surfaceStrong: '#F8DCC5', border: '#E7BB93', shadow: 'rgba(242, 167, 109, 0.18)', text: '#6B4A2D', muted: '#8D6A4D' },
  { accent: '#F08BB8', surface: '#FBE0ED', surfaceStrong: '#F6D2E4', border: '#E7A9C7', shadow: 'rgba(240, 139, 184, 0.18)', text: '#6B3450', muted: '#8B5B72' },
  { accent: '#9C93F4', surface: '#ECE8FD', surfaceStrong: '#DED8FB', border: '#B9B2F1', shadow: 'rgba(156, 147, 244, 0.17)', text: '#453C74', muted: '#6E6698' },
  { accent: '#77B5F7', surface: '#E0EEFD', surfaceStrong: '#D2E4FB', border: '#9BC5EF', shadow: 'rgba(119, 181, 247, 0.18)', text: '#2C4F73', muted: '#607B97' },
  { accent: '#66C7CF', surface: '#DDF5F6', surfaceStrong: '#CDEDEE', border: '#97D9DC', shadow: 'rgba(102, 199, 207, 0.18)', text: '#245D62', muted: '#5A8589' },
  { accent: '#A8D36B', surface: '#EDF7DB', surfaceStrong: '#E1F0C8', border: '#C6DD94', shadow: 'rgba(168, 211, 107, 0.18)', text: '#425C26', muted: '#708552' },
];

const { accountScope, manualSemesterAnchors, timeConfigMode } = usePreferences();

const isLoading = ref(true);
const isLoadingTerm = ref(false);
const errorMsg = ref('');
const warningMsg = ref('');
const scholarPayload = ref<ScholarPayload | null>(null);
const todoList = ref<TodoItem[]>([]);
const termTabs = ref<TermDescriptor[]>([]);
const activeTermName = ref('');
const activePayload = ref<TimetablePayload | null>(null);
const occurrences = ref<TimetableOccurrence[]>([]);
const currentWeek = ref(1);
const totalWeeks = ref(1);
const selectedDateKey = ref('');
const anchorInfo = ref<ReturnType<typeof resolveTermAnchor> | null>(null);
const baseOffline = ref(false);
const timetableOffline = ref(false);
const refreshStatus = ref('');
const calendarMode = ref<'table' | 'list'>('table');

const timetableCache = new Map<string, TimetablePayload>();
const timetableMetaCache = new Map<string, 'network' | 'cache' | 'unknown'>();

function setCalendarMode(value: 'table' | 'list') {
  calendarMode.value = value;
}

function formatReason(reason: unknown) {
  if (!reason) return '未知错误';
  if (reason instanceof Error) return reason.message;
  return String(reason);
}

function refreshFallbackMessage(meta?: { requestedFresh?: boolean; source?: string; fallbackReason?: string }) {
  if (!meta?.requestedFresh || meta.source !== 'cache') return '';
  return `已尝试强制刷新，但网络失败，当前回退到本地缓存${meta.fallbackReason ? `：${meta.fallbackReason}` : ''}。`;
}

function parseDateTime(value: unknown): Date | null {
  const raw = String(value || '').trim();
  if (!raw) return null;

  const direct = new Date(raw);
  if (!Number.isNaN(direct.getTime())) {
    return direct;
  }

  const match = /^(\d{4})-(\d{2})-(\d{2})(?:[ T](\d{2}):(\d{2})(?::(\d{2}))?)?/.exec(raw);
  if (!match) return null;

  const year = Number.parseInt(match[1], 10);
  const month = Number.parseInt(match[2], 10);
  const day = Number.parseInt(match[3], 10);
  const hour = Number.parseInt(match[4] || '0', 10);
  const minute = Number.parseInt(match[5] || '0', 10);
  const second = Number.parseInt(match[6] || '0', 10);
  return new Date(year, month - 1, day, hour, minute, second, 0);
}

function parseClock(value: unknown) {
  const match = /^(\d{1,2}):(\d{2})/.exec(String(value || '').trim());
  if (!match) return null;
  return {
    hour: Number.parseInt(match[1], 10),
    minute: Number.parseInt(match[2], 10),
  };
}

function formatDayLabel(date: Date) {
  return `${date.getMonth() + 1}月${date.getDate()}日`;
}

function formatDateTimeLabel(date: Date) {
  return `${formatDayLabel(date)} ${date.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })}`;
}

function buildTermTabs(payload: ScholarPayload | null) {
  const current = resolveCurrentTimetableTerm(new Date());
  const map = new Map<string, TermDescriptor>([[current.name, current]]);

  for (const semester of payload?.semesters || []) {
    const parsed = parseTermDescriptor(semester.term);
    if (parsed) {
      map.set(parsed.name, parsed);
    }
  }

  return [...map.values()].sort((left, right) => {
    const leftRank = Number.parseInt(left.year, 10) * 10 + Number.parseInt(left.academicSemester, 10);
    const rightRank = Number.parseInt(right.year, 10) * 10 + Number.parseInt(right.academicSemester, 10);
    return rightRank - leftRank;
  });
}

function ensureSelectedDate() {
  if (!anchorInfo.value) return;

  const monday = getWeekMonday(anchorInfo.value.date, currentWeek.value);
  const keys = Array.from({ length: 7 }, (_, index) => formatDateKey(addDays(monday, index)));
  if (!keys.includes(selectedDateKey.value)) {
    const todayKey = formatDateKey(new Date());
    selectedDateKey.value = keys.includes(todayKey) ? todayKey : keys[0];
  }
}

function rebuildFromPayload(resetWeek = false) {
  if (!activePayload.value) {
    occurrences.value = [];
    totalWeeks.value = 1;
    anchorInfo.value = null;
    return;
  }

  const nextOccurrences = buildCourseOccurrences(activePayload.value, {
    manualAnchors: manualSemesterAnchors.value,
    timeConfigMode: timeConfigMode.value,
  });
  const nextAnchor = resolveTermAnchor(activePayload.value, {
    manualAnchors: manualSemesterAnchors.value,
    timeConfigMode: timeConfigMode.value,
  });

  occurrences.value = nextOccurrences;
  anchorInfo.value = nextAnchor;
  totalWeeks.value = getTotalWeeks(nextOccurrences, activePayload.value.sessions);

  if (resetWeek) {
    const today = startOfLocalDay(new Date());
    const isCurrentTerm = activePayload.value.term.name === resolveCurrentTimetableTerm(today).name;
    currentWeek.value = isCurrentTerm
      ? clampWeekNumber(getWeekNumberForDate(today, nextAnchor.date), totalWeeks.value)
      : 1;
    selectedDateKey.value = isCurrentTerm
      ? formatDateKey(today)
      : formatDateKey(getWeekMonday(nextAnchor.date, currentWeek.value));
  } else {
    currentWeek.value = clampWeekNumber(currentWeek.value, totalWeeks.value);
  }

  ensureSelectedDate();
}

async function loadActiveTerm(resetWeek = false, forceRefresh = false) {
  const term = termTabs.value.find((item) => item.name === activeTermName.value);
  if (!term) {
    activePayload.value = null;
    return false;
  }

  isLoadingTerm.value = true;
  errorMsg.value = '';
  if (forceRefresh) {
    timetableCache.delete(term.name);
    timetableMetaCache.delete(term.name);
  }

  try {
    if (!forceRefresh && timetableCache.has(term.name)) {
      activePayload.value = timetableCache.get(term.name) || null;
      timetableOffline.value = (timetableMetaCache.get(term.name) || 'unknown') === 'cache';
      rebuildFromPayload(resetWeek);
      return true;
    }

    const env = await fetchTimetable({ year: term.year, semester: term.timetableSemester, forceRefresh });
    refreshStatus.value = refreshFallbackMessage(env._meta as any) || refreshStatus.value;
    timetableCache.set(term.name, env.data);
    timetableMetaCache.set(term.name, env._meta?.source || 'unknown');
    timetableOffline.value = env._meta?.source === 'cache';
    activePayload.value = env.data;
    rebuildFromPayload(resetWeek);
    return true;
  } catch (error: any) {
    activePayload.value = null;
    occurrences.value = [];
    errorMsg.value = error?.message || String(error);
    return false;
  } finally {
    isLoadingTerm.value = false;
  }
}

async function loadCalendar(forceRefresh = false) {
  isLoading.value = true;
  errorMsg.value = '';
  warningMsg.value = '';
  refreshStatus.value = '';
  scholarPayload.value = null;
  todoList.value = [];
  termTabs.value = [];
  activePayload.value = null;
  occurrences.value = [];
  timetableCache.clear();
  timetableMetaCache.clear();
  baseOffline.value = false;
  timetableOffline.value = false;

  const currentTerm = resolveCurrentTimetableTerm(new Date());
  const [scholarResult, todoResult] = await Promise.allSettled([
    fetchScholarData({ forceRefresh }),
    fetchTodos({ forceRefresh }),
  ]);
  const warnings: string[] = [];

  if (scholarResult.status === 'fulfilled') {
    scholarPayload.value = scholarResult.value.data;
    baseOffline.value = baseOffline.value || scholarResult.value._meta?.source === 'cache';
    refreshStatus.value = refreshFallbackMessage(scholarResult.value._meta as any) || refreshStatus.value;
  } else {
    warnings.push(`成绩/考试 ${formatReason(scholarResult.reason)}`);
  }

  if (todoResult.status === 'fulfilled') {
    todoList.value = todoResult.value.data.todo_list || [];
    baseOffline.value = baseOffline.value || todoResult.value._meta?.source === 'cache';
    refreshStatus.value = refreshFallbackMessage(todoResult.value._meta as any) || refreshStatus.value;
  } else {
    warnings.push(`任务 ${formatReason(todoResult.reason)}`);
  }

  termTabs.value = buildTermTabs(scholarPayload.value);
  activeTermName.value = termTabs.value.some((item) => item.name === currentTerm.name)
    ? currentTerm.name
    : termTabs.value[0]?.name || currentTerm.name;

  const loaded = await loadActiveTerm(true, forceRefresh);
  warningMsg.value = warnings.join('；');

  if (!loaded && warnings.length > 0) {
    errorMsg.value = `${warnings.join('；')}；课表拉取失败`;
  }

  isLoading.value = false;
}

async function activateTerm(name: string) {
  if (name === activeTermName.value) return;
  activeTermName.value = name;
  await loadActiveTerm(true);
}

async function forceRefreshCalendar() {
  await loadCalendar(true);
}

function changeWeek(offset: number) {
  currentWeek.value = clampWeekNumber(currentWeek.value + offset, totalWeeks.value);
  ensureSelectedDate();
}

function goToToday() {
  const currentTerm = resolveCurrentTimetableTerm(new Date());
  if (activeTermName.value !== currentTerm.name) {
    void activateTerm(currentTerm.name);
    return;
  }

  if (!anchorInfo.value) return;
  currentWeek.value = clampWeekNumber(getWeekNumberForDate(new Date(), anchorInfo.value.date), totalWeeks.value);
  selectedDateKey.value = formatDateKey(new Date());
  ensureSelectedDate();
}

function courseHash(key: string) {
  let hash = 0;
  for (const char of key) {
    hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  }
  return hash;
}

function coursePalette(key: string) {
  return COURSE_PALETTES[courseHash(key) % COURSE_PALETTES.length];
}

function courseStyleVars(key: string) {
  const palette = coursePalette(key);
  return {
    '--course-accent': palette.accent,
    '--course-surface': palette.surface,
    '--course-surface-strong': palette.surfaceStrong,
    '--course-border': palette.border,
    '--course-shadow': palette.shadow,
    '--course-text': palette.text,
    '--course-muted': palette.muted,
  } as Record<string, string>;
}

function normalizeTodo(todo: TodoItem): NormalizedTodo | null {
  const date = parseDateTime(todo.endTime || todo.end_time);
  if (!date) return null;
  return {
    id: todo.id,
    title: todo.title || '未命名任务',
    courseName: todo.courseName || todo.course_name || '学在浙大',
    date,
    dateKey: formatDateKey(date),
    timeLabel: date.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' }),
    status: todo.status || 'pending',
  };
}

function normalizeExam(raw: any, index: number): NormalizedExam | null {
  const baseDate = parseDateTime(raw?.ksrq || raw?.date || raw?.time || raw?.kssj);
  if (!baseDate) return null;

  const clock = parseClock(raw?.kssj || raw?.qzkssj || raw?.time);
  const date = new Date(baseDate);
  if (clock) {
    date.setHours(clock.hour, clock.minute, 0, 0);
  }

  return {
    id: String(raw?.id || raw?.kcmc || `exam-${index}`),
    title: String(raw?.kcmc || raw?.title || '未命名考试'),
    location: String(raw?.cdmc || raw?.location || '地点待定'),
    date,
    dateKey: formatDateKey(date),
    timeLabel: clock ? `${clock.hour.toString().padStart(2, '0')}:${clock.minute.toString().padStart(2, '0')}` : formatDayLabel(date),
  };
}

const normalizedTodos = computed(() => {
  return todoList.value
    .map((item) => normalizeTodo(item))
    .filter((item): item is NormalizedTodo => Boolean(item))
    .sort((left, right) => left.date.getTime() - right.date.getTime());
});

const normalizedExams = computed(() => {
  return (scholarPayload.value?.exams || [])
    .map((item, index) => normalizeExam(item, index))
    .filter((item): item is NormalizedExam => Boolean(item))
    .sort((left, right) => left.date.getTime() - right.date.getTime());
});

const occurrenceMap = computed(() => groupOccurrencesByDate(occurrences.value));
const todoMap = computed(() => {
  const grouped = new Map<string, NormalizedTodo[]>();
  for (const item of normalizedTodos.value) {
    const bucket = grouped.get(item.dateKey) || [];
    bucket.push(item);
    grouped.set(item.dateKey, bucket);
  }
  return grouped;
});
const examMap = computed(() => {
  const grouped = new Map<string, NormalizedExam[]>();
  for (const item of normalizedExams.value) {
    const bucket = grouped.get(item.dateKey) || [];
    bucket.push(item);
    grouped.set(item.dateKey, bucket);
  }
  return grouped;
});

const isOffline = computed(() => baseOffline.value || timetableOffline.value);
const activeTerm = computed(() => termTabs.value.find((item) => item.name === activeTermName.value) || null);
const weekMonday = computed(() => (anchorInfo.value ? getWeekMonday(anchorInfo.value.date, currentWeek.value) : null));
const weekRangeLabel = computed(() => {
  if (!weekMonday.value) return '未加载';
  const sunday = addDays(weekMonday.value, 6);
  return `${formatDayLabel(weekMonday.value)} - ${formatDayLabel(sunday)}`;
});

const weekDays = computed(() => {
  if (!weekMonday.value) return [];

  const todayKey = formatDateKey(new Date());
  return Array.from({ length: 7 }, (_, index) => {
    const date = addDays(weekMonday.value as Date, index);
    const dateKey = formatDateKey(date);
    return {
      label: WEEKDAY_LABELS[index],
      date,
      dateKey,
      isToday: todayKey === dateKey,
      isSelected: selectedDateKey.value === dateKey,
      courses: occurrenceMap.value.get(dateKey) || [],
      todos: todoMap.value.get(dateKey) || [],
      exams: examMap.value.get(dateKey) || [],
    };
  });
});

const periodRows = computed(() => {
  const maxPeriod = Math.max(11, ...weekDays.value.flatMap((day) => day.courses.map((course) => course.session.endPeriod)));
  return Array.from({ length: maxPeriod }, (_, index) => index + 1);
});

const periodSlots = computed(() => {
  const slotMap = new Map((activePayload.value?.timeConfig?.sessionTimes || []).map((slot) => [slot.index, slot]));
  return periodRows.value.map((index) => ({
    index,
    start: slotMap.get(index)?.start || '--:--',
    end: slotMap.get(index)?.end || '--:--',
  }));
});

const tableBackgroundCells = computed(() => {
  return weekDays.value.flatMap((day, dayIndex) => periodSlots.value.map((slot) => ({
    id: `${day.dateKey}-${slot.index}`,
    dateKey: day.dateKey,
    column: String(dayIndex + 2),
    row: String(slot.index + 1),
    selected: day.isSelected,
  })));
});

const tableCourseBlocks = computed(() => {
  return weekDays.value.flatMap((day, dayIndex) => day.courses.map((course) => ({
    id: course.id,
    dateKey: day.dateKey,
    styleVars: courseStyleVars(course.session.xkkh || course.session.courseName),
    column: String(dayIndex + 2),
    row: `${course.session.startPeriod + 1} / ${course.session.endPeriod + 2}`,
    periodLabel: course.session.startPeriod === course.session.endPeriod
      ? `第${course.session.startPeriod}节`
      : `第${course.session.startPeriod}-${course.session.endPeriod}节`,
    start: course.startSlot?.start || '--:--',
    end: course.endSlot?.end || '--:--',
    course,
  })));
});

const selectedDay = computed(() => weekDays.value.find((item) => item.dateKey === selectedDateKey.value) || weekDays.value[0] || null);
const selectedDayItemsCount = computed(() => (selectedDay.value?.courses.length || 0) + (selectedDay.value?.todos.length || 0) + (selectedDay.value?.exams.length || 0));
const weekCourseCount = computed(() => weekDays.value.reduce((sum, item) => sum + item.courses.length, 0));
const weekPeriodCount = computed(() => weekDays.value.reduce((sum, item) => sum + item.courses.reduce((courseSum, course) => courseSum + (course.session.endPeriod - course.session.startPeriod + 1), 0), 0));
const weekTodoCount = computed(() => weekDays.value.reduce((sum, item) => sum + item.todos.length, 0));
const weekExamCount = computed(() => weekDays.value.reduce((sum, item) => sum + item.exams.length, 0));
const visibleMonthLabel = computed(() => {
  if (!weekMonday.value) return '未加载';
  const sunday = addDays(weekMonday.value, 6);
  if (weekMonday.value.getFullYear() === sunday.getFullYear() && weekMonday.value.getMonth() === sunday.getMonth()) {
    return `${weekMonday.value.getFullYear()} 年 ${weekMonday.value.getMonth() + 1} 月`;
  }
  if (weekMonday.value.getFullYear() === sunday.getFullYear()) {
    return `${weekMonday.value.getFullYear()} 年 ${weekMonday.value.getMonth() + 1} 月 - ${sunday.getMonth() + 1} 月`;
  }
  return `${weekMonday.value.getFullYear()} 年 ${weekMonday.value.getMonth() + 1} 月 - ${sunday.getFullYear()} 年 ${sunday.getMonth() + 1} 月`;
});
const selectedDayLabel = computed(() => {
  if (!selectedDay.value) return '未选择日期';
  return `${formatDayLabel(selectedDay.value.date)} ${selectedDay.value.label}`;
});
const selectedDaySubtitle = computed(() => {
  if (!selectedDay.value) return '未选择日期';
  return `课程 ${selectedDay.value.courses.length} 项 · 任务 ${selectedDay.value.todos.length} 项 · 考试 ${selectedDay.value.exams.length} 项`;
});
const selectedDayFocusText = computed(() => {
  if (!selectedDay.value) return '未选择日期';
  const fragments: string[] = [];
  const firstCourse = selectedDay.value.courses[0];
  if (firstCourse) {
    fragments.push(`最早课程 ${firstCourse.startSlot?.start || '--:--'} 开始`);
  }
  if (selectedDay.value.todos.length) {
    fragments.push(`${selectedDay.value.todos.length} 项待办提醒`);
  }
  if (selectedDay.value.exams.length) {
    fragments.push(`${selectedDay.value.exams.length} 场考试安排`);
  }
  return fragments.join(' · ') || '这一天适合留给复盘、整理资料或休息。';
});
const selectedDaySummaryCards = computed(() => {
  if (!selectedDay.value) return [];
  return [
    { label: '课程', value: `${selectedDay.value.courses.length}`, hint: selectedDay.value.courses.length ? '含上课节次与地点' : '今天暂无课程' },
    { label: '任务', value: `${selectedDay.value.todos.length}`, hint: selectedDay.value.todos.length ? '按截止时间提醒' : '今天暂无任务' },
    { label: '考试', value: `${selectedDay.value.exams.length}`, hint: selectedDay.value.exams.length ? '按考试时间提醒' : '今天暂无考试' },
  ];
});
const selectedDayTimeline = computed(() => {
  if (!selectedDay.value) return [];

  const courseItems = selectedDay.value.courses.map((course) => ({
    id: `course-${course.id}`,
    type: 'course' as const,
    sortTime: course.startDateTime?.getTime() ?? course.date.getTime(),
    badge: '课程',
    title: course.session.courseName,
    meta: `${course.session.location || '地点待定'}${course.session.teacher ? ` · ${course.session.teacher}` : ''}`,
    note: `第${course.session.startPeriod}-${course.session.endPeriod}节`,
    timeLabel: `${course.startSlot?.start || '--:--'} - ${course.endSlot?.end || '--:--'}`,
    tone: coursePalette(course.session.xkkh || course.session.courseName).accent,
  }));

  const todoItems = selectedDay.value.todos.map((item) => ({
    id: `todo-${item.id}`,
    type: 'todo' as const,
    sortTime: item.date.getTime(),
    badge: '任务',
    title: item.title,
    meta: item.courseName,
    note: item.status || 'pending',
    timeLabel: formatDateTimeLabel(item.date),
    tone: 'var(--danger-text)',
  }));

  const examItems = selectedDay.value.exams.map((item) => ({
    id: `exam-${item.id}`,
    type: 'exam' as const,
    sortTime: item.date.getTime(),
    badge: '考试',
    title: item.title,
    meta: item.location,
    note: '考试安排',
    timeLabel: item.timeLabel,
    tone: 'var(--warning-text)',
  }));

  return [...courseItems, ...todoItems, ...examItems].sort((left, right) => left.sortTime - right.sortTime || left.title.localeCompare(right.title));
});
const anchorLabel = computed(() => {
  if (!anchorInfo.value) return '未加载';
  if (anchorInfo.value.source === 'manual') return '手动校准';
  if (anchorInfo.value.source === 'remote') return '远程时间配置';
  return '默认时间配置';
});
const calendarStageTitle = computed(() => (calendarMode.value === 'table' ? '周课表' : '列表模式'));
const calendarStageSubtitle = computed(() => {
  const title = activePayload.value?.displayName || activeTerm.value?.displayName || '当前学期';
  return calendarMode.value === 'table'
    ? `${title} · 恢复原来的表格课表，按星期与节次直接看整周分布。`
    : `${title} · 列表模式更适合移动端快速扫今天和本周安排。`;
});
const calendarModeNote = computed(() => (
  calendarMode.value === 'table'
    ? '表格模式适合看整周节次分布；点课程块后，下方详情会同步切到对应日期。'
    : '列表模式适合手机快速浏览；需要完整周课表时随时切回表格。'
));

watch([manualSemesterAnchors, timeConfigMode], () => {
  if (activePayload.value) {
    rebuildFromPayload(false);
  }
});

watch(() => currentWeek.value, () => {
  ensureSelectedDate();
});

watch(accountScope, () => {
  void loadCalendar();
});

onMounted(() => {
  if (window.innerWidth <= 860) {
    calendarMode.value = 'list';
  }
  void loadCalendar();
});
</script>

<template>
  <div class="page-shell calendar-view">
    <header class="page-header">
      <div>
        <h1>日程</h1>
        <p class="page-subtitle">课表恢复为表格主视图，列表模式改为切换项，不再和表格上下堆叠。</p>
      </div>
      <div class="calendar-header-actions">
        <ActionPill tone="accent" :disabled="isLoading || isLoadingTerm" @click="forceRefreshCalendar">强制刷新</ActionPill>
        <span class="badge" :class="isOffline ? 'warning' : 'accent'">{{ isOffline ? '缓存模式' : '实时数据' }}</span>
      </div>
    </header>

    <StatusBanner v-if="errorMsg" tone="danger" title="课表异常">
      {{ errorMsg }}
    </StatusBanner>
    <StatusBanner v-else-if="refreshStatus" tone="warning" title="强制刷新回退">
      {{ refreshStatus }}
    </StatusBanner>
    <StatusBanner v-else-if="warningMsg" tone="warning" title="部分数据未完成">
      {{ warningMsg }}
    </StatusBanner>

    <SectionCard v-if="isLoading" title="加载中" subtitle="正在同步当前学期、课表与近期待办。">
      <div class="state-card">请稍候，正在整理当前周视图。</div>
    </SectionCard>

    <template v-else>
      <SectionCard v-if="!isLoadingTerm && activePayload" dense class="calendar-command-card">
        <div class="calendar-command">
          <div class="calendar-command__hero">
            <div class="calendar-command__week">
              <button type="button" class="week-switch" :disabled="currentWeek <= 1 || isLoadingTerm" @click="changeWeek(-1)">
                <ChevronLeft :size="18" />
              </button>
              <div class="week-hero">
                <div class="week-hero__meta">
                  <span class="week-hero__eyebrow">{{ visibleMonthLabel }}</span>
                  <span class="week-hero__term">{{ activePayload?.displayName || activeTerm?.displayName || '当前学期' }}</span>
                </div>
                <strong>第 {{ currentWeek }} / {{ totalWeeks }} 周</strong>
                <p>{{ weekRangeLabel }}</p>
              </div>
              <button type="button" class="week-switch" :disabled="currentWeek >= totalWeeks || isLoadingTerm" @click="changeWeek(1)">
                <ChevronRight :size="18" />
              </button>
            </div>

            <div class="calendar-command__side">
              <div class="view-switch" role="tablist" aria-label="课表模式切换">
                <button type="button" class="view-switch__item" :class="{ active: calendarMode === 'table' }" @click="setCalendarMode('table')">
                  <CalendarDays :size="16" />
                  <span>周课表</span>
                </button>
                <button type="button" class="view-switch__item" :class="{ active: calendarMode === 'list' }" @click="setCalendarMode('list')">
                  <List :size="16" />
                  <span>列表模式</span>
                </button>
              </div>
              <ActionPill tone="accent" :disabled="!activePayload || isLoadingTerm" @click="goToToday">回到今天</ActionPill>
            </div>
          </div>

          <div class="calendar-command__stats">
            <article class="command-stat primary">
              <span>本周课程</span>
              <strong>{{ weekCourseCount }} 节</strong>
              <small>{{ weekPeriodCount }} 节课时</small>
            </article>
            <article class="command-stat">
              <span>本周任务 / 考试</span>
              <strong>{{ weekTodoCount }} / {{ weekExamCount }}</strong>
              <small>{{ selectedDayLabel }}</small>
            </article>
            <article class="command-stat">
              <span>时间基准</span>
              <strong>{{ anchorLabel }}</strong>
              <small>{{ anchorInfo?.key || '未加载' }}</small>
            </article>
          </div>

          <div class="term-tabs command-terms">
            <button
              v-for="term in termTabs"
              :key="term.name"
              type="button"
              class="term-tab"
              :class="{ active: activeTermName === term.name }"
              @click="activateTerm(term.name)"
            >
              {{ term.displayName }}
            </button>
          </div>

          <p class="mode-note">{{ calendarModeNote }}</p>
        </div>
      </SectionCard>

      <SectionCard v-if="isLoadingTerm" title="切换学期中" subtitle="正在加载所选学期课表。">
        <div class="state-card">请稍候，正在重新构建本周视图。</div>
      </SectionCard>

      <div v-else-if="activePayload" class="calendar-main">
        <SectionCard class="calendar-stage" :title="calendarStageTitle" :subtitle="calendarStageSubtitle">
          <div v-if="calendarMode === 'table'" class="timetable-board-shell">
            <div class="timetable-board" :style="{ gridTemplateRows: `82px repeat(${periodSlots.length}, minmax(92px, auto))` }">
              <div class="timetable-board__corner">节次</div>

              <button
                v-for="(day, dayIndex) in weekDays"
                :key="`day-head-${day.dateKey}`"
                type="button"
                class="timetable-board__day"
                :class="{ today: day.isToday, selected: day.isSelected }"
                :style="{ gridColumn: String(dayIndex + 2), gridRow: '1' }"
                @click="selectedDateKey = day.dateKey"
              >
                <span>{{ formatDayLabel(day.date) }}</span>
                <strong>{{ day.label }}</strong>
                <small>{{ day.courses.length }} 课 · {{ day.todos.length }} 任务 · {{ day.exams.length }} 考试</small>
              </button>

              <div
                v-for="slot in periodSlots"
                :key="`time-${slot.index}`"
                class="timetable-board__time"
                :style="{ gridColumn: '1', gridRow: String(slot.index + 1) }"
              >
                <strong>{{ slot.index }}</strong>
                <span>{{ slot.start }}</span>
              </div>

              <div
                v-for="cell in tableBackgroundCells"
                :key="cell.id"
                class="timetable-board__cell"
                :class="{ selected: cell.selected }"
                :style="{ gridColumn: cell.column, gridRow: cell.row }"
              ></div>

              <button
                v-for="block in tableCourseBlocks"
                :key="block.id"
                type="button"
                class="timetable-course-block"
                :class="{ selected: block.dateKey === selectedDateKey }"
                :style="{
                  gridColumn: block.column,
                  gridRow: block.row,
                  ...block.styleVars,
                }"
                @click="selectedDateKey = block.dateKey"
              >
                <span class="timetable-course-block__period">{{ block.periodLabel }}</span>
                <strong class="timetable-course-block__title">{{ block.course.session.courseName }}</strong>
                <small class="timetable-course-block__meta">{{ block.start }} - {{ block.end }}</small>
                <small class="timetable-course-block__meta">{{ block.course.session.location || '地点待定' }}</small>
              </button>
            </div>
          </div>

          <div v-else class="week-list">
            <article
              v-for="day in weekDays"
              :key="day.dateKey"
              class="week-list-day"
              :class="{ today: day.isToday, selected: day.isSelected }"
            >
              <button type="button" class="week-list-day__head" @click="selectedDateKey = day.dateKey">
                <div class="week-list-day__headline">
                  <span class="week-list-day__weekday">{{ day.label }}</span>
                  <strong>{{ formatDayLabel(day.date) }}</strong>
                  <p>{{ day.courses.length ? `${day.courses.length} 节课程安排` : '今日暂无课程' }}</p>
                </div>
                <div class="week-list-day__badges">
                  <span class="badge" :class="day.courses.length ? 'accent' : ''">{{ day.courses.length }} 节</span>
                  <span v-if="day.isToday" class="badge accent">今天</span>
                  <span v-if="day.todos.length" class="badge danger">{{ day.todos.length }} 任务</span>
                  <span v-if="day.exams.length" class="badge warning">{{ day.exams.length }} 考试</span>
                </div>
              </button>

              <div v-if="day.courses.length" class="week-list-day__courses">
                <button
                  v-for="course in day.courses"
                  :key="course.id"
                  type="button"
                  class="week-list-course"
                  :style="courseStyleVars(course.session.xkkh || course.session.courseName)"
                  @click="selectedDateKey = day.dateKey"
                >
                  <div>
                    <strong class="week-list-course__title">{{ course.session.courseName }}</strong>
                    <p class="week-list-course__meta">{{ course.startSlot?.start || '--:--' }} - {{ course.endSlot?.end || '--:--' }} · 第{{ course.session.startPeriod }}-{{ course.session.endPeriod }}节</p>
                  </div>
                  <small class="week-list-course__meta">{{ course.session.location || '地点待定' }}</small>
                </button>
              </div>
              <div v-else class="day-empty compact">今日无课，右侧 / 下方详情仍会展示任务与考试。</div>
            </article>
          </div>
        </SectionCard>

        <SectionCard class="calendar-detail" :title="selectedDayLabel" :subtitle="selectedDaySubtitle">
          <div class="detail-hero">
            <div class="detail-hero__copy">
              <span class="detail-hero__eyebrow">{{ selectedDay?.isToday ? '今天焦点' : '当日日程' }}</span>
              <strong>{{ selectedDayLabel }}</strong>
              <p>{{ selectedDayFocusText }}</p>
            </div>
            <span class="badge" :class="selectedDayItemsCount ? 'accent' : ''">{{ selectedDayItemsCount }} 项</span>
          </div>

          <div class="day-picker">
            <button
              v-for="day in weekDays"
              :key="`picker-${day.dateKey}`"
              type="button"
              class="day-picker__item"
              :class="{ active: day.isSelected, today: day.isToday }"
              @click="selectedDateKey = day.dateKey"
            >
              <strong>{{ day.label }}</strong>
              <span>{{ formatDayLabel(day.date) }}</span>
            </button>
          </div>

          <div class="detail-summary-grid">
            <article v-for="item in selectedDaySummaryCards" :key="item.label" class="detail-summary-card">
              <span>{{ item.label }}</span>
              <strong>{{ item.value }}</strong>
              <small>{{ item.hint }}</small>
            </article>
          </div>

          <div v-if="!selectedDayItemsCount" class="state-card">这一天没有课程、任务或考试安排。</div>

          <div v-else class="detail-timeline">
            <article
              v-for="item in selectedDayTimeline"
              :key="item.id"
              class="timeline-item"
              :class="item.type"
              :style="{ '--timeline-accent': item.tone }"
            >
              <div class="timeline-item__time">
                <span>{{ item.badge }}</span>
                <strong>{{ item.timeLabel }}</strong>
              </div>
              <div class="timeline-item__content">
                <div class="timeline-item__head">
                  <strong>{{ item.title }}</strong>
                  <span class="badge" :class="item.type === 'course' ? 'accent' : item.type === 'todo' ? 'danger' : 'warning'">{{ item.note }}</span>
                </div>
                <p>{{ item.meta }}</p>
              </div>
            </article>
          </div>
        </SectionCard>
      </div>

      <SectionCard v-else title="未获取到课表" subtitle="当前学期课表为空或接口返回失败。">
        <div class="state-card">请确认已登录，并重新同步当前学期课表。</div>
      </SectionCard>
    </template>
  </div>
</template>

<style scoped>
.calendar-view {
  gap: 1rem;
}

.calendar-header-actions,
.term-tabs,
.week-list-day__badges {
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem;
}

.calendar-header-actions {
  align-items: center;
}

.calendar-command-card {
  background: linear-gradient(135deg, color-mix(in srgb, var(--accent-text) 6%, white) 0%, color-mix(in srgb, var(--accent-text) 2%, var(--surface-1)) 52%, var(--surface-2) 100%);
}

.calendar-command {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.calendar-command__hero {
  display: grid;
  grid-template-columns: minmax(0, 1.4fr) auto;
  gap: 1rem;
  align-items: center;
}

.calendar-command__week {
  display: grid;
  grid-template-columns: 48px minmax(0, 1fr) 48px;
  gap: 0.8rem;
  align-items: center;
}

.week-switch {
  width: 48px;
  height: 48px;
  border-radius: 999px;
  border: 1px solid var(--border-subtle);
  background: color-mix(in srgb, var(--surface-1) 94%, transparent);
  color: var(--text-primary);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  box-shadow: var(--shadow-soft);
  transition: transform 160ms ease, border-color 160ms ease, box-shadow 160ms ease, background 160ms ease;
}

.week-switch:hover:not(:disabled),
.term-tab:hover,
.day-picker__item:hover,
.view-switch__item:hover,
.week-list-course:hover,
.timetable-course-block:hover {
  transform: translateY(-1px);
}

.week-switch:disabled {
  opacity: 0.45;
  cursor: not-allowed;
  box-shadow: none;
}

.week-hero {
  min-height: 7.9rem;
  border: 1px solid color-mix(in srgb, var(--accent-border) 28%, var(--border-subtle));
  border-radius: 32px;
  background: linear-gradient(160deg, color-mix(in srgb, white 82%, var(--surface-1)) 0%, color-mix(in srgb, var(--surface-2) 94%, var(--accent-text) 6%) 100%);
  padding: 1.15rem 1.4rem;
  box-shadow: 0 24px 44px color-mix(in srgb, var(--accent-text) 8%, transparent);
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 0.3rem;
}

.week-hero__meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  flex-wrap: wrap;
}

.week-hero__eyebrow {
  color: var(--text-secondary);
  font-size: 0.88rem;
  letter-spacing: 0.04em;
}

.week-hero__term {
  display: inline-flex;
  align-items: center;
  min-height: 1.85rem;
  padding: 0.25rem 0.7rem;
  border-radius: 999px;
  background: color-mix(in srgb, var(--accent-text) 10%, var(--surface-1));
  color: var(--text-primary);
  font-size: 0.84rem;
}

.week-hero strong {
  color: var(--text-primary);
  font-size: clamp(1.4rem, 2.1vw, 1.85rem);
  line-height: 1.15;
}

.week-hero p {
  margin: 0;
  color: var(--text-secondary);
}

.calendar-command__side {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 0.8rem;
}

.view-switch {
  display: inline-flex;
  gap: 0.42rem;
  padding: 0.32rem;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, var(--border-subtle) 92%, transparent);
  background: linear-gradient(180deg, color-mix(in srgb, white 80%, var(--surface-1)) 0%, color-mix(in srgb, var(--surface-2) 96%, transparent) 100%);
  box-shadow: inset 0 1px 0 color-mix(in srgb, white 64%, transparent), var(--shadow-soft);
}

.view-switch__item {
  border: 1px solid transparent;
  border-radius: 999px;
  background: transparent;
  color: var(--text-secondary);
  font-weight: 600;
  min-height: 2.5rem;
  padding: 0.55rem 0.98rem;
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  cursor: pointer;
  transition: transform 160ms ease, border-color 160ms ease, background 160ms ease, box-shadow 160ms ease, color 160ms ease;
}

.view-switch__item.active {
  background: linear-gradient(180deg, color-mix(in srgb, var(--accent-text) 13%, white) 0%, color-mix(in srgb, var(--accent-text) 9%, var(--surface-1)) 100%);
  border-color: color-mix(in srgb, var(--accent-border) 76%, var(--border-subtle));
  color: var(--text-primary);
  box-shadow: 0 10px 18px color-mix(in srgb, var(--accent-text) 9%, transparent);
}

.calendar-command__stats {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 0.75rem;
}

.command-stat {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: color-mix(in srgb, var(--surface-1) 92%, transparent);
  padding: 0.9rem 0.95rem;
  display: flex;
  flex-direction: column;
  gap: 0.22rem;
}

.command-stat span,
.command-stat small {
  color: var(--text-secondary);
}

.command-stat strong {
  color: var(--text-primary);
  font-size: 1.15rem;
  line-height: 1.2;
}

.command-stat.primary {
  background: linear-gradient(160deg, color-mix(in srgb, var(--accent-text) 10%, var(--surface-1)) 0%, var(--surface-accent) 100%);
  border-color: color-mix(in srgb, var(--accent-border) 70%, var(--border-subtle));
}

.term-tabs {
  flex-wrap: nowrap;
  overflow-x: auto;
  padding-bottom: 0.15rem;
  scrollbar-width: none;
}

.term-tabs::-webkit-scrollbar,
.timetable-board-shell::-webkit-scrollbar,
.day-picker::-webkit-scrollbar {
  display: none;
}

.command-terms {
  gap: 0.55rem;
}

.term-tab {
  border: 1px solid color-mix(in srgb, var(--border-subtle) 90%, transparent);
  background: linear-gradient(180deg, color-mix(in srgb, white 84%, var(--surface-1)) 0%, color-mix(in srgb, var(--surface-2) 94%, transparent) 100%);
  color: var(--text-secondary);
  border-radius: 999px;
  min-height: 2.35rem;
  padding: 0.55rem 0.95rem;
  cursor: pointer;
  white-space: nowrap;
  box-shadow: inset 0 1px 0 color-mix(in srgb, white 62%, transparent);
  transition: transform 160ms ease, border-color 160ms ease, background 160ms ease, box-shadow 160ms ease, color 160ms ease;
}

.term-tab.active {
  background: linear-gradient(180deg, color-mix(in srgb, var(--accent-text) 15%, white) 0%, color-mix(in srgb, var(--accent-text) 11%, var(--surface-1)) 100%);
  border-color: var(--accent-border);
  color: var(--text-primary);
  box-shadow: 0 10px 18px color-mix(in srgb, var(--accent-text) 8%, transparent);
}

.mode-note {
  margin: 0;
  color: var(--text-secondary);
  font-size: 0.9rem;
}

.calendar-main {
  display: grid;
  grid-template-columns: minmax(0, 1.7fr) minmax(320px, 0.95fr);
  gap: 1rem;
  align-items: start;
}

.calendar-stage {
  background: linear-gradient(180deg, color-mix(in srgb, var(--accent-text) 3%, var(--surface-1)) 0%, var(--surface-1) 100%);
}

.calendar-detail {
  position: sticky;
  top: calc(var(--safe-top, 0px) + 1rem);
  background: linear-gradient(180deg, color-mix(in srgb, var(--accent-text) 3%, var(--surface-1)) 0%, var(--surface-1) 100%);
}

.timetable-board-shell {
  overflow-x: auto;
  padding: 0.45rem 0.3rem 0.55rem;
  border-radius: 32px;
  background: linear-gradient(145deg, color-mix(in srgb, white 72%, var(--surface-1)) 0%, color-mix(in srgb, var(--accent-text) 2%, var(--surface-1)) 100%);
  border: 1px solid color-mix(in srgb, var(--accent-border) 14%, var(--border-subtle));
}

.timetable-board {
  display: grid;
  grid-template-columns: 88px repeat(7, minmax(142px, 1fr));
  gap: 0.7rem;
  min-width: 1180px;
}

.timetable-board__corner,
.timetable-board__day,
.timetable-board__time,
.timetable-board__cell {
  border-radius: var(--radius-card-sm);
  border: 1px solid var(--border-subtle);
}

.timetable-board__corner,
.timetable-board__time {
  background: linear-gradient(180deg, color-mix(in srgb, white 86%, var(--surface-1)) 0%, color-mix(in srgb, var(--surface-2) 94%, white) 100%);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.25rem;
  color: var(--text-primary);
}

.timetable-board__corner {
  grid-column: 1;
  grid-row: 1;
  font-weight: 700;
  letter-spacing: 0.04em;
}

.timetable-board__time strong {
  font-size: 1.08rem;
  line-height: 1;
  font-variant-numeric: tabular-nums;
}

.timetable-board__time span {
  color: var(--text-secondary);
  font-size: 0.8rem;
  letter-spacing: 0.02em;
}

.timetable-board__day {
  background: linear-gradient(180deg, color-mix(in srgb, white 86%, var(--surface-1)) 0%, color-mix(in srgb, var(--surface-2) 95%, white) 100%);
  padding: 0.9rem 0.78rem 0.82rem;
  text-align: left;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 0.2rem;
  cursor: pointer;
  color: var(--text-primary);
}

.timetable-board__day strong {
  color: var(--text-primary);
  font-size: 1rem;
  letter-spacing: 0.01em;
}

.timetable-board__day span,
.timetable-board__day small {
  color: var(--text-secondary);
}

.timetable-board__day span {
  font-size: 0.84rem;
}

.timetable-board__day small {
  font-size: 0.78rem;
}

.timetable-board__day.today {
  border-color: color-mix(in srgb, var(--accent-border) 82%, var(--border-subtle));
}

.timetable-board__day.selected {
  background: linear-gradient(180deg, color-mix(in srgb, var(--accent-text) 10%, var(--surface-1)) 0%, var(--surface-accent) 100%);
  border-color: var(--accent-border);
}

.timetable-board__cell {
  background: linear-gradient(180deg, color-mix(in srgb, white 78%, var(--surface-1)) 0%, color-mix(in srgb, var(--surface-1) 96%, var(--surface-2)) 100%);
  border-color: color-mix(in srgb, var(--border-subtle) 76%, transparent);
}

.timetable-board__cell.selected {
  background: color-mix(in srgb, var(--accent-text) 6%, var(--surface-2));
  border-color: color-mix(in srgb, var(--accent-border) 65%, var(--border-subtle));
}

.timetable-board__day,
.timetable-board__time,
.timetable-board__cell {
  backdrop-filter: blur(10px);
}

.timetable-course-block {
  border: 1px solid var(--course-border, color-mix(in srgb, var(--course-accent, var(--accent-text)) 40%, var(--border-subtle)));
  border-radius: 22px;
  background: linear-gradient(165deg, var(--course-surface-strong, color-mix(in srgb, var(--course-accent, var(--accent-text)) 18%, white)) 0%, var(--course-surface, color-mix(in srgb, var(--course-accent, var(--accent-text)) 10%, var(--surface-1))) 100%);
  color: var(--course-text, var(--text-primary));
  box-shadow: 0 18px 34px var(--course-shadow, color-mix(in srgb, var(--course-accent, var(--accent-text)) 14%, transparent));
  padding: 0.95rem 0.9rem;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  gap: 0.45rem;
  text-align: left;
  cursor: pointer;
}

.timetable-course-block.selected {
  box-shadow: 0 22px 40px var(--course-shadow, color-mix(in srgb, var(--course-accent, var(--accent-text)) 18%, transparent)), inset 0 0 0 1px color-mix(in srgb, var(--course-border, var(--course-accent, var(--accent-text))) 74%, white);
}

.timetable-course-block strong {
  color: var(--course-text, var(--text-primary));
  line-height: 1.35;
}

.timetable-course-block__title,
.week-list-course__title {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.timetable-course-block__title {
  font-size: 0.98rem;
  line-height: 1.32;
}

.week-list-course__title {
  font-size: 0.96rem;
  line-height: 1.3;
}

.timetable-course-block small,
.timetable-course-block__period {
  color: var(--course-muted, var(--text-secondary));
}

.timetable-course-block__meta {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.timetable-course-block__period {
  font-size: 0.79rem;
  font-weight: 700;
  letter-spacing: 0.02em;
}

.week-list {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
  gap: 0.72rem;
}

.week-list-day {
  border: 1px solid color-mix(in srgb, var(--border-subtle) 82%, transparent);
  border-radius: var(--radius-card-sm);
  background: linear-gradient(165deg, color-mix(in srgb, var(--accent-text) 7%, var(--surface-1)) 0%, var(--surface-2) 100%);
  padding: 0.9rem;
  box-shadow: var(--shadow-soft);
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.week-list-day.today {
  border-color: color-mix(in srgb, var(--accent-border) 82%, var(--border-subtle));
}

.week-list-day.selected {
  border-color: var(--accent-border);
  background: linear-gradient(165deg, color-mix(in srgb, var(--accent-text) 10%, var(--surface-1)) 0%, var(--surface-accent) 100%);
}

.week-list-day__head {
  border: none;
  background: transparent;
  padding: 0;
  text-align: left;
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.65rem;
  cursor: pointer;
}

.week-list-day__headline {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 0.16rem;
}

.week-list-day__weekday {
  color: var(--text-secondary);
  font-size: 0.78rem;
  letter-spacing: 0.05em;
}

.week-list-day__head strong,
.day-picker__item strong,
.detail-summary-card strong,
.timeline-item__head strong,
.timeline-item__time strong {
  color: var(--text-primary);
}

.week-list-day__head strong {
  font-size: 1rem;
  line-height: 1.25;
}

.week-list-course strong {
  color: var(--course-text, var(--text-primary));
}

.week-list-day__head p,
.day-picker__item span,
.detail-summary-card span,
.detail-summary-card small,
.timeline-item__time span,
.timeline-item__content p,
.day-empty {
  margin: 0;
  color: var(--text-secondary);
}

.week-list-day__head p {
  font-size: 0.83rem;
  line-height: 1.35;
}

.week-list-course p,
.week-list-course small {
  margin: 0;
  color: var(--course-muted, var(--text-secondary));
}

.week-list-day__badges {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.week-list-day__badges .badge {
  box-shadow: inset 0 1px 0 color-mix(in srgb, white 56%, transparent);
}

.week-list-day__courses {
  display: flex;
  flex-direction: column;
  gap: 0.55rem;
}

.week-list-course {
  border: 1px solid var(--course-border, color-mix(in srgb, var(--course-accent, var(--accent-text)) 28%, var(--border-subtle)));
  border-radius: var(--radius-card-sm);
  background: linear-gradient(160deg, var(--course-surface-strong, color-mix(in srgb, var(--course-accent, var(--accent-text)) 10%, var(--surface-1))) 0%, var(--course-surface, var(--surface-1)) 100%);
  padding: 0.72rem 0.82rem;
  text-align: left;
  box-shadow: 0 10px 22px var(--course-shadow, color-mix(in srgb, var(--course-accent, var(--accent-text)) 9%, transparent));
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 0.65rem;
  cursor: pointer;
}

.detail-hero {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.9rem;
  margin-bottom: 0.95rem;
  padding: 0.9rem 0.95rem;
  border: 1px solid var(--border-subtle);
  border-radius: calc(var(--radius-card-sm) + 4px);
  background: linear-gradient(145deg, color-mix(in srgb, var(--accent-text) 6%, var(--surface-1)) 0%, var(--surface-1) 100%);
}

.detail-hero__copy {
  display: flex;
  flex-direction: column;
  gap: 0.24rem;
}

.detail-hero__eyebrow {
  color: var(--text-secondary);
  font-size: 0.84rem;
  letter-spacing: 0.04em;
}

.detail-hero__copy strong {
  color: var(--text-primary);
  font-size: 1.05rem;
}

.detail-hero__copy p {
  margin: 0;
  color: var(--text-secondary);
  line-height: 1.45;
}

.day-picker {
  display: grid;
  grid-template-columns: repeat(7, minmax(0, 1fr));
  gap: 0.55rem;
  margin-bottom: 0.9rem;
}

.day-picker__item {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: color-mix(in srgb, var(--surface-1) 96%, transparent);
  padding: 0.7rem 0.55rem;
  text-align: left;
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  cursor: pointer;
}

.day-picker__item.today {
  border-color: color-mix(in srgb, var(--accent-border) 80%, var(--border-subtle));
}

.day-picker__item.active {
  background: color-mix(in srgb, var(--accent-text) 8%, var(--surface-1));
  border-color: var(--accent-border);
}

.detail-summary-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 0.7rem;
  margin-bottom: 0.95rem;
}

.detail-summary-card {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: color-mix(in srgb, var(--surface-1) 94%, transparent);
  padding: 0.8rem 0.85rem;
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  box-shadow: var(--shadow-soft);
}

.detail-timeline {
  display: flex;
  flex-direction: column;
  gap: 0.7rem;
}

.timeline-item {
  --timeline-accent: var(--accent-text);
  display: grid;
  grid-template-columns: minmax(110px, 0.42fr) minmax(0, 1fr);
  gap: 0.75rem;
  border: 1px solid color-mix(in srgb, var(--timeline-accent) 28%, var(--border-subtle));
  border-radius: var(--radius-card-sm);
  background: linear-gradient(160deg, color-mix(in srgb, var(--timeline-accent) 8%, var(--surface-1)) 0%, var(--surface-1) 100%);
  padding: 0.8rem 0.85rem;
  box-shadow: 0 12px 24px color-mix(in srgb, var(--timeline-accent) 8%, transparent);
}

.timeline-item__time {
  display: flex;
  flex-direction: column;
  gap: 0.18rem;
  padding-right: 0.75rem;
  border-right: 1px solid color-mix(in srgb, var(--timeline-accent) 18%, var(--border-subtle));
}

.timeline-item__time span {
  font-size: 0.82rem;
  font-weight: 600;
}

.timeline-item__time strong {
  font-size: 0.98rem;
}

.timeline-item__content {
  display: flex;
  flex-direction: column;
  gap: 0.28rem;
  min-width: 0;
}

.timeline-item__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
}

.timeline-item__content p {
  line-height: 1.45;
}

.timeline-item.course {
  background: linear-gradient(160deg, color-mix(in srgb, var(--timeline-accent) 9%, var(--surface-1)) 0%, var(--surface-1) 100%);
}

.week-list-course__meta {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.day-empty {
  border: 1px dashed var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: var(--surface-1);
  padding: 0.9rem;
}

.day-empty.compact {
  padding: 0.75rem 0.85rem;
}

@media (max-width: 1440px) {
  .calendar-main {
    grid-template-columns: minmax(0, 1.45fr) minmax(300px, 0.95fr);
  }
}

@media (max-width: 1180px) {
  .calendar-command__hero,
  .calendar-main {
    grid-template-columns: 1fr;
  }

  .calendar-command__side {
    align-items: flex-start;
  }

  .calendar-detail {
    position: static;
  }

  .day-picker {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }
}

@media (max-width: 900px) {
  .calendar-command__week {
    grid-template-columns: 44px minmax(0, 1fr) 44px;
  }

  .calendar-command__stats,
  .detail-summary-grid {
    grid-template-columns: 1fr;
  }

  .timeline-item {
    grid-template-columns: 1fr;
  }

  .timeline-item__time {
    border-right: none;
    border-bottom: 1px solid color-mix(in srgb, var(--timeline-accent) 18%, var(--border-subtle));
    padding-right: 0;
    padding-bottom: 0.55rem;
  }
}

@media (max-width: 720px) {
  .calendar-command__hero {
    gap: 0.9rem;
  }

  .view-switch {
    width: 100%;
  }

  .view-switch__item {
    flex: 1;
    justify-content: center;
    padding-inline: 0.78rem;
  }

  .detail-hero {
    flex-direction: column;
  }

  .day-picker {
    display: flex;
    overflow-x: auto;
    padding-bottom: 0.1rem;
    gap: 0.55rem;
  }

  .day-picker__item {
    min-width: 104px;
    flex: 0 0 auto;
  }

  .week-list {
    grid-template-columns: 1fr;
    gap: 0.65rem;
  }

  .week-list-day {
    padding: 0.8rem 0.82rem;
    gap: 0.65rem;
  }

  .week-list-day__headline {
    gap: 0.12rem;
  }

  .week-list-day__badges {
    gap: 0.42rem;
  }

  .week-list-day__badges .badge {
    min-height: 1.6rem;
    padding-inline: 0.5rem;
    font-size: 0.74rem;
  }

  .week-list-course {
    flex-direction: column;
    gap: 0.35rem;
    padding: 0.7rem 0.76rem;
  }

  .week-list-course__meta {
    white-space: normal;
    overflow: visible;
    text-overflow: unset;
  }
}
</style>
