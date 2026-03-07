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
const refreshStatus = ref('');
const pomodoroExpanded = ref(false);
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

function refreshFallbackMessage(meta?: { requestedFresh?: boolean; source?: string; fallbackReason?: string }) {
  if (!meta?.requestedFresh || meta.source !== 'cache') return '';
  return `已尝试强制刷新，但网络失败，当前回退到本地缓存${meta.fallbackReason ? `：${meta.fallbackReason}` : ''}。`;
}

async function copyLink(item: TaskBucketItem) {
  if (!item.linkUrl) return;
  await navigator.clipboard.writeText(item.linkUrl);
  copiedId.value = item.id;
  window.setTimeout(() => {
    if (copiedId.value === item.id) copiedId.value = '';
  }, 1600);
}

async function loadTasks(forceRefresh = false) {
  isLoading.value = true;
  errorMsg.value = '';
  copiedId.value = '';
  refreshStatus.value = '';

  try {
    const env = await fetchTodos({ forceRefresh });
    refreshStatus.value = refreshFallbackMessage(env._meta as any);
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
watch(accountScope, () => {
  void loadTasks();
});
</script>

<template>
  <div class="page-shell task-view">
    <header class="page-header">
      <div>
        <h1>任务</h1>
        <p class="page-subtitle">任务与接下来优先；番茄钟保留为次级工具，只在你需要时展开使用。</p>
      </div>
      <div class="task-header-actions">
        <ActionPill tone="accent" :disabled="isLoading" @click="loadTasks(true)">强制刷新</ActionPill>
        <span class="badge" :class="isOffline ? 'warning' : 'accent'">{{ isOffline ? '缓存模式' : '实时同步' }}</span>
      </div>
    </header>

    <SectionCard class="task-summary-card" title="任务摘要" subtitle="首屏先看真正影响节奏的事。" dense>
      <div class="task-stats">
        <InlineStat label="总任务" :value="String(summary.total)" emphasis />
        <InlineStat label="24h 内" :value="String(summary.today)" />
        <InlineStat label="7 天内" :value="String(summary.week)" />
        <InlineStat label="已逾期" :value="String(summary.overdue)" />
      </div>
    </SectionCard>

    <StatusBanner class="task-feedback-banner" v-if="errorMsg" tone="danger" title="同步失败">
      {{ errorMsg }}
    </StatusBanner>
    <StatusBanner class="task-feedback-banner" v-else-if="refreshStatus" tone="warning" title="强制刷新回退">
      {{ refreshStatus }}
    </StatusBanner>
    <StatusBanner class="task-feedback-banner" v-else-if="isOffline && offlineTime" tone="warning" title="缓存回退">
      当前展示的是本地缓存，更新时间 {{ offlineTime }}。
    </StatusBanner>
    <StatusBanner class="task-feedback-banner" v-else-if="copiedId" tone="success" title="复制成功">
      已复制任务链接，可以直接粘贴给同学或在浏览器打开。
    </StatusBanner>

    <SectionCard v-if="isLoading" title="加载中" subtitle="正在拉取待办和截止时间。">
      <div class="state-card">请稍候，正在整理任务优先级。</div>
    </SectionCard>

    <SectionCard v-else-if="summary.total === 0" title="暂无待办" subtitle="当前没有需要你跟进的任务。">
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

    <SectionCard title="节奏工具" subtitle="默认收起，避免打断任务主视图；需要专注时再展开。" dense class="task-pomodoro-card">
      <div class="task-tool-toggle">
        <p>接下来和截止项优先。番茄钟只在你准备进入专注阶段时打开。</p>
        <ActionPill @click="pomodoroExpanded = !pomodoroExpanded">{{ pomodoroExpanded ? '收起番茄钟' : '展开番茄钟' }}</ActionPill>
      </div>
      <PomodoroWidget v-if="pomodoroExpanded" compact />
    </SectionCard>
  </div>
</template>

<style scoped>
.task-view,
.task-sections,
.task-list {
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
}

.task-header-actions {
  display: flex;
  gap: 0.75rem;
  align-items: center;
  flex-wrap: wrap;
}

.task-summary-card {
  margin-bottom: 0.05rem;
}

.task-summary-card :deep(.section-card) {
  padding-block: 0.82rem 0.88rem;
}

.task-summary-card :deep(.section-card__header) {
  margin-bottom: 0.72rem;
}

.task-summary-card :deep(.inline-stat) {
  padding: 0.74rem 0.8rem;
  gap: 0.12rem;
}

.task-summary-card :deep(.inline-stat__value) {
  font-size: 1rem;
}

.task-feedback-banner {
  padding: 0.72rem 0.82rem;
}

.task-pomodoro-card {
  margin-top: 0.1rem;
}

.task-tool-toggle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.9rem;
  flex-wrap: wrap;
}

.task-tool-toggle p {
  margin: 0;
  color: var(--text-secondary);
}

.task-stats {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 0.58rem;
}

.task-card {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 0.75rem;
  align-items: center;
  padding: 0.82rem 0.9rem;
  border: 1px solid color-mix(in srgb, var(--border-subtle) 90%, transparent);
  border-radius: calc(var(--radius-card-sm) + 2px);
  background: linear-gradient(160deg, color-mix(in srgb, white 86%, var(--surface-1)) 0%, var(--surface-1) 100%);
  box-shadow: 0 12px 24px color-mix(in srgb, var(--accent-text) 5%, transparent);
}

.task-card.overdue {
  border-color: color-mix(in srgb, var(--danger-border) 80%, var(--border-subtle));
  background: linear-gradient(160deg, color-mix(in srgb, var(--danger-soft) 88%, white) 0%, var(--surface-1) 100%);
}

.task-card.today {
  border-color: color-mix(in srgb, var(--warning-border) 78%, var(--border-subtle));
  background: linear-gradient(160deg, color-mix(in srgb, var(--warning-soft) 82%, white) 0%, var(--surface-1) 100%);
}

.task-card.week {
  border-color: color-mix(in srgb, var(--accent-border) 78%, var(--border-subtle));
  background: linear-gradient(160deg, color-mix(in srgb, var(--accent-soft) 80%, white) 0%, var(--surface-1) 100%);
}

.task-card.later {
  background: linear-gradient(160deg, color-mix(in srgb, var(--surface-2) 82%, white) 0%, var(--surface-1) 100%);
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
  font-size: 1rem;
  line-height: 1.3;
}

.task-card__main p,
.task-card__meta time {
  margin: 0.2rem 0 0;
  color: var(--text-secondary);
}

.task-card__meta time {
  font-variant-numeric: tabular-nums;
}

.task-card__meta {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.6rem;
}

@media (max-width: 900px) {
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
