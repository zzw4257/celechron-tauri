<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { save } from '@tauri-apps/plugin-dialog';
import { writeTextFile } from '@tauri-apps/plugin-fs';
import { CalendarDays, BookOpen, Clock, AlertTriangle, MapPin, User, CalendarRange, Download } from "lucide-vue-next";
import { fetchScholarData, fetchTimetable as fetchTimetableApi, fetchTodos } from "../../services/api";
import {
  buildXkkhPrefix,
  normalizeAcademicSemesterCode,
  parseAcademicTermFromSemesterName,
  resolveCurrentTimetableTerm,
  toTimetableTerm,
} from "../../utils/semester";
const isDev = import.meta.env.DEV;

const weekDays = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
// ZJU 标准 12 节课时间表 (每节独立)
const periods = [
  { label: "1", time: "08:00" },
  { label: "2", time: "08:50" },
  { label: "3", time: "10:00" },
  { label: "4", time: "10:50" },
  { label: "5", time: "11:40" },
  { label: "6", time: "13:25" },
  { label: "7", time: "14:15" },
  { label: "8", time: "15:05" },
  { label: "9", time: "16:15" },
  { label: "10", time: "17:05" },
  { label: "11", time: "18:50" },
  { label: "12", time: "19:40" },
  { label: "13", time: "20:30" },
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

const showMonthNav = ref(false);
const selectedDate = ref(new Date());
const hideCourseInfo = ref(localStorage.getItem('hideCourseInfo') === 'true');

function toggleHideCourseInfo() {
  hideCourseInfo.value = !hideCourseInfo.value;
  localStorage.setItem('hideCourseInfo', hideCourseInfo.value.toString());
}

// Semester tabs

function formatSemesterName(name: string) {
  if (!name || !name.includes('-')) return name;
  const parts = name.split('-');
  if (parts.length < 3) return name;
  const startYear = parts[0].slice(-2);
  const endYear = parts[1].slice(-2);
  const semType = parts[2] === '1' ? '秋冬' : (parts[2] === '2' ? '春夏' : '短');
  return `${startYear}-${endYear} ${semType}`;
}

const semesterTabs = ref<any[]>([]);
const activeSemIdx = ref(0);

function switchSemester(idx: number) {
  activeSemIdx.value = idx;
  const tab = semesterTabs.value[idx];
  fetchTimetable(tab.year, tab.sem);
}

const totalWeeks = ref(18);
const isLoading = ref(true);
const isOffline = ref(false);
const offlineTime = ref("");
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

// 课时统计 (class hours per 2 weeks)
const classHoursStats = computed(() => {
  const courses = allCourses.value;
  if (courses.length === 0) return { total: 0, perTwoWeeks: '0' };
  // Total periods across all active weeks
  let totalPeriods = 0;
  courses.forEach(c => {
    totalPeriods += c.span * c.activeWeeks.length;
  });
  // Max week to determine semester length
  const maxWeek = Math.max(...courses.flatMap(c => c.activeWeeks), 1);
  const perTwoWeeks = maxWeek > 0 ? (totalPeriods / maxWeek * 2).toFixed(1) : '0';
  return { total: totalPeriods, perTwoWeeks };
});

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
  const activeTab = semesterTabs.value[activeSemIdx.value];
  if (activeTab?.label) {
    return activeTab.label;
  }
  const term = resolveCurrentTimetableTerm();
  const parsedYear = Number.parseInt(term.year, 10);
  const nextYear = Number.isFinite(parsedYear) ? String(parsedYear + 1) : term.year;
  const semText = term.academicSemester === '2' ? '春夏' : '秋冬';
  return `${term.year}-${nextYear} ${semText}学期`;
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
     const expires = t.expires || t.end_time;
     if(!expires) return false;
     const tMs = new Date(expires).getTime();
     return tMs >= dayTimeStart && tMs <= dayTimeEnd;
  });

  return { courses: dayCourses, exams, todos };
});

async function fetchExtraData() {
    try {
        const tr = await fetchTodos();
        allTodos.value = tr.data.todo_list || [];
        const sr = await fetchScholarData();
        allExams.value = sr.data.exams || [];
    } catch(e) {}
}

async function fetchTimetable(overrideYear?: string, overrideSem?: string) {
  isLoading.value = true;

  try {
    const defaultTerm = resolveCurrentTimetableTerm();
    const zjuYearStr = overrideYear || defaultTerm.year;
    const zjuSemStr = overrideSem || defaultTerm.timetableSemester;

    const response = await fetchTimetableApi({ year: zjuYearStr, semester: zjuSemStr });
    const responseYear = String(response.data.year || zjuYearStr);
    const responseAcademicSem =
      normalizeAcademicSemesterCode(response.data.semester || zjuSemStr) ||
      normalizeAcademicSemesterCode(zjuSemStr) ||
      '1';
    const responseTerm = toTimetableTerm({
      year: responseYear,
      academicSemester: responseAcademicSem,
    });
    
    // Auto-update metadata and class hours
    fetchScholarData().then((sr) => {
      const data: any = sr.data;
      if (data.semesters && data.semesters.length > 0) {
         const newTabs = data.semesters
           .map((s: any) => {
             const term = parseAcademicTermFromSemesterName(String(s.name || ''));
             if (!term) return null;
             const timetableTerm = toTimetableTerm(term);
             return {
               label: formatSemesterName(s.name),
               year: timetableTerm.year,
               sem: timetableTerm.timetableSemester,
               academicSem: timetableTerm.academicSemester,
             };
           })
           .filter(Boolean) as any[];
         semesterTabs.value = newTabs;
         
         // Select best matching tab
         const foundIdx = newTabs.findIndex(
           (t: any) => t.year === responseTerm.year && t.sem === responseTerm.timetableSemester
         );
         if (foundIdx !== -1) activeSemIdx.value = foundIdx;
      }
    });

    const data: any[] = response.data.timetable || [];
    const prefixCounter: Record<string, number> = {};
    let targetMatchedRawCount = 0;
    let filteredCount = 0;

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
    const targetXkkhPrefix = buildXkkhPrefix(responseTerm.year, responseTerm.academicSemester);
    console.log('[CalendarView] Target xkkh Prefix:', targetXkkhPrefix);

    data.forEach((session: any) => {
      const xkkh = (session.xkkh || '').trim();
      const prefix = extractXkkhPrefix(xkkh);
      prefixCounter[prefix] = (prefixCounter[prefix] || 0) + 1;
      if (xkkh.startsWith(targetXkkhPrefix)) {
        targetMatchedRawCount++;
      }

      // Skip graduate courses — same as Flutter: sfyjskc !== "1"
      if (session.sfyjskc === '1') return;
      
      // CRITICAL BUG FIX: The ZJU API frequently ignores xnm/xqm and returns courses 
      // from multiple past & future semesters. We MUST strictly match the target xkkh prefix.
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
      if (periodIdx >= 0 && periodIdx < 13) {
        filteredCount++;
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

    const top = Object.entries(prefixCounter)
      .sort((a, b) => b[1] - a[1])
      .slice(0, 10)
      .map(([prefix, count]) => ({ prefix, count }));
    timetableDebug.value = {
      requestYear: zjuYearStr,
      requestSem: zjuSemStr,
      responseYear: responseTerm.year,
      responseSem: responseTerm.academicSemester,
      responseXqm: response.data.xqm || "",
      metaSource: response._meta?.source || "unknown",
      metaTime: response._meta?.timestamp
        ? new Date(response._meta.timestamp * 1000).toLocaleString('zh-CN', { hour12: false })
        : "N/A",
      targetPrefix: targetXkkhPrefix,
      rawCount: data.length,
      targetMatchedRawCount,
      filteredCount,
      prefixTop: top,
    };

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

function extractXkkhPrefix(xkkh: string) {
  if (!xkkh.startsWith('(')) return "UNKNOWN";
  const end = xkkh.indexOf(')');
  if (end <= 0) return "UNKNOWN";
  return xkkh.slice(0, end + 1);
}

// --- iCal Export ---
function formatDateForICS(date: Date): string {
  const pad = (n: number) => n.toString().padStart(2, '0');
  return `${date.getUTCFullYear()}${pad(date.getUTCMonth()+1)}${pad(date.getUTCDate())}T${pad(date.getUTCHours())}${pad(date.getUTCMinutes())}00Z`;
}

async function exportToICS() {
  if (allCourses.value.length === 0) {
    alert("当前没有可导出的课程数据！");
    return;
  }
  
  try {
    const filePath = await save({
      filters: [{ name: 'iCalendar', extensions: ['ics'] }],
      defaultPath: `Celechron_${semesterTabs.value[activeSemIdx.value]?.label || '课表'}.ics`,
    });
    
    if (!filePath) return; // User canceled dialog

    let icsContent = 
`BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Celechron//Tauri App//ZH
CALSCALE:GREGORIAN
METHOD:PUBLISH
X-WR-CALNAME:ZJU 课程表
X-WR-TIMEZONE:Asia/Shanghai
`;

    // Baseline is week 1 Monday
    const baseMonday = new Date(startDateMs.value);
    
    // We already parsed periods mapping
    const periodTimes = periods.map(p => {
      const parts = p.time.split('-');
      return { start: parts[0], end: parts[1] };
    });

    const nowStr = formatDateForICS(new Date());

    allCourses.value.forEach(course => {
      course.activeWeeks.forEach(week => {
        // Calculate date of this specific class:
        // Base Monday + (week - 1) * 7 days + dayIdx
        const classDate = new Date(baseMonday.getTime() + (week - 1) * 7 * 24 * 60 * 60 * 1000 + course.dayIdx * 24 * 60 * 60 * 1000);
        
        const pt = periodTimes[course.periodIdx];
        const ptEnd = periodTimes[course.periodIdx + course.span - 1];
        if (!pt || !ptEnd) return; // Safety check

        // Create Start Date
        const startDate = new Date(classDate);
        startDate.setHours(parseInt(pt.start.split(':')[0]), parseInt(pt.start.split(':')[1]), 0);
        
        // Create End Date
        const endDate = new Date(classDate);
        endDate.setHours(parseInt(ptEnd.end.split(':')[0]), parseInt(ptEnd.end.split(':')[1]), 0);

        icsContent += `BEGIN:VEVENT
UID:${crypto.randomUUID()}
DTSTAMP:${nowStr}
DTSTART:${formatDateForICS(startDate)}
DTEND:${formatDateForICS(endDate)}
SUMMARY:${course.name}
LOCATION:${course.location || '未知地点'}
DESCRIPTION:${course.teacher ? `教师: ${course.teacher}` : ''}
END:VEVENT
`;
      });
    });

    icsContent += "END:VCALENDAR";

    await writeTextFile(filePath, icsContent);
    alert("成功导出为 ics 文件！您可以将其导入至系统日历。");
  } catch (e: any) {
    console.error(e);
    alert(`导出失败: ${e.message || e}`);
  }
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
              ({{ periods[selectedCourse.periodIdx]?.time }}-{{ periods[selectedCourse.periodIdx + selectedCourse.span - 1]?.time }})
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
  color: #e2e8f0;
}
.month-label {
  font-size: 1rem;
  color: #38bdf8;
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
  color: #94a3b8;
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
  color: #f8fafc;
}
.semester-tab.active {
  background: #38bdf8;
  color: white;
  box-shadow: 0 4px 12px rgba(56, 189, 248, 0.3);
}

:global(.light-theme) .semester-tabs {
  background: rgba(0, 0, 0, 0.03);
  border-color: rgba(0, 0, 0, 0.06);
}
:global(.light-theme) .semester-tab {
  color: #64748b;
}
:global(.light-theme) .semester-tab:hover {
  background: rgba(0, 0, 0, 0.06);
}
:global(.light-theme) .semester-tab.active {
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
  color: #94a3b8;
}
.badge-value {
  font-size: 1.1rem;
  font-weight: 700;
  color: #38bdf8;
}
.pill-btn:hover { background: rgba(255,255,255,.1); color: #fff; }
.pill-label {
  padding: 0 12px;
  font-size: 0.85rem;
  font-weight: 600;
  color: #e2e8f0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  cursor: pointer;
  transition: opacity 0.2s;
}
.pill-label:hover {
  opacity: 0.8;
}

:global(.light-theme) .week-selector-pill {
  background: rgba(0,0,0,0.05);
  border-color: rgba(0,0,0,0.1);
  box-shadow: inset 0 2px 4px rgba(0,0,0,0.02);
}
:global(.light-theme) .pill-btn {
  color: #64748b;
}
:global(.light-theme) .pill-btn:hover {
  background: rgba(0,0,0,0.08);
  color: #0f172a;
}
:global(.light-theme) .pill-label {
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
  color: white;
  font-size: 1.2rem;
  cursor: pointer;
  transition: all 0.2s;
}
.week-btn:hover {
  background: rgba(255, 255, 255, 0.1);
  transform: scale(1.05);
}
:global(.light-theme) .week-btn {
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
:global(.light-theme) .day-item {
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
:global(.light-theme) .calendar-view {
  color: #1e293b;
}
:global(.light-theme) .cal-header h1,
:global(.light-theme) .week-label,
:global(.light-theme) .month-label {
  color: #1e293b;
}
:global(.light-theme) .week-btn {
  background: rgba(0,0,0,0.06);
  color: #334155;
  border-color: rgba(0,0,0,0.08);
}
:global(.light-theme) .week-btn:hover {
  background: rgba(0,0,0,0.10);
}
:global(.light-theme) .schedule-grid {
  border-color: rgba(0,0,0,0.06);
}
:global(.light-theme) .grid-day-header {
  color: #334155;
}
:global(.light-theme) .grid-day-header .day-num {
  color: #1e293b;
}
:global(.light-theme) .grid-cell {
  border-color: rgba(0,0,0,0.04);
}
:global(.light-theme) .grid-period-label .period-num {
  color: #334155;
}
:global(.light-theme) .grid-period-label .period-time {
  color: #64748b;
}
:global(.light-theme) .course-block {
  box-shadow: 0 2px 8px rgba(0,0,0,0.08);
}
:global(.light-theme) .course-block .course-name {
  color: #fff;  /* keep white on colored accent bg */
}
:global(.light-theme) .course-block .course-loc {
  color: rgba(255,255,255,0.85);
}
:global(.light-theme) .today-summary h3 {
  color: #1e293b;
}
:global(.light-theme) .today-item .today-name {
  color: #1e293b;
}
:global(.light-theme) .today-item .today-time,
:global(.light-theme) .today-item .today-loc {
  color: #64748b;
}

/* Glass panel in light mode */
:global(.light-theme) .glass-panel {
  background: rgba(255, 255, 255, 0.8);
  border-color: rgba(0, 0, 0, 0.08);
  box-shadow: 0 8px 32px rgba(0,0,0,0.08);
}
:global(.light-theme) .modal-overlay {
  background: rgba(0,0,0,0.25);
}
:global(.light-theme) .modal-content h3 {
  color: #1e293b;
}
:global(.light-theme) .modal-content p {
  color: #64748b !important;
}

/* Detail rows light mode */
:global(.light-theme) .detail-row {
  background: rgba(0,0,0,0.03);
  border-color: rgba(0,0,0,0.05);
}
:global(.light-theme) .detail-label {
  color: #64748b;
}
:global(.light-theme) .detail-value {
  color: #1e293b;
}

/* Month grid light mode */
:global(.light-theme) .month-cell {
  background: rgba(255,255,255,0.6);
  border-color: rgba(0,0,0,0.06);
}
:global(.light-theme) .month-cell:not(.empty):hover {
  background: rgba(0,0,0,0.04);
}
:global(.light-theme) .month-day-num {
  color: #1e293b;
}
:global(.light-theme) .month-day-header {
  color: #64748b;
}
:global(.light-theme) .month-cell.selected {
  background: #0284c7;
  border-color: #0284c7;
}
:global(.light-theme) .month-cell.selected .month-day-num {
  color: #fff;
}
:global(.light-theme) .month-cell.in-week {
  background: rgba(2, 132, 199, 0.08);
  border-color: rgba(2, 132, 199, 0.15);
}
:global(.light-theme) .month-cell.is-today .month-day-num {
  background: #0284c7;
  color: #fff;
}

/* Selected day panel light mode */
:global(.light-theme) .panel-date {
  color: #1e293b;
}
:global(.light-theme) .empty-state {
  color: #64748b;
}
:global(.light-theme) .day-section h4 {
  color: #334155;
}
:global(.light-theme) .day-item {
  background: rgba(0,0,0,0.03);
}
:global(.light-theme) .item-title {
  color: #1e293b;
}
:global(.light-theme) .item-desc {
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
  color: #e2e8f0;
}
:global(.light-theme) .hide-course-settings span {
  color: #334155;
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
:global(.light-theme) .toggle-switch {
  background: rgba(0,0,0,0.15);
}

.action-icon-btn {
  color: #1e293b;
}
:global(.light-theme) .item-desc {
  color: #64748b;
}

.action-icon-btn {
  background: transparent;
  color: #64748b;
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

:global(.light-theme) .action-icon-btn {
  color: #64748b;
}
:global(.light-theme) .action-icon-btn:hover {
  background: rgba(0,0,0,0.05);
}
:global(.light-theme) .action-icon-btn.primary-icon-btn {
  background: #0ea5e9;
  color: white;
}
:global(.light-theme) .action-icon-btn.primary-icon-btn:hover {
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
