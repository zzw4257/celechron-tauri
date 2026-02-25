<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { LayoutGrid, CalendarDays, BookOpen, Clock, AlertTriangle, MapPin, User, CalendarRange, Hash } from "lucide-vue-next";

const weekDays = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
// ZJU 标准 12 节课时间表 (每节独立)
const periods = [
  { label: "第1节", time: "08:00-08:50" },
  { label: "第2节", time: "08:55-09:35" },
  { label: "第3节", time: "09:50-10:35" },
  { label: "第4节", time: "10:40-11:25" },
  { label: "第5节", time: "11:30-12:15" },
  { label: "第6节", time: "13:15-14:00" },
  { label: "第7节", time: "14:05-14:50" },
  { label: "第8节", time: "15:05-15:50" },
  { label: "第9节", time: "15:55-16:40" },
  { label: "第10节", time: "16:55-17:40" },
  { label: "第11节", time: "18:50-19:35" },
  { label: "第12节", time: "19:40-20:25" },
];

interface CourseSlot {
  name: string;
  location: string;
  teacher: string;
  xkkh: string;
  color: string;
  dayIdx: number;   // 0-6 (Mon-Sun)
  periodIdx: number; // 0-11 (直接对应第1-12节)
  span: number;      // 跨几节课 (通常 1, 2, 或 3)
  activeWeeks: number[];
}

const colors = ["#06b6d4", "#8b5cf6", "#f97316", "#22c55e", "#ec4899", "#eab308", "#14b8a6", "#3b82f6", "#ef4444"];

const allCourses = ref<CourseSlot[]>([]);
const allTodos = ref<any[]>([]);
const allExams = ref<any[]>([]);

const viewMode = ref<'week'|'month'>('week');
const selectedDate = ref(new Date());

const totalWeeks = ref(18); // Typical ZJU semester is 16-18 weeks
const isLoading = ref(true);
const isOffline = ref(false);
const offlineTime = ref("");

// Course detail modal
const selectedCourse = ref<CourseSlot | null>(null);
const showCourseDetail = ref(false);

function openCourseDetail(course: CourseSlot) {
  selectedCourse.value = course;
  showCourseDetail.value = true;
}

// --- Calibration Logic ---
const showCalibrateModal = ref(false);
const calibrateWeekInput = ref(1);

// Monday of Week 1
const startDateMs = ref<number>(
  parseInt(localStorage.getItem('semester_start_ms') || '0')
);

// Get current actual date

// Get current actual date
if (startDateMs.value === 0) {
  const now = new Date();
  const year = now.getFullYear();
  const month = now.getMonth() + 1; // 1-12
  let defaultStart = new Date();
  
  if (month >= 2 && month <= 7) {
    // Spring semester, usually starts late Feb (e.g. Feb 23 for 2026)
    defaultStart = new Date(`${year}-02-23T00:00:00`);
  } else {
    // Fall semester, usually starts early Sept
    const startYear = month < 2 ? year - 1 : year;
    defaultStart = new Date(`${startYear}-09-09T00:00:00`);
  }
  
  const day = defaultStart.getDay() || 7;
  defaultStart.setDate(defaultStart.getDate() - day + 1);
  startDateMs.value = defaultStart.getTime();
  localStorage.setItem('semester_start_ms', startDateMs.value.toString());
}

// Compute the "real" current week from today's date
// NOTE: Can be negative if semester hasn't started yet!
function getRealCurrentWeek() {
  const diff = Date.now() - startDateMs.value;
  const w = Math.floor(diff / (7 * 24 * 60 * 60 * 1000)) + 1;
  return w; // Allow negative weeks for pre-semester viewing
}

const currentWeek = ref(getRealCurrentWeek());

// Semester info display
const semesterLabel = computed(() => {
  const now = new Date();
  const month = now.getMonth() + 1;
  const year = now.getFullYear();
  if (month >= 2 && month <= 8) {
    return `${year - 1}-${year} 春夏学期`;
  } else {
    const startYear = month === 1 ? year - 1 : year;
    return `${startYear}-${startYear + 1} 秋冬学期`;
  }
});

const semesterStartDateStr = computed(() => {
  const d = new Date(startDateMs.value);
  return `${d.getFullYear()}-${(d.getMonth()+1).toString().padStart(2,'0')}-${d.getDate().toString().padStart(2,'0')}`;
});

// Derived Monday Date for the 'currentWeek' user is viewing
const viewedMondayDate = computed(() => {
  return new Date(startDateMs.value + (currentWeek.value - 1) * 7 * 24 * 60 * 60 * 1000);
});

// String like "2024 年 3 月"
const currentMonthStr = computed(() => {
  const d = viewedMondayDate.value;
  return `${d.getFullYear()} 年 ${d.getMonth() + 1} 月`;
});

// Returns ["03/12 周一", "03/13 周二"...] based on current week
const dynamicWeekDays = computed(() => {
  const arr = [];
  for (let i = 0; i < 7; i++) {
    const d = new Date(viewedMondayDate.value.getTime() + i * 24 * 60 * 60 * 1000);
    const mm = (d.getMonth() + 1).toString().padStart(2, '0');
    const dd = d.getDate().toString().padStart(2, '0');
    arr.push(`${mm}/${dd} ${weekDays[i]}`);
  }
  return arr;
});

function confirmCalibration() {
  // User says THIS week is 'calibrateWeekInput'
  // So Monday of THIS week = today's Monday
  const today = new Date();
  const day = today.getDay() || 7;
  const thisMonday = new Date(today);
  thisMonday.setDate(today.getDate() - day + 1);
  thisMonday.setHours(0, 0, 0, 0);

  // So Monday of Week 1 is thisMonday - (calibrateWeekInput - 1) weeks
  const newStartMs = thisMonday.getTime() - (calibrateWeekInput.value - 1) * 7 * 24 * 60 * 60 * 1000;
  startDateMs.value = newStartMs;
  localStorage.setItem('semester_start_ms', newStartMs.toString());
  
  currentWeek.value = calibrateWeekInput.value;
  showCalibrateModal.value = false;
}

// 当前周的所有课程 (带完美的连通图重叠处理定位)
const currentWeekCourses = computed(() => {
  const weekCourses = allCourses.value.filter(c => c.activeWeeks.includes(currentWeek.value));
  const positioned: (CourseSlot & { overlapCount: number; overlapIndex: number })[] = [];
  
  for (let d = 0; d < 7; d++) {
    const dayCourses = weekCourses.filter(c => c.dayIdx === d);
    if (!dayCourses.length) continue;

    // 1. Sort courses by start time, then span descending
    dayCourses.sort((a, b) => {
      if (a.periodIdx !== b.periodIdx) return a.periodIdx - b.periodIdx;
      return b.span - a.span;
    });

    // 2. Cluster overlapping courses (connected components)
    const clusters: CourseSlot[][] = [];
    dayCourses.forEach(c => {
      let addedToCluster = false;
      for (const cluster of clusters) {
        const overlaps = cluster.some(existing => 
          Math.max(c.periodIdx, existing.periodIdx) < Math.min(c.periodIdx + c.span, existing.periodIdx + existing.span)
        );
        if (overlaps) {
          cluster.push(c);
          addedToCluster = true;
          break;
        }
      }
      if (!addedToCluster) {
        clusters.push([c]);
      }
    });

    // 3. For each cluster, use greedy coloring to assign columns
    clusters.forEach(cluster => {
      const columns: CourseSlot[][] = [];
      cluster.forEach(c => {
        let placed = false;
        for (let i = 0; i < columns.length; i++) {
          const overlaps = columns[i].some(existing => 
            Math.max(c.periodIdx, existing.periodIdx) < Math.min(c.periodIdx + c.span, existing.periodIdx + existing.span)
          );
          if (!overlaps) {
            columns[i].push(c);
            (c as any)._col = i;
            placed = true;
            break;
          }
        }
        if (!placed) {
          columns.push([c]);
          (c as any)._col = columns.length - 1;
        }
      });

      // 4. Assign finalized calculated positioning
      const maxCols = columns.length;
      cluster.forEach(c => {
        positioned.push({
          ...c,
          overlapCount: maxCols,
          overlapIndex: (c as any)._col
        });
      });
    });
  }
  
  return positioned;
});

const todayCourses = computed(() => {
  const day = new Date().getDay(); // 0=Sun, 1=Mon...
  const idx = day === 0 ? 6 : day - 1;
  return currentWeekCourses.value
    .filter(c => c.dayIdx === idx)
    .sort((a, b) => a.periodIdx - b.periodIdx)
    .map(c => ({ ...c, period: periods[c.periodIdx]?.label || '', time: periods[c.periodIdx]?.time || '' }));
});

// --- Month View Logic ---
// monthViewDate is now derived from the week selector
const monthViewDate = computed(() => viewedMondayDate.value);

const monthDays = computed(() => {
  const year = monthViewDate.value.getFullYear();
  const month = monthViewDate.value.getMonth();
  const daysInMonth = new Date(year, month + 1, 0).getDate();
  const firstDay = new Date(year, month, 1).getDay(); // 0 (Sun) to 6 (Sat)
  const offset = firstDay === 0 ? 6 : firstDay - 1; // Mon to Sun mapping
  
  const days = [];
  for (let i = 0; i < offset; i++) {
    days.push({ empty: true });
  }
  for (let i = 1; i <= daysInMonth; i++) {
    const d = new Date(year, month, i);
    days.push({ empty: false, date: d, dayNum: i });
  }
  return days;
});

const monthDisplayStr = computed(() => {
  return `${monthViewDate.value.getFullYear()} 年 ${monthViewDate.value.getMonth() + 1} 月`;
});

function isInViewedWeek(date: Date): boolean {
  const monday = viewedMondayDate.value.getTime();
  const sunday = monday + 6 * 24 * 60 * 60 * 1000;
  const d = date.getTime();
  return d >= monday && d <= sunday + 23 * 60 * 60 * 1000;
}

function isToday(date: Date): boolean {
  const now = new Date();
  return date.getFullYear() === now.getFullYear() && date.getMonth() === now.getMonth() && date.getDate() === now.getDate();
}

function getWeekForDate(date: Date) {
  const dLocal = new Date(date).setHours(0,0,0,0);
  const diff = dLocal - startDateMs.value;
  return Math.floor(diff / (7 * 24 * 60 * 60 * 1000)) + 1;
}

const selectedDayData = computed(() => {
  if (!selectedDate.value) return { courses: [], exams: [], todos: [] };
  
  const d = selectedDate.value;
  const w = getWeekForDate(d);
  const dayOfWeek = d.getDay() === 0 ? 6 : d.getDay() - 1; // 0-6
  
  // 1. Courses
  const dayCourses = allCourses.value.filter(c => c.activeWeeks.includes(w) && c.dayIdx === dayOfWeek);
  
  // 2. Exams
  const dayTimeStart = new Date(d).setHours(0,0,0,0);
  const dayTimeEnd = new Date(d).setHours(23,59,59,999);
  
  const exams = allExams.value.filter(e => {
    const timeStr = e.kssj || e.qzkssj || (e.time ? e.time[0] : '');
    const match = timeStr.match(/(\d{4})年(\d{2})月(\d{2})日/);
    if(match) {
        const examMs = new Date(`${match[1]}-${match[2]}-${match[3]}T12:00:00`).getTime();
        return examMs >= dayTimeStart && examMs <= dayTimeEnd;
    }
    return false;
  });
  
  // 3. Todos
  const todos = allTodos.value.filter(t => {
     if(!t.expires) return false;
     const tMs = new Date(t.expires).getTime();
     return tMs >= dayTimeStart && tMs <= dayTimeEnd;
  });

  return { courses: dayCourses, exams, todos };
});

async function fetchExtraData() {
    try {
        const tr: any = await invoke("fetch_todos", { sync: false });
        allTodos.value = tr.data || [];
        const sr: any = await invoke("fetch_scholar_data", { sync: false });
        allExams.value = sr.exams || [];
    } catch(e) {}
}

async function fetchTimetable() {
  try {
    isLoading.value = true;
    
    // Dynamic semester calculation for ZJU
    // Academic Year runs Fall -> Spring. E.g., Fall 2025 -> Spring 2026 is Year "2025".
    // Fall (Sem 1) typically starts ~September. Spring (Sem 2) typically starts ~February.
    const now = new Date();
    const currentMonth = now.getMonth() + 1;
    const currentYear = now.getFullYear();
    let zjuYearStr = "";
    let zjuSemStr = "";
    
    // ZJU 教务系统 API 编码: xqm=12 表示春夏学期, xqm=3 表示秋冬学期
    // Feb (2) to Aug (8) -> 春夏学期 (属于上一年开始的学年)
    if (currentMonth >= 2 && currentMonth <= 8) {
        zjuYearStr = (currentYear - 1).toString(); // e.g., Feb 2026 -> xnm=2025
        zjuSemStr = "12"; // 春夏学期 xqm=12
    } else {
        // Sep (9) to Jan (1) -> 秋冬学期
        if (currentMonth === 1) {
             zjuYearStr = (currentYear - 1).toString();
        } else {
             zjuYearStr = currentYear.toString();
        }
        zjuSemStr = "3"; // 秋冬学期 xqm=3
    }

    const response: any = await invoke("fetch_timetable", { year: zjuYearStr, semester: zjuSemStr });
    const data: any[] = response.timetable || [];

    if (response._meta && response._meta.source === "cache") {
      isOffline.value = true;
      offlineTime.value = new Date(response._meta.timestamp * 1000).toLocaleString('zh-CN', { hour12: false });
    } else {
      isOffline.value = false;
    }
    
    // ── DEBUG: print first 5 raw sessions to console ──────────────────
    console.log('[CalendarView] RAW API response meta:', response._meta);
    console.log('[CalendarView] Total sessions:', data.length);
    // ───────────────────────────────────────────────────────────────────

    // Clear timetable
    allCourses.value = [];

    let colorIdx = 0;
    const courseColors: Record<string, string> = {};

    // 严谨的防跨学期防串台机制：通过 xkkh 字段过滤
    const targetXkkhPrefix = `(${zjuYearStr}-${parseInt(zjuYearStr) + 1}-${zjuSemStr === '12' ? '2' : '1'})`;
    console.log('[CalendarView] Target xkkh Prefix:', targetXkkhPrefix);

    data.forEach((session: any) => {
      // Skip graduate courses — same as Flutter: sfyjskc !== "1"
      if (session.sfyjskc === '1') return;
      
      // CRITICAL BUG FIX: The ZJU API frequently ignores xnm/xqm and returns courses 
      // from multiple past & future semesters. We MUST strictly match the target xkkh prefix.
      const xkkh = (session.xkkh || '').trim();
      if (!xkkh.startsWith(targetXkkhPrefix)) return;

      const kcb: string = session.kcb || '';

      // 1. Course name — prefer structured field kcmc, fallback to kcb HTML
      let name = (session.kcmc || '').trim();
      if (!name && kcb) {
        name = kcb.split('<br>')[0].replace(/<[^>]+>/g, '').trim();
      }
      if (!name) return;

      // 2. Location — prefer cdmc, fallback to kcb HTML
      let loc = (session.cdmc || '').trim();
      let teacher = '';
      if (kcb) {
        const ps = kcb.split('<br>');
        // Flutter regex: (.*?)<br>(.*?)<br>(.*?)<br>(.*?)zwf
        // group 1=name, group 2=semester info, group 3=teacher, group 4=location
        if (ps.length > 2) teacher = ps[2].replace(/<[^>]+>/g, '').trim();
        if (!loc && ps.length > 3) loc = ps[3].replace(/<[^>]+>/g, '').split('zwf')[0].trim();
        else if (!loc && ps.length > 2) loc = ps[2].replace(/<[^>]+>/g, '').trim();
      }

      // 3. Day-of-week — xqj: "1"=Mon … "7"=Sun (same as Flutter dayOfWeek)
      const dayStr = session.xqj || session.xq;
      if (!dayStr) return;
      const dayIdx = parseInt(dayStr) - 1;
      if (dayIdx < 0 || dayIdx > 6) return;

      // 4. Period — djj = starting period (1-indexed), skcd = duration
      const startPeriod = parseInt(session.djj || '0') || 0;
      const span        = parseInt(session.skcd || '2') || 2;
      if (startPeriod <= 0) return;

      // 5. Active weeks parsing
      // Parse base weeks from '第xxx周' in kcb 
      // ZJU undergrad courses usually represent weeks relative to the half-semester (1-8).
      let parsedWeeks = [1, 2, 3, 4, 5, 6, 7, 8]; 
      const timeInfo = kcb.split('<br>')[1] || '';
      const weekMatch = timeInfo.match(/第([0-9,\-]+)周/);
      if (weekMatch && weekMatch[1]) {
        parsedWeeks = [];
        const parts = weekMatch[1].replace(/[^\d,\-]/g, '').split(',');
        parts.forEach((p: string) => {
          const r = p.split('-').map(Number);
          if (r.length === 2 && !isNaN(r[0]) && !isNaN(r[1]) && r[1] >= r[0]) {
            for (let w = r[0]; w <= r[1]; w++) parsedWeeks.push(w);
          } else if (r.length === 1 && !isNaN(r[0]) && r[0] > 0) {
            parsedWeeks.push(r[0]);
          }
        });
      }

      // Determine which half-semester this applies to, using xxq field
      const xxq = (session.xxq || '').trim();
      const firstHalf  = xxq.includes('秋') || xxq.includes('春');
      const secondHalf = xxq.includes('冬') || xxq.includes('夏');
      
      const absoluteWeeks: number[] = [];
      if (firstHalf) {
        // Appends unmodified relative weeks
        absoluteWeeks.push(...parsedWeeks);
      }
      if (secondHalf) {
        // Appends relative weeks shifted by 8
        absoluteWeeks.push(...parsedWeeks.map(w => w + 8));
      }

      // Apply odd/even filtering using dsz
      const dsz = (session.dsz || '').trim();
      const oddOnly    = dsz === '1';
      const evenOnly   = dsz === '0';

      let activeWeeks = absoluteWeeks;
      if (oddOnly)  activeWeeks = activeWeeks.filter(w => w % 2 !== 0);
      if (evenOnly) activeWeeks = activeWeeks.filter(w => w % 2 === 0);

      // If still empty, skip
      if (activeWeeks.length === 0) return;

      // 6. Color
      if (!courseColors[name]) {
        courseColors[name] = colors[colorIdx % colors.length];
        colorIdx++;
      }

      const periodIdx = startPeriod - 1;
      if (periodIdx >= 0 && periodIdx < 12) {
        allCourses.value.push({
          name,
          location: loc,
          teacher,
          xkkh,
          color: courseColors[name],
          dayIdx,
          periodIdx,
          span,
          activeWeeks: [...activeWeeks],
        });
      }
    });

  } catch (e) {
    console.error("Failed to fetch timetable:", e);
  } finally {
    isLoading.value = false;
  }
}

// Format week array [1,2,3,5,6,9] into readable '1-3, 5-6, 9'
function formatWeekRanges(weeks: number[]): string {
  if (!weeks.length) return '无';
  const sorted = [...weeks].sort((a, b) => a - b);
  const ranges: string[] = [];
  let start = sorted[0], end = sorted[0];
  for (let i = 1; i < sorted.length; i++) {
    if (sorted[i] === end + 1) {
      end = sorted[i];
    } else {
      ranges.push(start === end ? `${start}` : `${start}-${end}`);
      start = end = sorted[i];
    }
  }
  ranges.push(start === end ? `${start}` : `${start}-${end}`);
  return '第 ' + ranges.join(', ') + ' 周';
}

onMounted(() => {
  fetchTimetable();
  fetchExtraData();
});
</script>

<template>
  <div class="calendar-view">
    <header class="cal-header">
      <div class="cal-title-section">
        <h1>日程</h1>
        <span class="month-label">{{ viewMode === 'week' ? currentMonthStr : monthDisplayStr }}</span>
      </div>
      <div class="week-selector">
        <div class="view-toggle" style="display: flex; gap: 4px; margin-right: 12px; background: rgba(255,255,255,0.05); padding: 4px; border-radius: 8px;">
          <button class="toggle-icon-btn" :class="{active: viewMode === 'week'}" @click="viewMode = 'week'"><LayoutGrid :size="18"/></button>
          <button class="toggle-icon-btn" :class="{active: viewMode === 'month'}" @click="viewMode = 'month'"><CalendarDays :size="18"/></button>
        </div>
        <template v-if="viewMode === 'week'">
          <button class="week-btn" @click="currentWeek = currentWeek - 1">‹</button>
          <span class="week-label" @click="calibrateWeekInput = getRealCurrentWeek(); showCalibrateModal = true" style="cursor: pointer; text-decoration: underline dashed rgba(255,255,255,0.3); text-underline-offset: 4px;" title="点击校准周数">
            {{ semesterLabel }} · 第 {{ currentWeek }} 周 / {{ totalWeeks }}
          </span>
          <button class="week-btn" @click="currentWeek = currentWeek + 1">›</button>
        </template>
        <template v-else>
          <button class="week-btn" @click="currentWeek = currentWeek - 4">‹</button>
          <span class="week-label" @click="calibrateWeekInput = getRealCurrentWeek(); showCalibrateModal = true" style="cursor: pointer; text-decoration: underline dashed rgba(255,255,255,0.3); text-underline-offset: 4px;" title="点击校准周数">
            {{ monthDisplayStr }} · 第 {{ currentWeek }} 周
          </span>
          <button class="week-btn" @click="currentWeek = currentWeek + 4">›</button>
        </template>
      </div>
    </header>

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
    <section v-if="viewMode === 'week'" class="schedule-grid-container">
      <div class="schedule-grid">
        <!-- Row 1: Corner + Day Headers (auto-placed) -->
        <div class="grid-corner" :style="{ gridRow: 1, gridColumn: 1 }"></div>
        <div v-for="(day, di) in dynamicWeekDays" :key="day" class="grid-day-header" :style="{ gridRow: 1, gridColumn: di + 2 }">
          <div class="day-num">{{ day.split(' ')[0] }}</div>
          <div class="day-name">{{ day.split(' ')[1] }}</div>
        </div>

        <!-- Rows 2-13: Period labels + empty cells (explicitly placed) -->
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
          <span class="course-name">{{ course.name }}</span>
          <span class="course-loc">{{ course.location }}</span>
        </div>
      </div>
    </section>

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
              ({{ periods[selectedCourse.periodIdx]?.time.split('-')[0] }}-{{ periods[selectedCourse.periodIdx + selectedCourse.span - 1]?.time.split('-')[1] }})
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
    <section v-else class="month-grid-container fade-in">
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
            <div v-if="allCourses.some(c => c.activeWeeks.includes(getWeekForDate(dayObj.date!)) && c.dayIdx === (dayObj.date!.getDay()===0?6:dayObj.date!.getDay()-1))" class="dot dot-course"></div>
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
  color: #fff;
}
.cal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 2rem;
}
.cal-title-section {
  display: flex;
  align-items: baseline;
  gap: 15px;
}
.cal-title-section h1 {
  margin: 0;
  font-size: 2rem;
  font-weight: 700;
  color: #e2e8f0;
}
.month-label {
  font-size: 1.1rem;
  color: #38bdf8;
  font-weight: 600;
}
.week-selector {
  display: flex;
  align-items: center;
  background: rgba(255,255,255,.05);
  border: 1px solid rgba(255,255,255,.1);
  border-radius: 12px;
  padding: 4px;
}
.week-btn {
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
  border-radius: 8px;
  transition: all .2s;
}
.week-btn:hover { background: rgba(255,255,255,.1); color: #fff; }
.week-label {
  padding: 0 15px;
  font-size: 1rem;
  font-weight: 600;
  color: #e2e8f0;
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
.btn-primary {
  background: linear-gradient(135deg, #38bdf8 0%, #0284c7 100%);
  color: #fff;
  border: 1px solid rgba(255,255,255,0.1);
  border-radius: 12px;
  cursor: pointer;
  font-weight: 600;
  transition: transform 0.2s;
}
.btn-primary:active { transform: translateY(2px); }

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
  color: #94a3b8;
  white-space: nowrap;
  margin-right: 1rem;
}
.detail-value {
  font-size: 0.9rem;
  color: #e2e8f0;
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
  color: #e2e8f0;
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
.today-name { font-weight: 700; font-size: .88rem; color: #e2e8f0; }
.today-time { font-size: .75rem; color: #94a3b8; }
.today-loc { font-size: .75rem; color: #64748b; }

/* Schedule Grid */
.schedule-grid-container {
  overflow-x: auto;
  border-radius: 20px;
  border: 1px solid rgba(255,255,255,.08);
  background: rgba(255,255,255,.03);
}
.schedule-grid {
  display: grid;
  grid-template-columns: 70px repeat(7, minmax(90px, 1fr));
  grid-template-rows: auto repeat(12, minmax(42px, auto));
  min-width: 800px;
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
.course-name { font-size: .72rem; font-weight: 700; color: var(--text-main); line-height: 1.2; }
.course-loc { font-size: .6rem; color: var(--text-muted); font-weight: 500; }

/* Month Grid Styles */
.month-grid-container {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
  margin-top: 1.5rem;
  padding-top: 1.5rem;
  border-top: 1px solid rgba(255, 255, 255, 0.1);
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
  background: rgba(0,0,0,0.2);
  border-radius: 12px;
  padding: 12px 16px;
  margin-bottom: 8px;
  border-left: 4px solid transparent;
}
:root.light-theme .day-item {
  background: rgba(0,0,0,0.03);
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
:root.light-theme .calendar-view {
  color: #1e293b;
}
:root.light-theme .cal-header h1,
:root.light-theme .week-label,
:root.light-theme .month-label {
  color: #1e293b;
}
:root.light-theme .week-btn {
  background: rgba(0,0,0,0.06);
  color: #334155;
  border-color: rgba(0,0,0,0.08);
}
:root.light-theme .week-btn:hover {
  background: rgba(0,0,0,0.10);
}
:root.light-theme .schedule-grid {
  border-color: rgba(0,0,0,0.06);
}
:root.light-theme .grid-day-header {
  color: #334155;
}
:root.light-theme .grid-day-header .day-num {
  color: #1e293b;
}
:root.light-theme .grid-cell {
  border-color: rgba(0,0,0,0.04);
}
:root.light-theme .grid-period-label .period-num {
  color: #334155;
}
:root.light-theme .grid-period-label .period-time {
  color: #64748b;
}
:root.light-theme .course-block {
  box-shadow: 0 2px 8px rgba(0,0,0,0.08);
}
:root.light-theme .course-block .course-name {
  color: #fff;  /* keep white on colored accent bg */
}
:root.light-theme .course-block .course-loc {
  color: rgba(255,255,255,0.85);
}
:root.light-theme .today-summary h3 {
  color: #1e293b;
}
:root.light-theme .today-item .today-name {
  color: #1e293b;
}
:root.light-theme .today-item .today-time,
:root.light-theme .today-item .today-loc {
  color: #64748b;
}

/* Glass panel in light mode */
:root.light-theme .glass-panel {
  background: rgba(255, 255, 255, 0.8);
  border-color: rgba(0, 0, 0, 0.08);
  box-shadow: 0 8px 32px rgba(0,0,0,0.08);
}
:root.light-theme .modal-overlay {
  background: rgba(0,0,0,0.25);
}
:root.light-theme .modal-content h3 {
  color: #1e293b;
}
:root.light-theme .modal-content p {
  color: #64748b !important;
}

/* Detail rows light mode */
:root.light-theme .detail-row {
  background: rgba(0,0,0,0.03);
  border-color: rgba(0,0,0,0.05);
}
:root.light-theme .detail-label {
  color: #64748b;
}
:root.light-theme .detail-value {
  color: #1e293b;
}

/* Month grid light mode */
:root.light-theme .month-cell {
  background: rgba(255,255,255,0.6);
  border-color: rgba(0,0,0,0.06);
}
:root.light-theme .month-cell:not(.empty):hover {
  background: rgba(0,0,0,0.04);
}
:root.light-theme .month-day-num {
  color: #1e293b;
}
:root.light-theme .month-day-header {
  color: #64748b;
}
:root.light-theme .month-cell.selected {
  background: #0284c7;
  border-color: #0284c7;
}
:root.light-theme .month-cell.selected .month-day-num {
  color: #fff;
}
:root.light-theme .month-cell.in-week {
  background: rgba(2, 132, 199, 0.08);
  border-color: rgba(2, 132, 199, 0.15);
}
:root.light-theme .month-cell.is-today .month-day-num {
  background: #0284c7;
  color: #fff;
}

/* Selected day panel light mode */
:root.light-theme .panel-date {
  color: #1e293b;
}
:root.light-theme .empty-state {
  color: #64748b;
}
:root.light-theme .day-section h4 {
  color: #334155;
}
:root.light-theme .day-item {
  background: rgba(0,0,0,0.03);
}
:root.light-theme .item-title {
  color: #1e293b;
}
:root.light-theme .item-desc {
  color: #64748b;
}

/* Toggle buttons */
:root.light-theme .toggle-icon-btn {
  color: #64748b;
}
:root.light-theme .toggle-icon-btn.active {
  background: #0284c7;
  color: #fff;
}

/* Calibration modal light overrides */
:root.light-theme .btn-primary {
  background: linear-gradient(135deg, #0ea5e9 0%, #0284c7 100%);
}

/* Offline banner light mode */
:root.light-theme .offline-banner {
  background: rgba(245,158,11,0.08);
  color: #92400e;
}

.toggle-icon-btn {
  background: transparent;
  color: var(--text-muted);
  border: none;
  padding: 6px 12px;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
}
.toggle-icon-btn.active {
  background: rgba(255,255,255,0.15);
  color: var(--text-main);
  box-shadow: 0 2px 8px rgba(0,0,0,0.1);
}

.fade-in {
  animation: fadeIn 0.4s ease-out forwards;
}
@keyframes fadeIn {
  from { opacity: 0; transform: translateY(10px); }
  to { opacity: 1; transform: translateY(0); }
}

@media (max-width: 600px) {
  .calendar-view { padding: 1rem 1rem 6rem; }
  .month-grid { gap: 4px; padding: 1rem; }
  .month-cell { border-radius: 8px; }
  .month-day-num { font-size: 0.95rem; }
}
</style>
