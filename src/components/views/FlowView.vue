<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import ActionPill from '../ui/ActionPill.vue';
import InlineStat from '../ui/InlineStat.vue';
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

const FLOW_TONES = ['#0f7aa9', '#1a8b4f', '#a85516', '#be123c', '#2563eb', '#0f766e'];

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

const summary = computed(() => ({
  total: items.value.length,
  courses: items.value.filter((item) => item.type === 'course').length,
  tasks: items.value.filter((item) => item.type === 'task').length,
}));

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

function openCalendar() {
  window.dispatchEvent(new CustomEvent('celechron:navigate', { detail: { tab: 'calendar' } }));
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
        tone: '#be123c',
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
          tone: courseTones.get(colorKey) || FLOW_TONES[0],
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
        <p class="page-subtitle">未来 7 天用同一条时间线汇总课程与截止任务，保留悬浮感而不过度装饰。</p>
      </div>
      <div class="flow-header-actions">
        <ActionPill tone="accent" @click="openCalendar">打开课表</ActionPill>
        <span class="badge" :class="isOffline ? 'warning' : 'accent'">{{ isOffline ? '缓存模式' : '实时数据' }}</span>
      </div>
    </header>

    <StatusBanner v-if="errorMsg" tone="danger" title="拉取失败">
      {{ errorMsg }}
    </StatusBanner>

    <SectionCard v-if="isLoading" title="加载中" subtitle="正在汇总未来 7 天安排。">
      <div class="state-card">请稍候，正在整理课表与截止项。</div>
    </SectionCard>

    <template v-else>
      <div class="flow-stats">
        <InlineStat label="总安排" :value="String(summary.total)" emphasis />
        <InlineStat label="课程" :value="String(summary.courses)" />
        <InlineStat label="任务" :value="String(summary.tasks)" />
      </div>

      <SectionCard v-if="items.length === 0" title="近期为空" subtitle="未来 7 天暂时没有课程或截止任务。">
        <div class="state-card">当前时段没有需要提醒的安排。</div>
      </SectionCard>

      <div v-else class="flow-groups">
        <SectionCard v-for="group in groupedItems" :key="group.label" :title="group.label" dense>
          <div class="flow-list">
            <article
              v-for="item in group.entries"
              :key="item.id"
              class="flow-card"
              :style="{ '--flow-tone': item.tone }"
            >
              <div class="flow-card__head">
                <span class="badge" :class="item.type === 'task' ? 'danger' : 'accent'">
                  {{ item.type === 'task' ? '任务' : '课程' }}
                </span>
                <time>{{ item.timeLabel }}</time>
              </div>
              <strong>{{ item.title }}</strong>
              <p>{{ item.subtitle }}</p>
            </article>
          </div>
        </SectionCard>
      </div>
    </template>
  </div>
</template>

<style scoped>
.flow-view,
.flow-groups,
.flow-list {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.flow-header-actions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.7rem;
}

.flow-stats {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 0.75rem;
}

.flow-list {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.flow-card {
  position: relative;
  overflow: hidden;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: linear-gradient(160deg, color-mix(in srgb, var(--flow-tone) 12%, var(--surface-1)) 0%, var(--surface-2) 100%);
  padding: 1rem;
  min-height: 8.2rem;
  box-shadow: var(--shadow-soft);
}

.flow-card::before {
  content: '';
  position: absolute;
  inset: auto -2rem -2.5rem auto;
  width: 8rem;
  height: 8rem;
  border-radius: 50%;
  background: color-mix(in srgb, var(--flow-tone) 20%, transparent);
  filter: blur(10px);
  pointer-events: none;
}

.flow-card__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  margin-bottom: 0.75rem;
}

.flow-card__head time,
.flow-card p {
  color: var(--text-secondary);
}

.flow-card strong {
  display: block;
  color: var(--text-primary);
  font-size: 1.05rem;
  margin-bottom: 0.3rem;
}

.flow-card p {
  margin: 0;
}

@media (max-width: 900px) {
  .flow-stats,
  .flow-list {
    grid-template-columns: 1fr;
  }
}
</style>
