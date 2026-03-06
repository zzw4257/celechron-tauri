<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import SectionCard from '../ui/SectionCard.vue';
import StatusBanner from '../ui/StatusBanner.vue';
import { fetchTimetable, fetchTodos } from '../../services/api';
import { usePreferences } from '../../composables/usePreferences';
import { resolveCurrentTimetableTerm } from '../../utils/semester';
import { buildCourseOccurrences, startOfLocalDay } from '../../utils/timetable';

interface FlowItem {
  id: string;
  type: 'course' | 'task';
  title: string;
  subtitle: string;
  timeLabel: string;
  timeMs: number;
  tone: string;
}

const isLoading = ref(true);
const items = ref<FlowItem[]>([]);
const isOffline = ref(false);
const errorMsg = ref('');

const FLOW_TONES = [
  'var(--accent-text)',
  'var(--warning-text)',
  'var(--success-text)',
  'var(--danger-text)',
];

const { accountScope, manualSemesterAnchors, timeConfigMode } = usePreferences();

const groupedItems = computed(() => {
  const groups = new Map<string, FlowItem[]>();
  for (const item of items.value) {
    const key = item.timeLabel.split(' ')[0] || item.timeLabel;
    const bucket = groups.get(key) || [];
    bucket.push(item);
    groups.set(key, bucket);
  }
  return [...groups.entries()].map(([label, entries]) => ({ label, entries }));
});

function buildDateLabel(date: Date): string {
  const today = startOfLocalDay(new Date());
  const current = startOfLocalDay(date);
  const diffDays = Math.round((current.getTime() - today.getTime()) / 86400000);
  if (diffDays === 0) return '今天';
  if (diffDays === 1) return '明天';
  return `${date.getMonth() + 1}月${date.getDate()}日`;
}

function normalizeTodoTime(value?: string) {
  if (!value) return null;
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
}

async function loadFlow() {
  isLoading.value = true;
  isOffline.value = false;
  errorMsg.value = '';

  try {
    const nextItems: FlowItem[] = [];
    const today = startOfLocalDay(new Date());
    const rangeEnd = new Date(today.getTime() + 7 * 24 * 60 * 60 * 1000);
    const terms = [resolveCurrentTimetableTerm(today), resolveCurrentTimetableTerm(rangeEnd)].filter(
      (term, index, source) => source.findIndex((candidate) => candidate.name === term.name) === index,
    );

    const [todoEnv, ...timetableEnvs] = await Promise.all([
      fetchTodos(),
      ...terms.map((term) => fetchTimetable({ year: term.year, semester: term.timetableSemester })),
    ]);

    isOffline.value = [todoEnv, ...timetableEnvs].some((env) => env._meta?.source === 'cache');

    for (const todo of todoEnv.data.todo_list) {
      const dueAt = normalizeTodoTime(todo.endTime || todo.end_time);
      if (!dueAt) continue;
      const dueMs = dueAt.getTime();
      if (dueMs < today.getTime() || dueMs >= rangeEnd.getTime()) continue;
      nextItems.push({
        id: `task-${todo.id}`,
        type: 'task',
        title: todo.title || '未命名任务',
        subtitle: todo.courseName || todo.course_name || '学在浙大',
        timeLabel: `${buildDateLabel(dueAt)} ${dueAt.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })}`,
        timeMs: dueMs,
        tone: 'var(--danger-text)',
      });
    }

    const courseTones = new Map<string, string>();
    let toneIndex = 0;

    for (const timetableEnv of timetableEnvs) {
      const occurrences = buildCourseOccurrences(timetableEnv.data, {
        manualAnchors: manualSemesterAnchors.value,
        timeConfigMode: timeConfigMode.value,
      });

      for (const occurrence of occurrences) {
        const startAt = occurrence.startDateTime;
        if (!startAt) continue;
        const startMs = startAt.getTime();
        if (startMs < today.getTime() || startMs >= rangeEnd.getTime()) continue;

        const colorKey = occurrence.session.xkkh || occurrence.session.courseName;
        if (!courseTones.has(colorKey)) {
          courseTones.set(colorKey, FLOW_TONES[toneIndex % FLOW_TONES.length]);
          toneIndex += 1;
        }

        nextItems.push({
          id: `course-${occurrence.id}`,
          type: 'course',
          title: occurrence.session.courseName,
          subtitle: occurrence.session.location || '地点待定',
          timeLabel: `${buildDateLabel(startAt)} ${occurrence.startSlot?.start || startAt.toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })}`,
          timeMs: startMs,
          tone: courseTones.get(colorKey) || 'var(--accent-text)',
        });
      }
    }

    nextItems.sort((left, right) => left.timeMs - right.timeMs || left.title.localeCompare(right.title));
    items.value = nextItems;
  } catch (error: any) {
    errorMsg.value = error?.message || String(error);
  } finally {
    isLoading.value = false;
  }
}

onMounted(loadFlow);
watch(accountScope, loadFlow);
</script>

<template>
  <div class="page-shell flow-view">
    <header class="page-header">
      <div>
        <h1>接下来</h1>
        <p class="page-subtitle">统一消费 7 天内的任务和课表事件。</p>
      </div>
      <span class="badge" :class="isOffline ? 'warning' : 'accent'">{{ isOffline ? '缓存模式' : '实时数据' }}</span>
    </header>

    <StatusBanner v-if="errorMsg" tone="danger" title="拉取失败">
      {{ errorMsg }}
    </StatusBanner>

    <SectionCard v-if="isLoading" title="加载中" subtitle="正在汇总未来 7 天安排。">
      <div class="state-card">请稍候，正在整理课表与截止项。</div>
    </SectionCard>

    <SectionCard v-else-if="items.length === 0" title="近期为空" subtitle="未来 7 天暂时没有课程或截止任务。">
      <div class="state-card">当前时段没有需要提醒的安排。</div>
    </SectionCard>

    <div v-else class="flow-groups">
      <SectionCard v-for="group in groupedItems" :key="group.label" :title="group.label" dense>
        <div class="flow-list">
          <article v-for="item in group.entries" :key="item.id" class="flow-item">
            <div class="flow-item__marker" :style="{ background: item.tone }"></div>
            <div class="flow-item__main">
              <div class="flow-item__head">
                <strong>{{ item.title }}</strong>
                <span class="badge" :class="item.type === 'task' ? 'danger' : 'accent'">
                  {{ item.type === 'task' ? '任务' : '课程' }}
                </span>
              </div>
              <p>{{ item.subtitle }}</p>
            </div>
            <time class="flow-item__time">{{ item.timeLabel }}</time>
          </article>
        </div>
      </SectionCard>
    </div>
  </div>
</template>

<style scoped>
.flow-view {
  gap: 1rem;
}

.flow-groups,
.flow-list {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.flow-item {
  display: grid;
  grid-template-columns: 0.5rem minmax(0, 1fr) auto;
  gap: 0.85rem;
  align-items: center;
  padding: 0.9rem 0.4rem;
  border-bottom: 1px solid var(--border-subtle);
}

.flow-item:last-child {
  border-bottom: none;
  padding-bottom: 0;
}

.flow-item__marker {
  width: 0.5rem;
  height: 100%;
  min-height: 3.4rem;
  border-radius: var(--radius-pill);
}

.flow-item__main {
  min-width: 0;
}

.flow-item__head {
  display: flex;
  align-items: center;
  gap: 0.55rem;
  justify-content: space-between;
}

.flow-item__head strong {
  color: var(--text-primary);
}

.flow-item__main p,
.flow-item__time {
  margin: 0.2rem 0 0;
  color: var(--text-secondary);
}

.flow-item__time {
  white-space: nowrap;
  text-align: right;
}

@media (max-width: 720px) {
  .flow-item {
    grid-template-columns: 0.45rem minmax(0, 1fr);
  }

  .flow-item__time {
    grid-column: 2;
    text-align: left;
  }
}
</style>
