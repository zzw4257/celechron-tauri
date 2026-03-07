<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import ActionPill from '../ui/ActionPill.vue';
import InlineStat from '../ui/InlineStat.vue';
import SectionCard from '../ui/SectionCard.vue';
import StatusBanner from '../ui/StatusBanner.vue';
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
const COURSE_TONES = ['#0f7aa9', '#1a8b4f', '#a85516', '#c2410c', '#7c3aed', '#2563eb', '#0f766e', '#be123c'];

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

const timetableCache = new Map<string, TimetablePayload>();
const timetableMetaCache = new Map<string, 'network' | 'cache' | 'unknown'>();

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

function courseTone(key: string) {
  let hash = 0;
  for (const char of key) {
    hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  }
  return COURSE_TONES[hash % COURSE_TONES.length];
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

function matrixCourseAt(dateKey: string, period: number) {
  const day = weekDays.value.find((item) => item.dateKey === dateKey);
  if (!day) return null;
  return day.courses.find((course) => course.session.startPeriod === period) || null;
}

function matrixCovered(dateKey: string, period: number) {
  const day = weekDays.value.find((item) => item.dateKey === dateKey);
  if (!day) return false;
  return day.courses.some((course) => course.session.startPeriod < period && course.session.endPeriod >= period);
}

const selectedDay = computed(() => weekDays.value.find((item) => item.dateKey === selectedDateKey.value) || weekDays.value[0] || null);
const selectedDayCourseCount = computed(() => selectedDay.value?.courses.length || 0);
const selectedDayItemsCount = computed(() => (selectedDay.value?.courses.length || 0) + (selectedDay.value?.todos.length || 0) + (selectedDay.value?.exams.length || 0));
const weekCourseCount = computed(() => weekDays.value.reduce((sum, item) => sum + item.courses.length, 0));
const weekPeriodCount = computed(() => weekDays.value.reduce((sum, item) => sum + item.courses.reduce((courseSum, course) => courseSum + (course.session.endPeriod - course.session.startPeriod + 1), 0), 0));
const weekTodoCount = computed(() => weekDays.value.reduce((sum, item) => sum + item.todos.length, 0));
const selectedDayLabel = computed(() => {
  if (!selectedDay.value) return '未选择日期';
  return `${formatDayLabel(selectedDay.value.date)} ${selectedDay.value.label}`;
});
const anchorLabel = computed(() => {
  if (!anchorInfo.value) return '未加载';
  if (anchorInfo.value.source === 'manual') return '手动校准';
  if (anchorInfo.value.source === 'remote') return '远程时间配置';
  return '默认时间配置';
});

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
  void loadCalendar();
});
</script>

<template>
  <div class="page-shell calendar-view">
    <header class="page-header">
      <div>
        <h1>日程</h1>
        <p class="page-subtitle">保留列表模式，同时补回一周七天按节次展开的周日历模式。</p>
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
      <SectionCard dense title="学期与周次" subtitle="默认定位到当前课表学期与当前周。">
        <div class="calendar-toolbar">
          <div class="term-tabs">
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

          <div class="week-actions">
            <ActionPill :disabled="!activePayload || currentWeek <= 1 || isLoadingTerm" @click="changeWeek(-1)">上一周</ActionPill>
            <span class="week-badge">第 {{ currentWeek }} / {{ totalWeeks }} 周</span>
            <ActionPill :disabled="!activePayload || currentWeek >= totalWeeks || isLoadingTerm" @click="changeWeek(1)">下一周</ActionPill>
            <ActionPill tone="accent" :disabled="!activePayload || isLoadingTerm" @click="goToToday">今天</ActionPill>
          </div>
        </div>
      </SectionCard>

      <div class="calendar-stats">
        <InlineStat label="当前学期" :value="activePayload?.displayName || activeTerm?.displayName || '未加载'" :hint="weekRangeLabel" emphasis />
        <InlineStat label="本周课程" :value="String(weekCourseCount)" :hint="`${weekPeriodCount} 节课时`" />
        <InlineStat label="待办 / 考试" :value="`${weekTodoCount} / ${normalizedExams.length}`" :hint="selectedDayLabel" />
        <InlineStat label="时间基准" :value="anchorLabel" :hint="anchorInfo?.key || '未加载'" />
      </div>

      <SectionCard v-if="isLoadingTerm" title="切换学期中" subtitle="正在加载所选学期课表。">
        <div class="state-card">请稍候，正在重新构建本周视图。</div>
      </SectionCard>

      <div v-else-if="activePayload" class="calendar-layout">
        <SectionCard title="列表模式" :subtitle="`${activePayload.displayName} · ${weekRangeLabel}`">
          <div class="week-grid">
            <article
              v-for="day in weekDays"
              :key="day.dateKey"
              class="day-column"
              :class="{ today: day.isToday, selected: day.isSelected }"
            >
              <button type="button" class="day-column__head" @click="selectedDateKey = day.dateKey">
                <div>
                  <strong>{{ day.label }}</strong>
                  <p>{{ formatDayLabel(day.date) }}</p>
                </div>
                <div class="day-column__meta">
                  <span class="badge" :class="day.courses.length ? 'accent' : ''">{{ day.courses.length }} 节</span>
                  <span v-if="day.isToday" class="badge accent">今天</span>
                  <span v-if="day.todos.length" class="badge danger">{{ day.todos.length }} 任务</span>
                  <span v-if="day.exams.length" class="badge warning">{{ day.exams.length }} 考试</span>
                </div>
              </button>

              <div v-if="day.courses.length" class="day-course-list">
                <button
                  v-for="course in day.courses"
                  :key="course.id"
                  type="button"
                  class="course-card"
                  :style="{ '--course-accent': courseTone(course.session.xkkh || course.session.courseName) }"
                  @click="selectedDateKey = day.dateKey"
                >
                  <strong>{{ course.session.courseName }}</strong>
                  <p>{{ course.startSlot?.start || '--:--' }} - {{ course.endSlot?.end || '--:--' }}</p>
                  <small>{{ course.session.location || '地点待定' }}</small>
                </button>
              </div>
              <div v-else class="day-empty">今日无课</div>
            </article>
          </div>
        </SectionCard>

        <SectionCard class="calendar-matrix-card" title="周日历模式" subtitle="保留原来的七天 x 节次矩阵，课程卡片可点击联动右侧当天详情。" dense>
          <div class="timetable-matrix">
            <div class="timetable-matrix__head period">节次</div>
            <div
              v-for="day in weekDays"
              :key="`head-${day.dateKey}`"
              class="timetable-matrix__head"
              :class="{ today: day.isToday, selected: day.isSelected }"
              @click="selectedDateKey = day.dateKey"
            >
              <strong>{{ day.label }}</strong>
              <small>{{ formatDayLabel(day.date) }}</small>
            </div>
            <template v-for="period in periodRows" :key="`period-${period}`">
              <div class="timetable-matrix__period">第{{ period }}节</div>
              <div
                v-for="day in weekDays"
                :key="`${day.dateKey}-${period}`"
                class="timetable-matrix__cell"
                :class="{ selected: day.isSelected }"
              >
                <button
                  v-if="matrixCourseAt(day.dateKey, period)"
                  type="button"
                  class="matrix-course-card"
                  :style="{ '--course-accent': courseTone(matrixCourseAt(day.dateKey, period)?.session.xkkh || matrixCourseAt(day.dateKey, period)?.session.courseName || '') }"
                  @click="selectedDateKey = day.dateKey"
                >
                  <strong>{{ matrixCourseAt(day.dateKey, period)?.session.courseName }}</strong>
                  <small>{{ matrixCourseAt(day.dateKey, period)?.startSlot?.start || '--:--' }} - {{ matrixCourseAt(day.dateKey, period)?.endSlot?.end || '--:--' }}</small>
                </button>
                <div v-else-if="matrixCovered(day.dateKey, period)" class="matrix-course-card matrix-course-card--ghost"></div>
              </div>
            </template>
          </div>
        </SectionCard>

        <SectionCard :title="selectedDayLabel" :subtitle="`当天共 ${selectedDayItemsCount} 项安排，课程 ${selectedDayCourseCount} 项。`">
          <div v-if="!selectedDayItemsCount" class="state-card">这一天没有课程、任务或考试安排。</div>

          <div v-else class="agenda-groups">
            <div v-if="selectedDay?.exams.length" class="agenda-group">
              <h3>考试</h3>
              <article v-for="item in selectedDay.exams" :key="item.id" class="agenda-item warning">
                <strong>{{ item.title }}</strong>
                <p>{{ item.location }}</p>
                <small>{{ item.timeLabel }}</small>
              </article>
            </div>

            <div v-if="selectedDay?.todos.length" class="agenda-group">
              <h3>任务</h3>
              <article v-for="item in selectedDay.todos" :key="item.id" class="agenda-item danger">
                <strong>{{ item.title }}</strong>
                <p>{{ item.courseName }}</p>
                <small>{{ formatDateTimeLabel(item.date) }}</small>
              </article>
            </div>

            <div v-if="selectedDay?.courses.length" class="agenda-group">
              <h3>课程</h3>
              <article
                v-for="course in selectedDay.courses"
                :key="course.id"
                class="agenda-item course"
                :style="{ '--course-accent': courseTone(course.session.xkkh || course.session.courseName) }"
              >
                <strong>{{ course.session.courseName }}</strong>
                <p>{{ course.session.location || '地点待定' }}<span v-if="course.session.teacher"> · {{ course.session.teacher }}</span></p>
                <small>{{ course.startSlot?.start || '--:--' }} - {{ course.endSlot?.end || '--:--' }} · 第{{ course.session.startPeriod }}-{{ course.session.endPeriod }}节</small>
              </article>
            </div>
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

.calendar-header-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.75rem;
}

.calendar-toolbar,
.week-actions,
.term-tabs,
.calendar-stats,
.day-column__meta,
.agenda-groups,
.agenda-group {
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem;
}

.calendar-toolbar {
  align-items: center;
  justify-content: space-between;
}

.term-tabs {
  flex: 1;
}

.term-tab {
  border: 1px solid var(--border-subtle);
  background: var(--surface-2);
  color: var(--text-secondary);
  border-radius: var(--radius-pill);
  min-height: 2.35rem;
  padding: 0.55rem 0.95rem;
  cursor: pointer;
}

.term-tab.active {
  background: var(--surface-accent);
  border-color: var(--accent-border);
  color: var(--accent-text);
}

.week-actions {
  align-items: center;
}

.week-badge {
  min-height: 2.35rem;
  display: inline-flex;
  align-items: center;
  padding: 0.55rem 0.95rem;
  border-radius: var(--radius-pill);
  border: 1px solid var(--border-subtle);
  background: var(--surface-2);
  color: var(--text-primary);
}

.calendar-stats {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
}

.calendar-layout {
  display: grid;
  grid-template-columns: minmax(0, 1.1fr) minmax(0, 1.1fr);
  gap: 1rem;
  align-items: start;
}

.calendar-matrix-card {
  grid-column: 1 / -1;
}

.week-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 0.75rem;
}

.day-column {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: linear-gradient(165deg, color-mix(in srgb, var(--accent-text) 8%, var(--surface-1)) 0%, var(--surface-2) 100%);
  padding: 0.9rem;
  box-shadow: var(--shadow-soft);
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  min-height: 15rem;
}

.day-column.today {
  border-color: var(--accent-border);
}

.day-column.selected {
  background: var(--surface-accent);
  box-shadow: inset 0 0 0 1px var(--accent-border);
}

.day-column__head {
  border: none;
  background: transparent;
  text-align: left;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.55rem;
  color: var(--text-primary);
  cursor: pointer;
}

.day-column__head strong,
.agenda-group h3,
.course-card strong,
.agenda-item strong {
  color: var(--text-primary);
}

.day-column__head p,
.course-card p,
.course-card small,
.agenda-item p,
.agenda-item small {
  margin: 0;
  color: var(--text-secondary);
}

.day-course-list,
.agenda-group {
  display: flex;
  flex-direction: column;
  gap: 0.65rem;
}

.course-card,
.agenda-item {
  border: 1px solid color-mix(in srgb, var(--course-accent, var(--accent-text)) 26%, var(--border-subtle));
  border-radius: var(--radius-card-sm);
  background: linear-gradient(160deg, color-mix(in srgb, var(--course-accent, var(--accent-text)) 10%, var(--surface-1)) 0%, var(--surface-1) 100%);
  padding: 0.8rem 0.9rem;
  text-align: left;
  box-shadow: 0 14px 30px color-mix(in srgb, var(--course-accent, var(--accent-text)) 10%, transparent);
}

.course-card {
  cursor: pointer;
}

.agenda-item.warning {
  border-left-color: var(--warning-text);
}

.agenda-item.danger {
  border-left-color: var(--danger-text);
}

.day-empty {
  color: var(--text-secondary);
  border: 1px dashed var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: var(--surface-1);
  padding: 0.9rem;
}

@media (max-width: 1440px) {
  .week-grid {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }

  .timetable-matrix {
    grid-template-columns: 84px repeat(7, minmax(110px, 1fr));
    overflow-x: auto;
  }
}

@media (max-width: 1100px) {
  .timetable-matrix {
    grid-template-columns: 84px repeat(7, minmax(110px, 1fr));
    overflow-x: auto;
  }
  .calendar-layout {
    grid-template-columns: 1fr;
  }

  .calendar-stats {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .week-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 720px) {
  .calendar-stats,
  .week-grid {
    grid-template-columns: 1fr;
  }

  .calendar-toolbar {
    flex-direction: column;
    align-items: stretch;
  }

  .week-actions {
    justify-content: flex-start;
  }
}
</style>
