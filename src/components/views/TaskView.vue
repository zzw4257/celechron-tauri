<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import PomodoroWidget from '../PomodoroWidget.vue';
import ActionPill from '../ui/ActionPill.vue';
import InlineStat from '../ui/InlineStat.vue';
import SectionCard from '../ui/SectionCard.vue';
import StatusBanner from '../ui/StatusBanner.vue';
import { fetchTodos } from '../../services/api';
import type { TodoItem } from '../../types/api';
import { usePreferences } from '../../composables/usePreferences';

interface TaskBucketItem {
  id: string;
  title: string;
  courseName: string;
  deadlineLabel: string;
  remainLabel: string;
  endMs: number;
  linkUrl?: string | null;
}

const isLoading = ref(true);
const isOffline = ref(false);
const errorMsg = ref('');
const copiedId = ref('');
const offlineTime = ref('');
const buckets = ref<Record<'overdue' | 'today' | 'week' | 'later', TaskBucketItem[]>>({
  overdue: [],
  today: [],
  week: [],
  later: [],
});

const { accountScope } = usePreferences();

const summary = computed(() => ({
  total: Object.values(buckets.value).reduce((acc, items) => acc + items.length, 0),
  today: buckets.value.today.length,
  week: buckets.value.week.length,
  overdue: buckets.value.overdue.length,
}));

function normalizeTodoTime(todo: TodoItem) {
  const raw = todo.endTime || todo.end_time;
  if (!raw) return null;
  const parsed = new Date(raw);
  return Number.isNaN(parsed.getTime()) ? null : parsed;
}

function formatRemain(ms: number) {
  const diffDays = ms / 86400000;
  if (diffDays < 0) return `已逾期 ${Math.abs(Math.ceil(diffDays))} 天`;
  if (diffDays <= 1) return '24 小时内截止';
  if (diffDays <= 7) return `${Math.ceil(diffDays)} 天内截止`;
  return `${Math.ceil(diffDays)} 天后`;
}

async function copyLink(item: TaskBucketItem) {
  if (!item.linkUrl) return;
  await navigator.clipboard.writeText(item.linkUrl);
  copiedId.value = item.id;
  window.setTimeout(() => {
    if (copiedId.value === item.id) copiedId.value = '';
  }, 1600);
}

async function loadTasks() {
  isLoading.value = true;
  errorMsg.value = '';
  copiedId.value = '';

  try {
    const env = await fetchTodos();
    isOffline.value = env._meta?.source === 'cache';
    offlineTime.value = env._meta?.timestamp
      ? new Date(env._meta.timestamp * 1000).toLocaleString('zh-CN', { hour12: false })
      : '';

    const nextBuckets: Record<'overdue' | 'today' | 'week' | 'later', TaskBucketItem[]> = {
      overdue: [],
      today: [],
      week: [],
      later: [],
    };

    const now = Date.now();
    for (const todo of env.data.todo_list) {
      const deadline = normalizeTodoTime(todo);
      if (!deadline) continue;
      const diffMs = deadline.getTime() - now;
      const item: TaskBucketItem = {
        id: todo.id,
        title: todo.title || '未命名任务',
        courseName: todo.courseName || todo.course_name || '学在浙大',
        deadlineLabel: deadline.toLocaleString('zh-CN', {
          month: '2-digit',
          day: '2-digit',
          hour: '2-digit',
          minute: '2-digit',
          hour12: false,
        }),
        remainLabel: formatRemain(diffMs),
        endMs: deadline.getTime(),
        linkUrl: todo.linkUrl || null,
      };

      if (diffMs < 0) nextBuckets.overdue.push(item);
      else if (diffMs <= 86400000) nextBuckets.today.push(item);
      else if (diffMs <= 7 * 86400000) nextBuckets.week.push(item);
      else nextBuckets.later.push(item);
    }

    for (const list of Object.values(nextBuckets)) {
      list.sort((left, right) => left.endMs - right.endMs || left.title.localeCompare(right.title));
    }

    buckets.value = nextBuckets;
  } catch (error: any) {
    errorMsg.value = error?.message || String(error);
  } finally {
    isLoading.value = false;
  }
}

onMounted(loadTasks);
watch(accountScope, loadTasks);
</script>

<template>
  <div class="page-shell task-view">
    <header class="page-header">
      <div>
        <h1>任务</h1>
        <p class="page-subtitle">后端已标准化截止时间与真实链接，只在可访问时才显示复制入口。</p>
      </div>
      <span class="badge" :class="isOffline ? 'warning' : 'accent'">{{ isOffline ? '缓存模式' : '实时同步' }}</span>
    </header>

    <div class="task-grid">
      <SectionCard title="节奏区" subtitle="番茄钟保留，但压缩到次级位置。" dense>
        <PomodoroWidget />
      </SectionCard>

      <SectionCard title="任务摘要" subtitle="首屏只保留真正影响节奏的信息。" dense>
        <div class="task-stats">
          <InlineStat label="总任务" :value="String(summary.total)" emphasis />
          <InlineStat label="24h 内" :value="String(summary.today)" />
          <InlineStat label="7 天内" :value="String(summary.week)" />
          <InlineStat label="已逾期" :value="String(summary.overdue)" />
        </div>
      </SectionCard>
    </div>

    <StatusBanner v-if="errorMsg" tone="danger" title="同步失败">
      {{ errorMsg }}
    </StatusBanner>
    <StatusBanner v-else-if="isOffline && offlineTime" tone="warning" title="缓存回退">
      当前展示的是本地缓存，更新时间 {{ offlineTime }}。
    </StatusBanner>
    <StatusBanner v-else-if="copiedId" tone="success" title="复制成功">
      已复制任务链接，可以直接粘贴给同学或在浏览器打开。
    </StatusBanner>

    <SectionCard v-if="isLoading" title="加载中" subtitle="正在拉取待办和截止时间。">
      <div class="state-card">请稍候，正在整理任务优先级。</div>
    </SectionCard>

    <SectionCard
      v-else-if="summary.total === 0"
      title="暂无待办"
      subtitle="当前没有需要你跟进的任务。"
    >
      <div class="state-card">最近没有截止项，适合回头处理资料或复盘成绩。</div>
    </SectionCard>

    <div v-else class="task-sections">
      <SectionCard v-if="buckets.overdue.length" title="已逾期" subtitle="优先回收。" dense>
        <div class="task-list">
          <article v-for="item in buckets.overdue" :key="item.id" class="task-card overdue">
            <div class="task-card__main">
              <div class="task-card__head">
                <strong>{{ item.title }}</strong>
                <span class="badge danger">{{ item.remainLabel }}</span>
              </div>
              <p>{{ item.courseName }}</p>
            </div>
            <div class="task-card__meta">
              <time>{{ item.deadlineLabel }}</time>
              <ActionPill v-if="item.linkUrl" tone="danger" @click="copyLink(item)">复制链接</ActionPill>
            </div>
          </article>
        </div>
      </SectionCard>

      <SectionCard v-if="buckets.today.length" title="24 小时内截止" subtitle="今天和明天需要处理。" dense>
        <div class="task-list">
          <article v-for="item in buckets.today" :key="item.id" class="task-card today">
            <div class="task-card__main">
              <div class="task-card__head">
                <strong>{{ item.title }}</strong>
                <span class="badge warning">{{ item.remainLabel }}</span>
              </div>
              <p>{{ item.courseName }}</p>
            </div>
            <div class="task-card__meta">
              <time>{{ item.deadlineLabel }}</time>
              <ActionPill v-if="item.linkUrl" tone="warning" @click="copyLink(item)">复制链接</ActionPill>
            </div>
          </article>
        </div>
      </SectionCard>

      <SectionCard v-if="buckets.week.length" title="7 天内" subtitle="本周排程。" dense>
        <div class="task-list">
          <article v-for="item in buckets.week" :key="item.id" class="task-card week">
            <div class="task-card__main">
              <div class="task-card__head">
                <strong>{{ item.title }}</strong>
                <span class="badge accent">{{ item.remainLabel }}</span>
              </div>
              <p>{{ item.courseName }}</p>
            </div>
            <div class="task-card__meta">
              <time>{{ item.deadlineLabel }}</time>
              <ActionPill v-if="item.linkUrl" tone="accent" @click="copyLink(item)">复制链接</ActionPill>
            </div>
          </article>
        </div>
      </SectionCard>

      <SectionCard v-if="buckets.later.length" title="更晚" subtitle="暂时不抢首屏注意力。" dense>
        <div class="task-list">
          <article v-for="item in buckets.later" :key="item.id" class="task-card later">
            <div class="task-card__main">
              <div class="task-card__head">
                <strong>{{ item.title }}</strong>
                <span class="badge">{{ item.remainLabel }}</span>
              </div>
              <p>{{ item.courseName }}</p>
            </div>
            <div class="task-card__meta">
              <time>{{ item.deadlineLabel }}</time>
              <ActionPill v-if="item.linkUrl" @click="copyLink(item)">复制链接</ActionPill>
            </div>
          </article>
        </div>
      </SectionCard>
    </div>
  </div>
</template>

<style scoped>
.task-view,
.task-sections,
.task-list {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.task-grid {
  display: grid;
  grid-template-columns: minmax(300px, 0.95fr) minmax(0, 1.05fr);
  gap: 1rem;
}

.task-stats {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 0.7rem;
}

.task-card {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 0.85rem;
  align-items: center;
  padding: 0.95rem 0.2rem;
  border-bottom: 1px solid var(--border-subtle);
}

.task-card:last-child {
  border-bottom: none;
  padding-bottom: 0;
}

.task-card__main {
  min-width: 0;
}

.task-card__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.6rem;
}

.task-card__head strong {
  color: var(--text-primary);
}

.task-card__main p,
.task-card__meta time {
  margin: 0.28rem 0 0;
  color: var(--text-secondary);
}

.task-card__meta {
  display: flex;
  align-items: center;
  gap: 0.7rem;
}

@media (max-width: 900px) {
  .task-grid {
    grid-template-columns: 1fr;
  }

  .task-stats {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 720px) {
  .task-card {
    grid-template-columns: 1fr;
  }

  .task-card__meta {
    justify-content: space-between;
    flex-wrap: wrap;
  }
}
</style>
