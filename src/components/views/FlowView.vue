<script setup lang="ts">
import { ref, onMounted } from "vue";
import { LiquidGlass } from '@wxperia/liquid-glass-vue';
import { fetchTimetable, fetchTodos } from "../../services/api";
import { usePreferences } from "../../composables/usePreferences";
import { resolveCurrentTimetableTerm } from "../../utils/semester";
import { buildCourseOccurrences, startOfLocalDay } from "../../utils/timetable";

interface FlowItem {
  id: string;
  type: 'course' | 'task';
  title: string;
  subtitle: string;
  timeLabel: string;
  timeMs: number;
  color: string;
}

const isLoading = ref(true);
const items = ref<FlowItem[]>([]);
const isOffline = ref(false);
const DEFAULT_FLOW_COLORS = ["#06b6d4", "#8b5cf6", "#f97316", "#22c55e", "#ec4899", "#eab308"];
const FLOW_COLOR_TOKENS = ["--accent-blue", "--accent-purple", "--accent-amber", "--accent-green", "--accent-pink", "--accent-yellow"];
const colors = ref<string[]>([...DEFAULT_FLOW_COLORS]);
const { manualSemesterAnchors, timeConfigMode } = usePreferences();

function readCssVar(name: string, fallback: string) {
  if (typeof window === "undefined") {
    return fallback;
  }
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

function buildFlowColors() {
  return DEFAULT_FLOW_COLORS.map((fallback, index) => readCssVar(FLOW_COLOR_TOKENS[index], fallback));
}

function buildDateLabel(date: Date): string {
  const today = startOfLocalDay(new Date());
  const current = startOfLocalDay(date);
  const diffDays = Math.round((current.getTime() - today.getTime()) / (24 * 60 * 60 * 1000));
  if (diffDays === 0) {
    return '今天';
  }
  if (diffDays === 1) {
    return '明天';
  }
  return `${date.getMonth() + 1}月${date.getDate()}日`;
}

async function loadFlow() {
  isLoading.value = true;
  isOffline.value = false;

  try {
    const newItems: FlowItem[] = [];
    const today = startOfLocalDay(new Date());
    const rangeEnd = new Date(today.getTime() + 7 * 24 * 60 * 60 * 1000);
    const terms = [resolveCurrentTimetableTerm(today), resolveCurrentTimetableTerm(rangeEnd)]
      .filter((term, index, source) => source.findIndex((item) => item.name === term.name) === index);

    const [todoEnv, ...timetableEnvs] = await Promise.all([
      fetchTodos(),
      ...terms.map((term) => fetchTimetable({ year: term.year, semester: term.timetableSemester })),
    ]);

    if (todoEnv._meta?.source === 'cache') {
      isOffline.value = true;
    }

    for (const todo of todoEnv.data.todo_list || []) {
      const dueAt = new Date(todo.end_time).getTime();
      if (dueAt > today.getTime() && dueAt < rangeEnd.getTime()) {
        newItems.push({
          id: `task-${todo.id}`,
          type: 'task',
          title: todo.title,
          subtitle: todo.course_name || '学在浙大',
          timeLabel: new Date(todo.end_time).toLocaleString('zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' }),
          timeMs: dueAt,
          color: readCssVar('--accent-red', '#ef4444'),
        });
      }
    }

    const courseColors = new Map<string, string>();
    let colorIdx = 0;

    for (const timetableEnv of timetableEnvs) {
      if (timetableEnv._meta?.source === 'cache') {
        isOffline.value = true;
      }

      const occurrences = buildCourseOccurrences(timetableEnv.data, {
        manualAnchors: manualSemesterAnchors.value,
        timeConfigMode: timeConfigMode.value,
      });

      for (const occurrence of occurrences) {
        const startAt = occurrence.startDateTime;
        if (!startAt) {
          continue;
        }
        const startMs = startAt.getTime();
        if (startMs < today.getTime() || startMs >= rangeEnd.getTime()) {
          continue;
        }

        const colorKey = occurrence.session.xkkh || occurrence.session.courseName;
        if (!courseColors.has(colorKey)) {
          courseColors.set(colorKey, colors.value[colorIdx % colors.value.length]);
          colorIdx += 1;
        }

        newItems.push({
          id: `course-${occurrence.id}`,
          type: 'course',
          title: occurrence.session.courseName,
          subtitle: occurrence.session.location || '无地点',
          timeLabel: `${buildDateLabel(startAt)} ${occurrence.startSlot?.start || startAt.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })}`,
          timeMs: startMs,
          color: courseColors.get(colorKey)!,
        });
      }
    }

    newItems.sort((left, right) => left.timeMs - right.timeMs || left.title.localeCompare(right.title));
    items.value = newItems;
  } catch (error) {
    console.error(error);
  } finally {
    isLoading.value = false;
  }
}

onMounted(() => {
  colors.value = buildFlowColors();
  loadFlow();
});
</script>

<template>
  <div class="flow-view">
    <header class="section-header">
      <h2>接下来 <span class="section-subtitle">(Flow)</span></h2>
      <div v-if="isOffline" class="offline-badge">离线模式</div>
    </header>

    <div v-if="isLoading" class="loading-state">
      <div class="loader"></div>
    </div>

    <div v-else-if="items.length === 0" class="empty-state">
      <span class="empty-icon">🌴</span>
      <h3>近期暂无安排</h3>
      <p>享受你的空闲时间吧！</p>
    </div>

    <div v-else class="timeline">
      <div v-for="item in items" :key="item.id" class="timeline-item">
        <div class="time-block">
          <span class="time-dot" :style="{ background: item.color }"></span>
          <span class="time-text">{{ item.timeLabel }}</span>
        </div>
        
        <LiquidGlass
          :displacement-scale="32"
          :blur-amount="0.1"
          :saturation="120"
          :aberration-intensity="1"
          :elasticity="0.25"
          :corner-radius="16"
          class="card-wrapper"
        >
          <div class="item-card" :style="{ '--accent': item.color }">
            <span class="badge" :class="item.type">{{ item.type === 'task' ? 'DDL' : '课程' }}</span>
            <div class="content">
              <h4>{{ item.title }}</h4>
              <p>{{ item.subtitle }}</p>
            </div>
          </div>
        </LiquidGlass>
      </div>
    </div>
  </div>
</template>

<style scoped>
.flow-view {
  --flow-title: var(--text-main);
  --flow-subtitle: var(--text-muted);
  --flow-offline-bg: color-mix(in srgb, var(--accent-amber) 18%, transparent);
  --flow-offline-text: var(--accent-amber);
  --flow-offline-border: color-mix(in srgb, var(--accent-amber) 35%, transparent);
  --flow-timeline-line: var(--panel-border);
  --flow-time-text: var(--text-muted);
  --flow-item-bg: color-mix(in srgb, var(--accent) 5%, transparent);
  --flow-heading: var(--text-main);
  --flow-muted: var(--text-muted);
  --flow-state-text: var(--text-muted);
  --flow-loader-border: var(--card-border);
  --flow-loader-top: var(--accent-blue);

  padding: 2rem 2.5rem 6rem;
  max-width: 800px;
  margin: 0 auto;
}

.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 2rem;
}
.section-header h2 {
  font-size: 2rem;
  margin: 0;
  color: var(--flow-title);
}

.section-subtitle {
  color: var(--flow-subtitle);
  font-weight: 400;
  font-size: 1.2rem;
}

.offline-badge {
  background: var(--flow-offline-bg);
  color: var(--flow-offline-text);
  padding: 4px 12px;
  border-radius: 20px;
  font-size: 0.85rem;
  border: 1px solid var(--flow-offline-border);
}

.timeline {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
  position: relative;
}
.timeline::before {
  content: '';
  position: absolute;
  left: 6px;
  top: 10px;
  bottom: 0;
  width: 2px;
  background: linear-gradient(to bottom, var(--flow-timeline-line), transparent);
}

.timeline-item {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-left: 24px;
  position: relative;
  animation: fade-in 0.4s ease-out forwards;
}

.time-block {
  display: flex;
  align-items: center;
  gap: 10px;
  position: relative;
  left: -24px;
}
.time-dot {
  width: 14px;
  height: 14px;
  border-radius: 50%;
  box-shadow: 0 0 10px currentColor;
  border: 2px solid var(--bg-main);
  z-index: 2;
}
.time-text {
  font-size: 0.9rem;
  font-weight: 600;
  color: var(--flow-time-text);
}

.card-wrapper {
  width: 100%;
}
.item-card {
  padding: 1.2rem;
  display: flex;
  align-items: flex-start;
  gap: 15px;
  background: var(--flow-item-bg);
  border-left: 3px solid var(--accent);
}
.badge {
  padding: 4px 8px;
  border-radius: 8px;
  font-size: 0.75rem;
  font-weight: 700;
  flex-shrink: 0;
}
.badge.course { background: rgba(56, 189, 248, 0.15); color: #38bdf8; }
.badge.task { background: rgba(239, 68, 68, 0.15); color: #ef4444; }

.content h4 {
  margin: 0 0 4px;
  font-size: 1.1rem;
  color: var(--flow-heading);
}
.content p {
  margin: 0;
  color: var(--flow-muted);
  font-size: 0.85rem;
}

.loading-state, .empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 400px;
  color: var(--flow-state-text);
  text-align: center;
}

.empty-icon {
  font-size: 3rem;
  margin-bottom: 1rem;
}

.loader {
  width: 30px;
  height: 30px;
  border: 3px solid var(--flow-loader-border);
  border-radius: 50%;
  border-top-color: var(--flow-loader-top);
  animation: spin 1s linear infinite;
}

@keyframes spin { to { transform: rotate(360deg); } }
@keyframes fade-in {
  from { opacity: 0; transform: translateX(-10px); }
  to { opacity: 1; transform: translateX(0); }
}

:global(html[data-theme='light']) .badge.course {
  background: rgba(2, 132, 199, 0.1);
  color: #0284c7;
}
:global(html[data-theme='light']) .badge.task {
  background: rgba(220, 38, 38, 0.1);
  color: #dc2626;
}
:global(html[data-theme='light']) .item-card {
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.05);
}

</style>
