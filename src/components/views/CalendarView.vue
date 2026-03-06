<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import { save } from '@tauri-apps/plugin-dialog';
import { writeTextFile } from '@tauri-apps/plugin-fs';
import { CalendarDays, BookOpen, Clock, AlertTriangle, MapPin, User, CalendarRange, Download } from "lucide-vue-next";
import { fetchScholarData, fetchTimetable as fetchTimetableApi, fetchTodos } from "../../services/api";
import type { ScholarSemester, SessionTimeSlot, TimetablePayload } from "../../types/api";
import { usePreferences } from "../../composables/usePreferences";
import {
  buildTermDescriptor,
  formatTermDisplayName,
  parseAcademicTermFromSemesterName,
  parseTermDescriptor,
  resolveCurrentTimetableTerm,
  type TermDescriptor,
} from "../../utils/semester";
import {
  buildCourseOccurrences,
  clampWeekNumber,
  formatDateKey,
  getTotalWeeks,
  getWeekMonday,
  getWeekNumberForDate,
  resolveTermAnchor,
  startOfLocalDay
} from "../../utils/timetable";

const isDev = import.meta.env.DEV;
const weekDays = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
const palette = ["#06b6d4", "#8b5cf6", "#f97316", "#22c55e", "#ec4899", "#eab308", "#14b8a6", "#3b82f6", "#ef4444"];
const fallbackSessionTimes: SessionTimeSlot[] = [
  { index: 1, start: "08:00", end: "08:45" },
  { index: 2, start: "08:50", end: "09:35" },
  { index: 3, start: "10:00", end: "10:45" },
  { index: 4, start: "10:50", end: "11:35" },
  { index: 5, start: "11:40", end: "12:25" },
  { index: 6, start: "13:25", end: "14:10" },
  { index: 7, start: "14:15", end: "15:00" },
  { index: 8, start: "15:05", end: "15:50" },
  { index: 9, start: "16:15", end: "17:00" },
  { index: 10, start: "17:05", end: "17:50" },
  { index: 11, start: "18:50", end: "19:35" },
  { index: 12, start: "19:40", end: "20:25" },
  { index: 13, start: "20:30", end: "21:15" },
  { index: 14, start: "21:20", end: "22:05" },
];

interface SemesterTab {
  label: string;
  year: string;
  sem: string;
  academicSem: '1' | '2';
  term: TermDescriptor;
}

interface CourseSlot {
  id: string;
  name: string;
  location: string;
  teacher: string;
  xkkh: string;
  color: string;
  dayIdx: number;
  periodIdx: number;
  span: number;
  activeWeeks: number[];
  weekNumber: number;
  date: Date;
  dateKey: string;
  startTime: string;
  endTime: string;
  startDateTime: Date | null;
  endDateTime: Date | null;
}

const { manualSemesterAnchors, timeConfigMode, setManualSemesterAnchor, setTimeConfigMode } = usePreferences();

const timetablePayload = ref<TimetablePayload | null>(null);
const allCourses = ref<CourseSlot[]>([]);
const allTodos = ref<any[]>([]);
const allExams = ref<any[]>([]);
const semesterTabs = ref<SemesterTab[]>([]);
const activeSemIdx = ref(0);
const totalWeeks = ref(18);
const isLoading = ref(true);
const isOffline = ref(false);
const offlineTime = ref("");
const showMonthNav = ref(false);
const selectedDate = ref(new Date());
const hideCourseInfo = ref(localStorage.getItem('hideCourseInfo') === 'true');
const selectedCourse = ref<CourseSlot | null>(null);
const showCourseDetail = ref(false);
const showCalibrateModal = ref(false);
const calibrateWeekInput = ref(1);
const anchorDate = ref<Date | null>(null);
const anchorSource = ref<'manual' | 'remote' | 'fallback'>('fallback');
const currentWeek = ref(1);
const timetableDebug = ref({
  requestYear: "",
  requestSem: "",
  responseYear: "",
  responseSem: "",
  responseXqm: "",
  metaSource: "",
  metaTime: "",
  targetPrefix: "",
  rawCount: 0,
  targetMatchedRawCount: 0,
  filteredCount: 0,
  prefixTop: [] as Array<{ prefix: string; count: number }>,
});

const periods = computed(() => {
  const slots = timetablePayload.value?.timeConfig?.sessionTimes?.length
    ? timetablePayload.value.timeConfig.sessionTimes
    : fallbackSessionTimes;
  return slots.map((slot) => ({
    label: String(slot.index),
    time: slot.start,
    end: slot.end,
  }));
});

const classHoursStats = computed(() => {
  if (allCourses.value.length === 0) {
    return { total: 0, perTwoWeeks: '0' };
  }
  const totalPeriods = allCourses.value.reduce((sum, course) => sum + course.span, 0);
  const perTwoWeeks = totalWeeks.value > 0 ? (totalPeriods / totalWeeks.value * 2).toFixed(1) : '0';
  return { total: totalPeriods, perTwoWeeks };
});

function toggleHideCourseInfo() {
  hideCourseInfo.value = !hideCourseInfo.value;
  localStorage.setItem('hideCourseInfo', hideCourseInfo.value.toString());
}

function openCourseDetail(course: CourseSlot) {
  selectedCourse.value = course;
  showCourseDetail.value = true;
}

function syncActiveSemester(term?: TermDescriptor | null) {
  if (!term || semesterTabs.value.length === 0) {
    return;
  }
  const index = semesterTabs.value.findIndex((tab) => tab.term.name === term.name);
  if (index >= 0) {
    activeSemIdx.value = index;
  }
}

function buildSemesterTab(semester: ScholarSemester): SemesterTab | null {
  const descriptor = parseTermDescriptor(semester.term) || (() => {
    const parsed = parseAcademicTermFromSemesterName(semester.name);
    return parsed ? buildTermDescriptor(parsed) : null;
  })();
  if (!descriptor) {
    return null;
  }

  return {
    label: formatTermDisplayName(descriptor, semester.name),
    year: descriptor.year,
    sem: descriptor.timetableSemester,
    academicSem: descriptor.academicSemester,
    term: descriptor,
  };
}

function rebuildCourses(payload: TimetablePayload) {
  const occurrences = buildCourseOccurrences(payload, {
    manualAnchors: manualSemesterAnchors.value,
    timeConfigMode: timeConfigMode.value,
  });
  const colorMap = new Map<string, string>();
  let colorIdx = 0;

  const mapped = occurrences.map((occurrence) => {
    const colorKey = occurrence.session.xkkh || occurrence.session.courseName;
    if (!colorMap.has(colorKey)) {
      colorMap.set(colorKey, palette[colorIdx % palette.length]);
      colorIdx += 1;
    }

    return {
      id: occurrence.id,
      name: occurrence.session.courseName,
      location: occurrence.session.location,
      teacher: occurrence.session.teacher,
      xkkh: occurrence.session.xkkh,
      color: colorMap.get(colorKey)!,
      dayIdx: occurrence.dayIdx,
      periodIdx: occurrence.session.startPeriod - 1,
      span: occurrence.session.endPeriod - occurrence.session.startPeriod + 1,
      activeWeeks: [...occurrence.session.weekNumbers],
      weekNumber: occurrence.weekNumber,
      date: occurrence.date,
      dateKey: occurrence.dateKey,
      startTime: occurrence.startSlot?.start || '',
      endTime: occurrence.endSlot?.end || '',
      startDateTime: occurrence.startDateTime,
      endDateTime: occurrence.endDateTime,
    } satisfies CourseSlot;
  });

  allCourses.value = mapped;
  totalWeeks.value = getTotalWeeks(occurrences, payload.sessions);

  const anchor = resolveTermAnchor(payload, {
    manualAnchors: manualSemesterAnchors.value,
    timeConfigMode: timeConfigMode.value,
  });
  anchorDate.value = anchor.date;
  anchorSource.value = anchor.source;
  currentWeek.value = clampWeekNumber(getWeekNumberForDate(new Date(), anchor.date), totalWeeks.value);
  calibrateWeekInput.value = currentWeek.value;
  selectedDate.value = getWeekMonday(anchor.date, currentWeek.value);
}

function switchSemester(idx: number) {
  activeSemIdx.value = idx;
  const tab = semesterTabs.value[idx];
  fetchTimetable(tab.year, tab.sem);
}

function getRealCurrentWeek() {
  if (!anchorDate.value) {
    return 1;
  }
  return clampWeekNumber(getWeekNumberForDate(new Date(), anchorDate.value), totalWeeks.value);
}

const semesterLabel = computed(() => {
  const activeTab = semesterTabs.value[activeSemIdx.value];
  if (activeTab?.label) {
    return activeTab.label;
  }
  if (timetablePayload.value?.displayName) {
    return timetablePayload.value.displayName;
  }
  return resolveCurrentTimetableTerm().displayName;
});

const semesterStartDateStr = computed(() => {
  if (!anchorDate.value) {
    return '--';
  }
  return formatDateKey(anchorDate.value);
});

const viewedMondayDate = computed(() => {
  if (!anchorDate.value) {
    return startOfLocalDay(new Date());
  }
  return getWeekMonday(anchorDate.value, currentWeek.value);
});

const currentMonthStr = computed(() => {
  const d = viewedMondayDate.value;
  return `${d.getFullYear()} 年 ${d.getMonth() + 1} 月`;
});

const dynamicWeekDays = computed(() => {
  return Array.from({ length: 7 }, (_, index) => {
    const date = new Date(viewedMondayDate.value.getTime() + index * 24 * 60 * 60 * 1000);
    const mm = String(date.getMonth() + 1).padStart(2, '0');
    const dd = String(date.getDate()).padStart(2, '0');
    return `${mm}/${dd} ${weekDays[index]}`;
  });
});

function confirmCalibration() {
  if (!timetablePayload.value) {
    return;
  }
  const today = new Date();
  const weekday = today.getDay() || 7;
  const thisMonday = startOfLocalDay(new Date(today.getTime() - (weekday - 1) * 24 * 60 * 60 * 1000));
  const anchor = new Date(thisMonday.getTime() - (calibrateWeekInput.value - 1) * 7 * 24 * 60 * 60 * 1000);
  setManualSemesterAnchor(timetablePayload.value.term.name, formatDateKey(anchor));
  setTimeConfigMode('manual');
  rebuildCourses(timetablePayload.value);
  currentWeek.value = clampWeekNumber(calibrateWeekInput.value, totalWeeks.value);
  showCalibrateModal.value = false;
}

const currentWeekCourses = computed(() => {
  const weekCourses = allCourses.value.filter((course) => course.weekNumber === currentWeek.value);
  const positioned: (CourseSlot & { overlapCount: number; overlapIndex: number })[] = [];

  for (let day = 0; day < 7; day += 1) {
    const dayCourses = weekCourses
      .filter((course) => course.dayIdx === day)
      .sort((left, right) => left.periodIdx - right.periodIdx || right.span - left.span);
    if (!dayCourses.length) {
      continue;
    }

    const clusters: CourseSlot[][] = [];
    for (const course of dayCourses) {
      let attached = false;
      for (const cluster of clusters) {
        const overlaps = cluster.some((existing) => Math.max(course.periodIdx, existing.periodIdx) < Math.min(course.periodIdx + course.span, existing.periodIdx + existing.span));
        if (overlaps) {
          cluster.push(course);
          attached = true;
          break;
        }
      }
      if (!attached) {
        clusters.push([course]);
      }
    }

    for (const cluster of clusters) {
      const columns: CourseSlot[][] = [];
      for (const course of cluster) {
        let placed = false;
        for (let columnIndex = 0; columnIndex < columns.length; columnIndex += 1) {
          const overlaps = columns[columnIndex].some((existing) => Math.max(course.periodIdx, existing.periodIdx) < Math.min(course.periodIdx + course.span, existing.periodIdx + existing.span));
          if (!overlaps) {
            columns[columnIndex].push(course);
            (course as CourseSlot & { _col?: number })._col = columnIndex;
            placed = true;
            break;
          }
        }
        if (!placed) {
          columns.push([course]);
          (course as CourseSlot & { _col?: number })._col = columns.length - 1;
        }
      }

      const maxCols = columns.length;
      cluster.forEach((course) => {
        positioned.push({
          ...course,
          overlapCount: maxCols,
          overlapIndex: (course as CourseSlot & { _col?: number })._col || 0,
        });
      });
    }
  }

  return positioned;
});

const todayCourses = computed(() => {
  const todayKey = formatDateKey(new Date());
  return allCourses.value
    .filter((course) => course.dateKey === todayKey)
    .sort((left, right) => left.periodIdx - right.periodIdx)
    .map((course) => ({
      ...course,
      period: periods.value[course.periodIdx]?.label || '',
      time: course.startTime,
    }));
});

const monthViewDate = computed(() => viewedMondayDate.value);

const monthDays = computed(() => {
  const year = monthViewDate.value.getFullYear();
  const month = monthViewDate.value.getMonth();
  const daysInMonth = new Date(year, month + 1, 0).getDate();
  const firstDay = new Date(year, month, 1).getDay();
  const offset = firstDay === 0 ? 6 : firstDay - 1;

  const days: Array<{ empty: boolean; date?: Date; dayNum?: number }> = [];
  for (let index = 0; index < offset; index += 1) {
    days.push({ empty: true });
  }
  for (let day = 1; day <= daysInMonth; day += 1) {
    days.push({ empty: false, date: new Date(year, month, day), dayNum: day });
  }
  return days;
});

function isInViewedWeek(date: Date): boolean {
  const monday = viewedMondayDate.value.getTime();
  const sunday = monday + 6 * 24 * 60 * 60 * 1000;
  const current = startOfLocalDay(date).getTime();
  return current >= monday && current <= sunday;
}

function isToday(date: Date): boolean {
  return formatDateKey(date) === formatDateKey(new Date());
}


function hasCourseOnDate(date: Date): boolean {
  const dateKey = formatDateKey(date);
  return allCourses.value.some((course) => course.dateKey === dateKey);
}

const selectedDayData = computed(() => {
  if (!selectedDate.value) {
    return { courses: [], exams: [], todos: [] };
  }

  const dateKey = formatDateKey(selectedDate.value);
  const dayTimeStart = startOfLocalDay(selectedDate.value).getTime();
  const dayTimeEnd = dayTimeStart + 24 * 60 * 60 * 1000 - 1;

  const courses = allCourses.value
    .filter((course) => course.dateKey === dateKey)
    .sort((left, right) => left.periodIdx - right.periodIdx);

  const exams = allExams.value.filter((exam) => {
    const timeStr = exam.kssj || exam.qzkssj || (exam.time ? exam.time[0] : '');
    const match = /(\d{4})年(\d{2})月(\d{2})日/.exec(timeStr);
    if (!match) {
      return false;
    }
    const examMs = new Date(`${match[1]}-${match[2]}-${match[3]}T12:00:00`).getTime();
    return examMs >= dayTimeStart && examMs <= dayTimeEnd;
  });

  const todos = allTodos.value.filter((todo) => {
    const expires = todo.expires || todo.end_time;
    if (!expires) {
      return false;
    }
    const dueAt = new Date(expires).getTime();
    return dueAt >= dayTimeStart && dueAt <= dayTimeEnd;
  });

  return { courses, exams, todos };
});

async function fetchExtraData() {
  try {
    const [todoEnv, scholarEnv] = await Promise.all([fetchTodos(), fetchScholarData()]);
    allTodos.value = todoEnv.data.todo_list || [];
    allExams.value = scholarEnv.data.exams || [];
    semesterTabs.value = (scholarEnv.data.semesters || [])
      .map(buildSemesterTab)
      .filter((value): value is SemesterTab => Boolean(value));
    syncActiveSemester(timetablePayload.value?.term || null);
  } catch (error) {
    console.error(error);
  }
}

async function fetchTimetable(overrideYear?: string, overrideSem?: string) {
  isLoading.value = true;

  try {
    const defaultTerm = resolveCurrentTimetableTerm();
    const zjuYearStr = overrideYear || defaultTerm.year;
    const zjuSemStr = overrideSem || defaultTerm.timetableSemester;
    const response = await fetchTimetableApi({ year: zjuYearStr, semester: zjuSemStr });
    const payload = response.data;
    timetablePayload.value = payload;
    rebuildCourses(payload);
    syncActiveSemester(payload.term);

    if (response._meta?.source === 'cache') {
      isOffline.value = true;
      offlineTime.value = new Date(response._meta.timestamp * 1000).toLocaleString('zh-CN', { hour12: false });
    } else {
      isOffline.value = false;
      offlineTime.value = '';
    }

    const prefixCounter: Record<string, number> = {};
    let targetMatchedRawCount = 0;
    const targetPrefix = `(${payload.term.year}-${Number.parseInt(payload.term.year, 10) + 1}-${payload.semester})`;

    for (const row of payload.timetable || []) {
      const xkkh = String(row?.xkkh || '').trim();
      const prefix = extractXkkhPrefix(xkkh);
      prefixCounter[prefix] = (prefixCounter[prefix] || 0) + 1;
      if (xkkh.startsWith(targetPrefix)) {
        targetMatchedRawCount += 1;
      }
    }

    timetableDebug.value = {
      requestYear: zjuYearStr,
      requestSem: zjuSemStr,
      responseYear: payload.year,
      responseSem: payload.semester,
      responseXqm: payload.xqm || '',
      metaSource: response._meta?.source || 'unknown',
      metaTime: response._meta?.timestamp
        ? new Date(response._meta.timestamp * 1000).toLocaleString('zh-CN', { hour12: false })
        : 'N/A',
      targetPrefix,
      rawCount: payload.timetable?.length || 0,
      targetMatchedRawCount,
      filteredCount: payload.sessions.length,
      prefixTop: Object.entries(prefixCounter)
        .sort((left, right) => right[1] - left[1])
        .slice(0, 10)
        .map(([prefix, count]) => ({ prefix, count })),
    };
  } catch (error) {
    console.error('Failed to fetch timetable:', error);
  } finally {
    isLoading.value = false;
  }
}

function formatWeekRanges(weeks: number[]): string {
  if (!weeks.length) {
    return '无';
  }
  const sorted = [...weeks].sort((left, right) => left - right);
  const ranges: string[] = [];
  let start = sorted[0];
  let end = sorted[0];

  for (let index = 1; index < sorted.length; index += 1) {
    if (sorted[index] === end + 1) {
      end = sorted[index];
    } else {
      ranges.push(start === end ? `${start}` : `${start}-${end}`);
      start = sorted[index];
      end = sorted[index];
    }
  }

  ranges.push(start === end ? `${start}` : `${start}-${end}`);
  return `第 ${ranges.join(', ')} 周`;
}

function extractXkkhPrefix(xkkh: string) {
  if (!xkkh.startsWith('(')) {
    return 'UNKNOWN';
  }
  const end = xkkh.indexOf(')');
  return end > 0 ? xkkh.slice(0, end + 1) : 'UNKNOWN';
}

function formatDateForICS(date: Date): string {
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${date.getUTCFullYear()}${pad(date.getUTCMonth() + 1)}${pad(date.getUTCDate())}T${pad(date.getUTCHours())}${pad(date.getUTCMinutes())}00Z`;
}

async function exportToICS() {
  if (allCourses.value.length === 0) {
    alert('当前没有可导出的课程数据！');
    return;
  }

  try {
    const filePath = await save({
      filters: [{ name: 'iCalendar', extensions: ['ics'] }],
      defaultPath: `Celechron_${semesterTabs.value[activeSemIdx.value]?.label || semesterLabel.value}.ics`,
    });
    if (!filePath) {
      return;
    }

    let icsContent = `BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Celechron//Tauri App//ZH
CALSCALE:GREGORIAN
METHOD:PUBLISH
X-WR-CALNAME:ZJU 课程表
X-WR-TIMEZONE:Asia/Shanghai
`;
    const nowStamp = formatDateForICS(new Date());

    for (const course of allCourses.value) {
      if (!course.startDateTime || !course.endDateTime) {
        continue;
      }
      icsContent += `BEGIN:VEVENT
UID:${crypto.randomUUID()}
DTSTAMP:${nowStamp}
DTSTART:${formatDateForICS(course.startDateTime)}
DTEND:${formatDateForICS(course.endDateTime)}
SUMMARY:${course.name}
LOCATION:${course.location || '未知地点'}
DESCRIPTION:${course.teacher ? `教师: ${course.teacher}` : ''}
END:VEVENT
`;
    }

    icsContent += 'END:VCALENDAR';
    await writeTextFile(filePath, icsContent);
    alert('成功导出为 ics 文件！您可以将其导入至系统日历。');
  } catch (error: any) {
    console.error(error);
    alert(`导出失败: ${error.message || error}`);
  }
}

watch(currentWeek, (value) => {
  const clamped = clampWeekNumber(value, totalWeeks.value);
  if (clamped !== value) {
    currentWeek.value = clamped;
  }
});

watch(totalWeeks, (value) => {
  currentWeek.value = clampWeekNumber(currentWeek.value, value);
});

onMounted(async () => {
  await Promise.all([fetchTimetable(), fetchExtraData()]);
});
</script>

<template>
  <div class="calendar-view">
    <header class="cal-header">
      <div class="cal-title-section">
        <span class="month-label-large">{{ currentMonthStr }}</span>
      </div>
      <div class="cal-actions-section">
        <div class="week-selector-pill glass-panel">
          <button class="pill-btn" @click="currentWeek = currentWeek - 1">‹</button>
          <span class="pill-label" @click="calibrateWeekInput = getRealCurrentWeek(); showCalibrateModal = true" title="点击校准周数">
            {{ semesterLabel }} · 第 {{ currentWeek }} 周 / {{ totalWeeks }}
          </span>
          <button class="pill-btn" @click="currentWeek = currentWeek + 1">›</button>
        </div>
        <button class="action-icon-btn primary-icon-btn" @click="currentWeek = getRealCurrentWeek()" title="回到本周">
          <CalendarDays :size="18"/>
        </button>
        <button class="action-icon-btn" :class="{active: showMonthNav}" @click="showMonthNav = !showMonthNav" title="月历导航">
          <CalendarRange :size="18"/>
        </button>
        <button class="action-icon-btn" @click="exportToICS" title="导出日历(ics)">
          <Download :size="18"/>
        </button>
      </div>
    </header>

    <!-- Semester Tabs & Stats -->
    <div class="calendar-meta-bar">
      <div class="semester-tabs">
        <button 
          v-for="(tab, idx) in semesterTabs" 
          :key="idx"
          class="semester-tab"
          :class="{ active: activeSemIdx === idx }"
          @click="switchSemester(idx)"
        >
          {{ tab.label }}
        </button>
      </div>
      <div class="class-hours-badge glass-panel" v-if="semesterTabs.length">
        <span class="badge-title">{{ semesterTabs[activeSemIdx]?.academicSem === '2' ? '春/夏' : '秋/冬' }}学期课时</span>
        <span class="badge-value">{{ classHoursStats.perTwoWeeks }} <span class="badge-unit">节 / 两周</span></span>
      </div>
    </div>

    <!-- Calibration Modal -->
    <div v-if="showCalibrateModal" class="modal-overlay" @click.self="showCalibrateModal = false">
      <div class="modal-content glass-panel">
        <h3 style="margin-top: 0;">日历校准</h3>
        <p style="color: #94a3b8; font-size: 0.9rem;">请选择当前处于第几周（学期未开始可输入负数），系统将自动对齐所有课程与日期：</p>
        <p style="color: #64748b; font-size: 0.8rem;">{{ semesterLabel }}，开学第一周周一: {{ semesterStartDateStr }}</p>
        <div style="display: flex; gap: 10px; align-items: center; justify-content: center; margin: 1.5rem 0;">
          <button class="week-btn" @click="calibrateWeekInput--">-</button>
          <span style="font-size: 1.2rem; min-width: 80px; text-align: center;">第 {{ calibrateWeekInput }} 周</span>
          <button class="week-btn" @click="calibrateWeekInput++">+</button>
        </div>
        <button class="btn-primary" @click="confirmCalibration" style="width: 100%; padding: 0.8rem;">保存设置</button>
      </div>
    </div>

    <!-- Offline Warning Banner -->
    <div v-if="isOffline" class="offline-banner">
      <span class="offline-icon">⚠️</span>
      <div class="offline-text">
        <strong>网络连接异常，暂未同步最新数据。</strong>
        当前显示的是缓存在本地的数据 (更新于: {{ offlineTime }})
      </div>
    </div>

    <section v-if="isDev" class="debug-panel glass-panel">
      <h3>开发诊断 · 课表学期/过滤</h3>
      <div class="debug-grid">
        <div class="debug-item"><span>请求学年</span><strong>{{ timetableDebug.requestYear }}</strong></div>
        <div class="debug-item"><span>请求学期参数</span><strong>{{ timetableDebug.requestSem }}</strong></div>
        <div class="debug-item"><span>返回学年</span><strong>{{ timetableDebug.responseYear }}</strong></div>
        <div class="debug-item"><span>返回学期(1/2)</span><strong>{{ timetableDebug.responseSem }}</strong></div>
        <div class="debug-item"><span>返回 xqm</span><strong>{{ timetableDebug.responseXqm || 'N/A' }}</strong></div>
        <div class="debug-item"><span>数据源</span><strong>{{ timetableDebug.metaSource }}</strong></div>
      </div>
      <div class="debug-grid">
        <div class="debug-item"><span>元数据时间</span><strong>{{ timetableDebug.metaTime }}</strong></div>
        <div class="debug-item"><span>目标前缀</span><strong>{{ timetableDebug.targetPrefix }}</strong></div>
        <div class="debug-item"><span>原始条数</span><strong>{{ timetableDebug.rawCount }}</strong></div>
        <div class="debug-item"><span>目标前缀命中</span><strong>{{ timetableDebug.targetMatchedRawCount }}</strong></div>
        <div class="debug-item"><span>过滤后渲染</span><strong>{{ timetableDebug.filteredCount }}</strong></div>
      </div>
      <div class="debug-rows">
        <div class="debug-row debug-head"><span>xkkh 前缀</span><span>出现次数</span></div>
        <div class="debug-row" v-for="row in timetableDebug.prefixTop" :key="row.prefix">
          <span>{{ row.prefix }}</span><span>{{ row.count }}</span>
        </div>
      </div>
    </section>

    <!-- Today's summary -->
    <section class="today-summary" v-if="todayCourses.length">
      <h3>📌 今日课程</h3>
      <div class="today-list">
        <div v-for="(c, i) in todayCourses" :key="i" class="today-item" :style="{ '--c': (c as any).color }">
          <span class="today-name">{{ (c as any).name }}</span>
          <span class="today-time">{{ (c as any).period }} · {{ (c as any).time }}</span>
          <span class="today-loc">📍 {{ (c as any).location }}</span>
        </div>
      </div>
    </section>

    <!-- Weekly Grid -->
    <section class="schedule-grid-container" v-show="!showMonthNav">
      <div class="schedule-grid">
        <!-- Row 1: Corner + Day Headers (auto-placed) -->
        <div class="grid-corner" :style="{ gridRow: 1, gridColumn: 1 }"></div>
        <div v-for="(day, di) in dynamicWeekDays" :key="day" class="grid-day-header" :style="{ gridRow: 1, gridColumn: di + 2 }">
          <div class="day-num">{{ day.split(' ')[0] }}</div>
          <div class="day-name">{{ day.split(' ')[1] }}</div>
        </div>

        <!-- Rows 2-14: Period labels + empty cells (explicitly placed) -->
        <template v-for="(period, pi) in periods" :key="'p'+pi">
          <div class="grid-period-label" :style="{ gridRow: pi + 2, gridColumn: 1 }">
            <span class="period-num">{{ period.label }}</span>
            <span class="period-time">{{ period.time }}</span>
          </div>
          <div v-for="di in 7" :key="'cell-'+pi+'-'+di" class="grid-cell" :style="{ gridRow: pi + 2, gridColumn: di + 1 + 1 }"></div>
        </template>

        <!-- Course blocks: explicitly positioned via CSS Grid -->
        <div
          v-for="(course, ci) in currentWeekCourses"
          :key="'course-'+ci"
          class="course-block"
          @click="openCourseDetail(course)"
          :style="{
            gridRow: (course.periodIdx + 2) + ' / span ' + course.span,
            gridColumn: course.dayIdx + 2,
            '--accent': course.color,
            width: `calc(100% / ${course.overlapCount} - 2px)`,
            marginLeft: `calc((100% / ${course.overlapCount}) * ${course.overlapIndex} + 1px)`,
            zIndex: 10 + course.overlapIndex,
            cursor: 'pointer'
          }"
        >
          <span class="course-name" v-if="!hideCourseInfo">{{ course.name }}</span>
          <span class="course-loc" v-if="!hideCourseInfo">{{ course.location }}</span>
        </div>
      </div>
    </section>

    <!-- Hide Course Info Toggle -->
    <div class="hide-course-settings glass-panel" v-show="!showMonthNav">
      <span>隐藏课程信息</span>
      <div class="toggle-switch" :class="{ active: hideCourseInfo }" @click="toggleHideCourseInfo">
        <div class="toggle-knob"></div>
      </div>
    </div>

    <!-- Course Detail Modal -->
    <div v-if="showCourseDetail && selectedCourse" class="modal-overlay" @click.self="showCourseDetail = false">
      <div class="modal-content glass-panel" style="max-width: 420px;">
        <div style="display: flex; align-items: center; gap: 10px; margin-bottom: 1rem;">
          <div style="width: 4px; height: 28px; border-radius: 2px;" :style="{ background: selectedCourse.color }"></div>
          <h3 style="margin: 0; font-size: 1.1rem;">{{ selectedCourse.name }}</h3>
        </div>
        <div class="detail-rows">
          <div class="detail-row" v-if="selectedCourse.location">
            <span class="detail-label"><MapPin :size="14" style="vertical-align: -2px; margin-right: 4px;" /> 教 室</span>
            <span class="detail-value">{{ selectedCourse.location }}</span>
          </div>
          <div class="detail-row" v-if="selectedCourse.teacher">
            <span class="detail-label"><User :size="14" style="vertical-align: -2px; margin-right: 4px;" /> 教 师</span>
            <span class="detail-value">{{ selectedCourse.teacher }}</span>
          </div>
          <div class="detail-row">
            <span class="detail-label"><CalendarDays :size="14" style="vertical-align: -2px; margin-right: 4px;" /> 星 期</span>
            <span class="detail-value">{{ weekDays[selectedCourse.dayIdx] }}</span>
          </div>
          <div class="detail-row">
            <span class="detail-label"><Clock :size="14" style="vertical-align: -2px; margin-right: 4px;" /> 节 次</span>
            <span class="detail-value">第 {{ selectedCourse.periodIdx + 1 }}-{{ selectedCourse.periodIdx + selectedCourse.span }} 节
              ({{ periods[selectedCourse.periodIdx]?.time }}-{{ periods[selectedCourse.periodIdx + selectedCourse.span - 1]?.end }})
            </span>
          </div>
          <div class="detail-row">
            <span class="detail-label"><CalendarRange :size="14" style="vertical-align: -2px; margin-right: 4px;" /> 周 次</span>
            <span class="detail-value">{{ formatWeekRanges(selectedCourse.activeWeeks) }}</span>
          </div>
          <div class="detail-row" v-if="selectedCourse.xkkh">
            <span class="detail-label"><Hash :size="14" style="vertical-align: -2px; margin-right: 4px;" /> 课程号</span>
            <span class="detail-value" style="font-size: 0.8rem; opacity: 0.7;">{{ selectedCourse.xkkh }}</span>
          </div>
        </div>
        <button class="btn-primary" @click="showCourseDetail = false" style="width: 100%; padding: 0.7rem; margin-top: 1.2rem;">关闭</button>
      </div>
    </div>

    <!-- Monthly Grid -->
    <section v-if="showMonthNav" class="month-grid-container fade-in" style="margin-top: 1rem;">
      <div class="month-grid glass-panel">
        <div v-for="day in weekDays" :key="day" class="month-day-header">{{ day }}</div>
        
        <div 
          v-for="(dayObj, idx) in monthDays" 
          :key="idx" 
          class="month-cell" 
          :class="{ 
            empty: dayObj.empty, 
            selected: !dayObj.empty && selectedDate && dayObj.date?.toDateString() === selectedDate.toDateString(),
            'in-week': !dayObj.empty && dayObj.date && isInViewedWeek(dayObj.date),
            'is-today': !dayObj.empty && dayObj.date && isToday(dayObj.date)
          }"
          @click="!dayObj.empty && dayObj.date && (selectedDate = dayObj.date)"
        >
          <span v-if="!dayObj.empty" class="month-day-num">{{ dayObj.dayNum }}</span>
          <div v-if="!dayObj.empty" class="month-dots">
            <!-- Courses dot -->
            <div v-if="hasCourseOnDate(dayObj.date!)" class="dot dot-course"></div>
            <!-- Todo dot -->
            <div v-if="allTodos.some(t => {
                if(!t.expires) return false;
                const d = new Date(t.expires).setHours(0,0,0,0);
                return d === new Date(dayObj.date!).setHours(0,0,0,0);
            })" class="dot dot-todo"></div>
            <!-- Exam dot -->
            <div v-if="allExams.some(e => {
                const ts = e.kssj || e.qzkssj || (e.time? e.time[0]:'');
                const m = ts.match(/(\d{4})年(\d{2})月(\d{2})日/);
                if(m) {
                  const ed = new Date(`${m[1]}-${m[2]}-${m[3]}T12:00:00`).setHours(0,0,0,0);
                  return ed === new Date(dayObj.date!).setHours(0,0,0,0);
                }
                return false;
            })" class="dot dot-exam"></div>
          </div>
        </div>
      </div>

      <div class="selected-day-panel glass-panel">
        <h3 class="panel-date" v-if="selectedDate">
          📅 {{ selectedDate.getFullYear() }}年{{ selectedDate.getMonth()+1 }}月{{ selectedDate.getDate() }}日
        </h3>
        
        <div v-if="!selectedDayData.courses.length && !selectedDayData.exams.length && !selectedDayData.todos.length" class="empty-state">
           今日没有日程安排，好好休息吧！ 🎉
        </div>

        <div class="day-section" v-if="selectedDayData.exams.length">
          <h4><AlertTriangle :size="16" /> 考试安排</h4>
          <div class="day-item exam-item" v-for="(e, i) in selectedDayData.exams" :key="'e'+i">
            <div class="item-title">{{ e.kcmc || "未知考试" }}</div>
            <div class="item-desc">{{ e.cdmc || "未知地点" }} · {{ e.kssj || e.qzkssj || "未知时间" }}</div>
          </div>
        </div>

        <div class="day-section" v-if="selectedDayData.todos.length">
          <h4><BookOpen :size="16" /> 学在浙大 (近在眼前)</h4>
          <div class="day-item todo-item" v-for="(t, i) in selectedDayData.todos" :key="'t'+i">
            <div class="item-title">{{ t.title || t.name }}</div>
            <div class="item-desc">截止于: {{ new Date(t.expires).toLocaleTimeString('zh-CN', {hour: '2-digit', minute:'2-digit'}) }}</div>
          </div>
        </div>

        <div class="day-section" v-if="selectedDayData.courses.length">
          <h4><Clock :size="16" /> 课程安排</h4>
          <div class="day-item course-item" v-for="(c, i) in selectedDayData.courses" :key="'c'+i" :style="{ borderLeftColor: c.color }">
            <div class="item-title">{{ c.name }}</div>
            <div class="item-desc">{{ c.location }} · {{ periods[c.periodIdx].time }}</div>
          </div>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.calendar-view {
  padding: 2rem 2.5rem 6rem;
  max-width: 950px;
  margin: 0 auto;
  color: var(--text-main);
}
.debug-panel {
  margin-bottom: 1rem;
  padding: 14px;
  border: 1px solid var(--debug-border, rgba(244, 63, 94, 0.32));
  background: var(--debug-panel-bg, rgba(15, 23, 42, 0.48));
}
.debug-panel h3 {
  margin: 0 0 10px;
  font-size: 0.95rem;
  color: var(--debug-title, #fda4af);
}
.debug-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 8px;
  margin-bottom: 8px;
}
.debug-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  padding: 7px 8px;
  border-radius: 8px;
  border: 1px solid var(--debug-item-border, rgba(148, 163, 184, 0.22));
  background: var(--debug-item-bg, rgba(30, 41, 59, 0.55));
  font-size: 0.8rem;
}
.debug-item span {
  color: var(--text-muted, #94a3b8);
}
.debug-item strong {
  color: var(--text-main, #f8fafc);
  font-variant-numeric: tabular-nums;
}
.debug-rows {
  border: 1px solid var(--debug-rows-border, rgba(148, 163, 184, 0.24));
  border-radius: 8px;
  overflow: hidden;
}
.debug-row {
  display: grid;
  grid-template-columns: 1.8fr 1fr;
  gap: 8px;
  padding: 7px 8px;
  border-top: 1px solid var(--debug-row-border, rgba(148, 163, 184, 0.18));
  font-size: 0.8rem;
}
.debug-row:first-child {
  border-top: none;
}
.debug-row span {
  color: var(--text-main, #e2e8f0);
  font-variant-numeric: tabular-nums;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.debug-head {
  background: var(--debug-head-bg, rgba(30, 41, 59, 0.85));
}
.debug-head span {
  color: var(--text-muted, #94a3b8);
  font-weight: 700;
}
.cal-header {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  margin-bottom: 1.2rem;
}
.cal-title-section {
  display: flex;
  align-items: baseline;
  gap: 10px;
  flex-shrink: 0;
}
.cal-title-section h1 {
  margin: 0;
  font-size: 1.6rem;
  font-weight: 700;
  color: var(--text-main);
}
.month-label {
  font-size: 1rem;
  color: var(--accent-blue);
  font-weight: 600;
}
.cal-actions-section {
  display: flex;
  align-items: center;
  gap: 12px;
}
.week-selector-pill {
  display: flex;
  align-items: center;
  background: rgba(255,255,255,0.08);
  border: 1px solid rgba(255,255,255,0.1);
  border-radius: 999px;
  padding: 4px 6px;
  flex-shrink: 0;
  max-width: 100%;
  overflow: hidden;
  box-shadow: inset 0 2px 4px rgba(255,255,255,0.05);
}
.pill-btn {
  background: transparent;
  border: none;
  color: #94a3b8;
  font-size: 1.5rem;
  cursor: pointer;
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  transition: all .2s;
}

.calendar-meta-bar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
  margin-bottom: 2rem;
  overflow-x: auto;
  padding-bottom: 8px;
}
.semester-tabs {
  display: flex;
  gap: 8px;
  background: rgba(255, 255, 255, 0.05);
  padding: 6px;
  border-radius: 14px;
  border: 1px solid rgba(255, 255, 255, 0.08);
}
.semester-tab {
  background: transparent;
  border: none;
  color: var(--text-muted);
  padding: 6px 14px;
  border-radius: 10px;
  font-size: 0.85rem;
  font-weight: 600;
  cursor: pointer;
  white-space: nowrap;
  transition: all 0.2s;
}
.semester-tab:hover {
  background: rgba(255, 255, 255, 0.1);
  color: var(--text-main);
}
.semester-tab.active {
  background: #38bdf8;
  color: white;
  box-shadow: 0 4px 12px rgba(56, 189, 248, 0.3);
}

:global(html[data-theme='light']) .semester-tabs {
  background: rgba(0, 0, 0, 0.03);
  border-color: rgba(0, 0, 0, 0.06);
}
:global(html[data-theme='light']) .semester-tab {
  color: #64748b;
}
:global(html[data-theme='light']) .semester-tab:hover {
  background: rgba(0, 0, 0, 0.06);
}
:global(html[data-theme='light']) .semester-tab.active {
  background: #0284c7;
  color: white;
}

.class-hours-badge {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.08);
  padding: 8px 16px;
  border-radius: 16px;
  flex-shrink: 0;
}
.badge-title {
  font-size: 0.7rem;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: var(--text-muted);
}
.badge-value {
  font-size: 1.1rem;
  font-weight: 700;
  color: var(--accent-blue);
}
.pill-btn:hover { background: rgba(255,255,255,.1); color: var(--text-main); }
.pill-label {
  padding: 0 12px;
  font-size: 0.85rem;
  font-weight: 600;
  color: var(--text-main);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  cursor: pointer;
  transition: opacity 0.2s;
}
.pill-label:hover {
  opacity: 0.8;
}

:global(html[data-theme='light']) .week-selector-pill {
  background: rgba(0,0,0,0.05);
  border-color: rgba(0,0,0,0.1);
  box-shadow: inset 0 2px 4px rgba(0,0,0,0.02);
}
:global(html[data-theme='light']) .pill-btn {
  color: #64748b;
}
:global(html[data-theme='light']) .pill-btn:hover {
  background: rgba(0,0,0,0.08);
  color: #0f172a;
}
:global(html[data-theme='light']) .pill-label {
  color: #334155;
}

/* Modal */
.modal-overlay {
  position: fixed;
  top: 0; left: 0; width: 100%; height: 100%;
  background: rgba(0,0,0,0.5);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 999;
}
.glass-panel {
  background: rgba(255, 255, 255, 0.05);
  backdrop-filter: blur(24px) saturate(150%);
  -webkit-backdrop-filter: blur(24px) saturate(150%);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 24px;
  padding: 2rem;
  width: 90%;
  max-width: 350px;
  box-shadow: 0 16px 40px -10px rgba(0, 0, 0, 0.4);
}
.week-btn {
  width: 44px;
  height: 44px;
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: var(--text-main);
  font-size: 1.2rem;
  cursor: pointer;
  transition: all 0.2s;
}
.week-btn:hover {
  background: rgba(255, 255, 255, 0.1);
  transform: scale(1.05);
}
:global(html[data-theme='light']) .week-btn {
  background: rgba(0, 0, 0, 0.05);
  border-color: rgba(0, 0, 0, 0.1);
  color: #1e293b;
}

.btn-primary {
  width: 100%;
  padding: 12px;
  margin-top: 1rem;
  background: linear-gradient(135deg, #38bdf8 0%, #0284c7 100%);
  color: #fff;
  border: none;
  border-radius: 12px;
  cursor: pointer;
  font-weight: 600;
  box-shadow: 0 4px 12px rgba(2, 132, 199, 0.3);
  transition: all 0.2s;
}
.btn-primary:hover {
  filter: brightness(1.1);
  transform: translateY(-1px);
}
.btn-primary:active { transform: translateY(1px); }

/* Course Detail Modal */
.detail-rows {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}
.detail-row {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  padding: 0.5rem 0.7rem;
  background: rgba(255, 255, 255, 0.03);
  border-radius: 10px;
  border: 1px solid rgba(255, 255, 255, 0.05);
}
.detail-label {
  font-size: 0.85rem;
  color: var(--text-muted);
  white-space: nowrap;
  margin-right: 1rem;
}
.detail-value {
  font-size: 0.9rem;
  color: var(--text-main);
  text-align: right;
  word-break: break-all;
}

/* Offline Banner */
.offline-banner {
  background: rgba(245, 158, 11, 0.15);
  border: 1px solid rgba(245, 158, 11, 0.3);
  color: #fcd34d;
  padding: 12px 16px;
  border-radius: 16px;
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 0.85rem;
  line-height: 1.4;
  backdrop-filter: blur(12px);
  animation: fade-in 0.4s ease-out;
}
.offline-icon {
  font-size: 1.3rem;
  animation: pulse-warn 2s infinite;
}
.offline-text strong {
  display: block;
  color: #fbbf24;
  margin-bottom: 2px;
}

@keyframes pulse-warn {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.7; transform: scale(1.1); }
}

/* Today Summary */
.today-summary {
  background: rgba(255,255,255,.04);
  border: 1px solid rgba(255,255,255,.08);
  border-radius: 20px;
  padding: 1.2rem 1.5rem;
}
.today-summary h3 {
  margin: 0 0 .8rem;
  font-size: 1rem;
  color: var(--text-main);
}
.today-list { display: flex; gap: 10px; overflow-x: auto; }
.today-item {
  min-width: 160px;
  background: color-mix(in srgb, var(--c) 10%, transparent);
  border: 1px solid color-mix(in srgb, var(--c) 20%, transparent);
  border-radius: 14px;
  padding: .8rem 1rem;
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex-shrink: 0;
}
.today-name { font-weight: 700; font-size: .88rem; color: var(--text-main); }
.today-time { font-size: .75rem; color: var(--text-muted); }
.today-loc { font-size: .75rem; color: var(--text-muted); }

/* Schedule Grid */
.schedule-grid-container {
  overflow-x: auto;
  border-radius: 20px;
  border: 1px solid rgba(255,255,255,.08);
  background: rgba(255,255,255,.03);
}
.schedule-grid {
  display: grid;
  grid-template-columns: 50px repeat(7, minmax(60px, 1fr));
  grid-template-rows: auto repeat(13, minmax(42px, auto));
  min-width: 600px;
}

.grid-corner {
  background: var(--panel-bg);
  border-bottom: 1px solid var(--card-border);
  border-right: 1px solid var(--card-border);
  padding: .6rem;
  backdrop-filter: blur(12px);
}
.grid-day-header {
  text-align: center;
  padding: .6rem;
  font-weight: 600;
  font-size: 0.9rem;
  color: var(--text-main);
  border-bottom: 1px solid var(--card-border);
  border-right: 1px solid var(--card-border);
  background: var(--panel-bg);
  display: flex;
  flex-direction: column;
  gap: 2px;
  backdrop-filter: blur(12px);
}
.day-num { font-size: 0.75rem; color: var(--text-muted); font-weight: 500; }
.day-name { font-size: 0.9rem; }

.grid-period-label {
  padding: .5rem .6rem;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  border-bottom: 1px solid var(--card-border);
  border-right: 1px solid var(--card-border);
  background: var(--panel-bg);
  backdrop-filter: blur(12px);
}
.period-num { font-size: .78rem; font-weight: 700; color: var(--text-main); }
.period-time { font-size: .65rem; color: var(--text-muted); }

.grid-cell {
  border-bottom: 1px solid var(--card-border);
  border-right: 1px solid var(--card-border);
  min-height: 42px;
}
.grid-cell:hover { background: var(--nav-hover-bg); }

.course-block {
  background: color-mix(in srgb, var(--accent) 30%, var(--card-bg));
  border: 1px solid color-mix(in srgb, var(--accent) 50%, transparent);
  border-radius: 6px;
  padding: .3rem .4rem;
  margin: 1px 2px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 1px;
  cursor: pointer;
  transition: transform .15s, box-shadow .15s;
  z-index: 2;
  overflow: hidden;
  box-shadow: 0 1px 4px var(--nav-shadow);
  backdrop-filter: blur(8px);
}
.course-block:hover { transform: scale(1.02); z-index: 10; box-shadow: 0 4px 12px var(--nav-shadow); }
.course-name { font-size: .72rem; font-weight: 700; color: var(--text-inverse); line-height: 1.2; }
.course-loc { font-size: .6rem; color: color-mix(in srgb, var(--text-inverse) 86%, transparent); font-weight: 500; }

/* Month Grid Styles */
.month-grid-container {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
  margin-top: 1.5rem;
  padding-top: 1.5rem;
  border-top: 1px solid var(--panel-border);
}

@media (min-width: 900px) {
  .month-grid-container {
    display: grid;
    grid-template-columns: 1fr 300px;
    align-items: flex-start;
  }
}

.month-grid {
  display: grid;
  grid-template-columns: repeat(7, 1fr);
  gap: 8px;
  padding: 1.5rem;
  border-radius: 20px;
}

.month-day-header {
  font-size: 0.8rem;
  color: var(--text-muted);
  text-align: center;
  font-weight: 600;
  margin-bottom: 8px;
}

.month-cell {
  aspect-ratio: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  border-radius: 12px;
  background: var(--card-bg, rgba(255,255,255,0.03));
  border: 1px solid var(--card-border, rgba(255,255,255,0.05));
  cursor: pointer;
  transition: all 0.2s;
  position: relative;
}

.month-cell:not(.empty):hover {
  background: rgba(255,255,255,0.08);
  transform: translateY(-2px);
}

.month-cell.empty {
  visibility: hidden;
}

.month-cell.selected {
  background: rgba(56, 189, 248, 0.2);
  border-color: #38bdf8;
  box-shadow: 0 4px 12px rgba(56, 189, 248, 0.2);
}

.month-cell.in-week {
  background: rgba(56, 189, 248, 0.08);
  border-color: rgba(56, 189, 248, 0.15);
}

.month-cell.is-today .month-day-num {
  background: #38bdf8;
  color: #0f172a;
  border-radius: 50%;
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.month-day-num {
  font-size: 1.1rem;
  font-weight: 600;
  color: var(--text-main);
  z-index: 2;
}

.month-dots {
  display: flex;
  gap: 3px;
  margin-top: 4px;
}
.dot {
  width: 5px; height: 5px; border-radius: 50%;
}
.dot-course { background: #38bdf8; }
.dot-todo { background: #f97316; }
.dot-exam { background: #ef4444; }

.selected-day-panel {
  padding: 1.5rem;
  border-radius: 20px;
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.panel-date {
  margin: 0;
  font-size: 1.2rem;
  color: var(--text-main);
}

.empty-state {
  text-align: center;
  color: var(--text-muted);
  font-size: 0.95rem;
  padding: 2rem 0;
}

.day-section h4 {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0 0 10px 0;
  font-size: 0.95rem;
  color: var(--text-muted);
}

.day-item {
  background: color-mix(in srgb, var(--panel-bg) 72%, transparent);
  border-radius: 12px;
  padding: 12px 16px;
  margin-bottom: 8px;
  border-left: 4px solid transparent;
}

.exam-item { border-left-color: #ef4444; }
.todo-item { border-left-color: #f97316; }

.item-title {
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--text-main);
}

.item-desc {
  font-size: 0.8rem;
  color: var(--text-muted);
  margin-top: 4px;
}

/* ═══════════════════════════════════════════════════════ */
/*             COMPREHENSIVE LIGHT MODE OVERRIDES          */
/* ═══════════════════════════════════════════════════════ */
:global(html[data-theme='light']) .calendar-view {
  color: #1e293b;
}
:global(html[data-theme='light']) .cal-header h1,
:global(html[data-theme='light']) .week-label,
:global(html[data-theme='light']) .month-label {
  color: #1e293b;
}
:global(html[data-theme='light']) .week-btn {
  background: rgba(0,0,0,0.06);
  color: #334155;
  border-color: rgba(0,0,0,0.08);
}
:global(html[data-theme='light']) .week-btn:hover {
  background: rgba(0,0,0,0.10);
}
:global(html[data-theme='light']) .schedule-grid {
  border-color: rgba(0,0,0,0.06);
}
:global(html[data-theme='light']) .grid-day-header {
  color: #334155;
}
:global(html[data-theme='light']) .grid-day-header .day-num {
  color: #1e293b;
}
:global(html[data-theme='light']) .grid-cell {
  border-color: rgba(0,0,0,0.04);
}
:global(html[data-theme='light']) .grid-period-label .period-num {
  color: #334155;
}
:global(html[data-theme='light']) .grid-period-label .period-time {
  color: #64748b;
}
:global(html[data-theme='light']) .course-block {
  box-shadow: 0 2px 8px rgba(0,0,0,0.08);
}
:global(html[data-theme='light']) .course-block .course-name {
  color: #fff;  /* keep white on colored accent bg */
}
:global(html[data-theme='light']) .course-block .course-loc {
  color: rgba(255,255,255,0.85);
}
:global(html[data-theme='light']) .today-summary h3 {
  color: #1e293b;
}
:global(html[data-theme='light']) .today-item .today-name {
  color: #1e293b;
}
:global(html[data-theme='light']) .today-item .today-time,
:global(html[data-theme='light']) .today-item .today-loc {
  color: #64748b;
}

/* Glass panel in light mode */
:global(html[data-theme='light']) .glass-panel {
  background: rgba(255, 255, 255, 0.8);
  border-color: rgba(0, 0, 0, 0.08);
  box-shadow: 0 8px 32px rgba(0,0,0,0.08);
}
:global(html[data-theme='light']) .modal-overlay {
  background: rgba(0,0,0,0.25);
}
:global(html[data-theme='light']) .modal-content h3 {
  color: #1e293b;
}
:global(html[data-theme='light']) .modal-content p {
  color: #64748b !important;
}

/* Detail rows light mode */
:global(html[data-theme='light']) .detail-row {
  background: rgba(0,0,0,0.03);
  border-color: rgba(0,0,0,0.05);
}
:global(html[data-theme='light']) .detail-label {
  color: #64748b;
}
:global(html[data-theme='light']) .detail-value {
  color: #1e293b;
}

/* Month grid light mode */
:global(html[data-theme='light']) .month-cell {
  background: rgba(255,255,255,0.6);
  border-color: rgba(0,0,0,0.06);
}
:global(html[data-theme='light']) .month-cell:not(.empty):hover {
  background: rgba(0,0,0,0.04);
}
:global(html[data-theme='light']) .month-day-num {
  color: #1e293b;
}
:global(html[data-theme='light']) .month-day-header {
  color: #64748b;
}
:global(html[data-theme='light']) .month-cell.selected {
  background: #0284c7;
  border-color: #0284c7;
}
:global(html[data-theme='light']) .month-cell.selected .month-day-num {
  color: #fff;
}
:global(html[data-theme='light']) .month-cell.in-week {
  background: rgba(2, 132, 199, 0.08);
  border-color: rgba(2, 132, 199, 0.15);
}
:global(html[data-theme='light']) .month-cell.is-today .month-day-num {
  background: #0284c7;
  color: #fff;
}

/* Selected day panel light mode */
:global(html[data-theme='light']) .panel-date {
  color: #1e293b;
}
:global(html[data-theme='light']) .empty-state {
  color: #64748b;
}
:global(html[data-theme='light']) .day-section h4 {
  color: #334155;
}
:global(html[data-theme='light']) .day-item {
  background: rgba(0,0,0,0.03);
}
:global(html[data-theme='light']) .item-title {
  color: #1e293b;
}
:global(html[data-theme='light']) .item-desc {
  color: #64748b;
}

/* Floating Toggle Switch for Course info */
.hide-course-settings {
  position: fixed;
  bottom: 24px;
  right: 24px;
  padding: 12px 20px;
  display: flex;
  align-items: center;
  gap: 12px;
  z-index: 100;
  border-radius: 999px;
  box-shadow: 0 8px 24px rgba(0,0,0,0.15);
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}
.hide-course-settings span {
  font-size: 0.9rem;
  font-weight: 600;
  color: var(--text-main);
}
.hide-course-settings:hover {
  transform: translateY(-2px);
  box-shadow: 0 12px 28px rgba(0,0,0,0.2);
}

.toggle-switch {
  width: 50px;
  height: 28px;
  background: rgba(255, 255, 255, 0.2);
  border-radius: 14px;
  position: relative;
  cursor: pointer;
  transition: background 0.3s;
  flex-shrink: 0;
}
.toggle-switch.active {
  background: #38bdf8;
}
.toggle-knob {
  width: 24px;
  height: 24px;
  background: #fff;
  border-radius: 50%;
  position: absolute;
  top: 2px;
  left: 2px;
  transition: transform 0.3s cubic-bezier(0.4, 0.0, 0.2, 1);
  box-shadow: 0 2px 4px rgba(0,0,0,0.2);
}
.toggle-switch.active .toggle-knob {
  transform: translateX(22px);
}
:global(html[data-theme='light']) .toggle-switch {
  background: rgba(0,0,0,0.15);
}

.action-icon-btn {
  background: transparent;
  color: var(--text-muted);
  border: none;
  padding: 8px;
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
}
.action-icon-btn.primary-icon-btn {
  background: #0ea5e9;
  color: white;
}
.action-icon-btn:hover {
  background: rgba(14, 165, 233, 0.1);
}
.action-icon-btn.primary-icon-btn:hover {
  background: #0284c7;
}

:global(html[data-theme='light']) .action-icon-btn {
  color: #64748b;
}
:global(html[data-theme='light']) .action-icon-btn:hover {
  background: rgba(0,0,0,0.05);
}
:global(html[data-theme='light']) .action-icon-btn.primary-icon-btn {
  background: #0ea5e9;
  color: white;
}
:global(html[data-theme='light']) .action-icon-btn.primary-icon-btn:hover {
  background: #0284c7;
}

.fade-in {
  animation: fadeIn 0.4s ease-out forwards;
}
@keyframes fadeIn {
  from { opacity: 0; transform: translateY(10px); }
  to { opacity: 1; transform: translateY(0); }
}

@media (max-width: 600px) {
  .calendar-view { padding: 1rem 0.8rem 6rem; }
  .month-grid { gap: 4px; padding: 1rem; }
  .month-cell { border-radius: 8px; }
  .month-day-num { font-size: 0.95rem; }
  .debug-grid { grid-template-columns: 1fr; }
}

/* ═══════════════════════════════════════════════════════ */
/*              MOBILE TIMETABLE RESPONSIVE               */
/* ═══════════════════════════════════════════════════════ */
@media (max-width: 768px) {
  /* Header: stack vertically */
  .cal-header {
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
    margin-bottom: 1.2rem;
  }
  .cal-actions-section {
    width: 100%;
    justify-content: space-between;
  }
  .week-selector-pill {
    flex-grow: 1;
    justify-content: center;
  }
  .pill-label {
    font-size: 0.75rem;
  }
  .month-label-large {
    font-size: 1.3rem;
  }
  .debug-grid {
    grid-template-columns: 1fr;
  }

  /* Grid: remove min-width, show 5 days only */
  .schedule-grid {
    grid-template-columns: 28px repeat(7, 1fr) !important;
    min-width: 0 !important;
  }
  .schedule-grid-container {
    overflow-x: auto;
    -webkit-overflow-scrolling: touch;
    border-radius: 14px;
  }

  /* Compact period labels */
  .grid-period-label {
    padding: 2px;
  }
  .period-num {
    font-size: 0.6rem !important;
  }
  .period-time {
    display: none;
  }

  /* Day headers compact */
  .grid-day-header {
    padding: 4px 2px;
    font-size: 0.75rem;
  }
  .day-num {
    font-size: 0.6rem !important;
  }
  .day-name {
    font-size: 0.7rem !important;
  }

  /* Grid cells compact */
  .grid-cell {
    min-height: 32px !important;
  }

  /* Course blocks: touch-optimized */
  .course-block {
    padding: 2px 3px;
    border-radius: 4px;
    margin: 0 1px;
  }
  .course-name {
    font-size: 0.58rem !important;
    line-height: 1.15;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .course-loc {
    display: none !important;
  }

  /* Touch feedback for course blocks */
  .course-block:active {
    transform: scale(0.93) !important;
    opacity: 0.85;
    transition: transform 0.08s, opacity 0.08s;
  }

  /* Today summary compact */
  .today-summary {
    padding: 0.8rem 1rem;
    border-radius: 14px;
  }
  .today-item {
    min-width: 130px;
    padding: 0.6rem 0.8rem;
  }

  /* Modal → Bottom Sheet */
  .modal-overlay {
    align-items: flex-end !important;
  }
  .modal-content.glass-panel {
    max-width: 100% !important;
    width: 100% !important;
    border-radius: 20px 20px 0 0 !important;
    margin: 0 !important;
    max-height: 70vh;
    overflow-y: auto;
    animation: slideUpSheet 0.3s ease-out;
  }

  /* View toggle buttons */
  .view-toggle {
    margin-right: 6px !important;
  }
  .toggle-icon-btn {
    padding: 4px 8px;
  }

  /* Month grid compact */
  .month-grid-container {
    grid-template-columns: 1fr !important;
  }
  .selected-day-panel {
    padding: 1rem;
  }
}

@keyframes slideUpSheet {
  from { transform: translateY(100%); opacity: 0; }
  to { transform: translateY(0); opacity: 1; }
}

/* Touch feedback for all interactive course blocks (all viewports) */
.course-block:active {
  transform: scale(0.96);
  opacity: 0.9;
  transition: transform 0.08s, opacity 0.08s;
}
</style>
