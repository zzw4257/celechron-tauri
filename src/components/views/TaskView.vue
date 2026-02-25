<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";

const isLoading = ref(true);
const isOffline = ref(false);
const offlineTime = ref("");

interface TaskItem {
  id: string;
  name: string;
  course: string;
  deadline: string;
  daysLeft: number;
}

const categorizedTasks = ref({
  overdue: [] as TaskItem[],
  today: [] as TaskItem[],
  week: [] as TaskItem[],
  later: [] as TaskItem[],
});

function formatDaysLeft(days: number) {
  if (days < 0) return `逾期 ${Math.abs(Math.ceil(days))} 天`;
  if (days === 0) return "今天内截止";
  if (days === 1) return "明天截止";
  return `${Math.ceil(days)} 天后`;
}

async function fetchTasks() {
  try {
    isLoading.value = true;
    const response: any = await invoke("fetch_todos");
    
    if (response._meta && response._meta.source === "cache") {
      isOffline.value = true;
      offlineTime.value = new Date(response._meta.timestamp * 1000).toLocaleString('zh-CN', { hour12: false });
    } else {
      isOffline.value = false;
    }

    const list = response.todo_list || [];
    const now = Date.now();

    const overdue: TaskItem[] = [];
    const today: TaskItem[] = [];
    const week: TaskItem[] = [];
    const later: TaskItem[] = [];

    list.forEach((t: any) => {
      const timeMs = new Date(t.end_time).getTime();
      const daysLeft = (timeMs - now) / 86400000;
      
      const item: TaskItem = {
        id: t.id || Math.random().toString(),
        name: t.title,
        course: t.course_name,
        deadline: new Date(t.end_time).toLocaleString('zh-CN', { month: 'short', day: 'numeric', hour: '2-digit', minute:'2-digit' }),
        daysLeft
      };

      if (daysLeft < 0) {
        overdue.push(item);
      } else if (daysLeft <= 1) {
        today.push(item);
      } else if (daysLeft <= 7) {
        week.push(item);
      } else {
        later.push(item);
      }
    });

    // Sort heavily
    overdue.sort((a, b) => a.daysLeft - b.daysLeft);
    today.sort((a, b) => a.daysLeft - b.daysLeft);
    week.sort((a, b) => a.daysLeft - b.daysLeft);
    later.sort((a, b) => a.daysLeft - b.daysLeft);

    categorizedTasks.value = { overdue, today, week, later };
  } catch (e) {
    console.error("Failed to fetch tasks:", e);
  } finally {
    isLoading.value = false;
  }
}

onMounted(() => {
  fetchTasks();
});
</script>

<template>
  <div class="task-view">
    <header class="task-header">
      <h1>任务</h1>
    </header>

    <!-- Offline Warning Banner -->
    <div v-if="isOffline" class="offline-banner">
      <span class="offline-icon">⚠️</span>
      <div class="offline-text">
        <strong>网络连接异常，暂未同步最新数据。</strong>
        当前显示的是缓存在本地的数据 (更新于: {{ offlineTime }})
      </div>
    </div>

    <div v-if="isLoading && !isOffline" class="loading-state">
      正在同步学在浙大...
    </div>

    <!-- Empty State -->
    <div class="empty-state" v-if="!isLoading && Object.values(categorizedTasks).every(arr => arr.length === 0)">
      🎉 太棒了！最近没有任何待办任务！
    </div>

    <div class="task-sections">
      <!-- Overdue -->
      <section class="section-card overdue-sec" v-if="categorizedTasks.overdue.length > 0">
        <div class="section-title">🚨 已逾期 ({{ categorizedTasks.overdue.length }})</div>
        <div class="task-list">
          <div class="task-card urgent" v-for="t in categorizedTasks.overdue" :key="t.id">
            <div class="task-info">
              <span class="task-name">{{ t.name }}</span>
              <span class="task-course">{{ t.course }}</span>
            </div>
            <div class="task-meta">
              <span class="task-ddl">{{ t.deadline }}</span>
              <span class="task-days badge-red">{{ formatDaysLeft(t.daysLeft) }}</span>
            </div>
          </div>
        </div>
      </section>

      <!-- Today -->
      <section class="section-card today-sec" v-if="categorizedTasks.today.length > 0">
        <div class="section-title">🔥 今天截止 ({{ categorizedTasks.today.length }})</div>
        <div class="task-list">
          <div class="task-card urgent" v-for="t in categorizedTasks.today" :key="t.id">
            <div class="task-info">
              <span class="task-name">{{ t.name }}</span>
              <span class="task-course">{{ t.course }}</span>
            </div>
            <div class="task-meta">
              <span class="task-ddl">{{ t.deadline }}</span>
              <span class="task-days badge-orange">{{ formatDaysLeft(t.daysLeft) }}</span>
            </div>
          </div>
        </div>
      </section>

      <!-- Week -->
      <section class="section-card week-sec" v-if="categorizedTasks.week.length > 0">
        <div class="section-title">📅 一周内 ({{ categorizedTasks.week.length }})</div>
        <div class="task-list">
          <div class="task-card" v-for="t in categorizedTasks.week" :key="t.id">
            <div class="task-info">
              <span class="task-name">{{ t.name }}</span>
              <span class="task-course">{{ t.course }}</span>
            </div>
            <div class="task-meta">
              <span class="task-ddl">{{ t.deadline }}</span>
              <span class="task-days badge-blue">{{ formatDaysLeft(t.daysLeft) }}</span>
            </div>
          </div>
        </div>
      </section>

      <!-- Later -->
      <section class="section-card later-sec" v-if="categorizedTasks.later.length > 0">
        <div class="section-title">📌 以后 ({{ categorizedTasks.later.length }})</div>
        <div class="task-list">
          <div class="task-card" v-for="t in categorizedTasks.later" :key="t.id">
            <div class="task-info">
              <span class="task-name">{{ t.name }}</span>
              <span class="task-course">{{ t.course }}</span>
            </div>
            <div class="task-meta">
              <span class="task-ddl">{{ t.deadline }}</span>
              <span class="task-days badge-gray">{{ formatDaysLeft(t.daysLeft) }}</span>
            </div>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.task-view {
  padding: 2rem 2.5rem 6rem;
  max-width: 950px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.task-header h1 {
  font-size: 1.8rem;
  font-weight: 700;
  margin: 0;
  background: linear-gradient(135deg, #e2e8f0, #f8fafc);
  background-clip: text;
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
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

.loading-state, .empty-state {
  text-align: center;
  color: #94a3b8;
  padding: 3rem 0;
  font-size: 1.1rem;
}

.task-sections {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

/* Section Card styled for Liquid Glass */
.section-card {
  background: rgba(255,255,255,0.03);
  border: 1px solid rgba(255,255,255,0.06);
  border-radius: 20px;
  padding: 1.5rem;
  backdrop-filter: blur(12px);
}
.section-title {
  font-size: 1.1rem;
  font-weight: 700;
  color: #e2e8f0;
  margin-bottom: 1rem;
}

.task-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.task-card {
  background: rgba(0,0,0,0.3);
  border: 1px solid rgba(255,255,255,0.05);
  border-radius: 14px;
  padding: 1rem 1.2rem;
  display: flex;
  justify-content: space-between;
  align-items: center;
  transition: transform 0.2s, background 0.2s;
  cursor: default;
}
.task-card:hover {
  background: rgba(255,255,255,0.03);
  transform: translateX(4px);
}
.task-card.urgent {
  border-left: 4px solid #ef4444;
}

.task-info {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.task-name {
  font-size: 1rem;
  font-weight: 600;
  color: #f8fafc;
}
.task-course {
  font-size: 0.75rem;
  color: #94a3b8;
}

.task-meta {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 6px;
}
.task-ddl {
  font-size: 0.8rem;
  color: #cbd5e1;
  font-variant-numeric: tabular-nums;
}

.badge-red { background: rgba(239,68,68,0.2); color: #fca5a5; padding: 2px 8px; border-radius: 12px; font-size: 0.7rem; font-weight: 600; }
.badge-orange { background: rgba(249,115,22,0.2); color: #fdba74; padding: 2px 8px; border-radius: 12px; font-size: 0.7rem; font-weight: 600; }
.badge-blue { background: rgba(56,189,248,0.2); color: #7dd3fc; padding: 2px 8px; border-radius: 12px; font-size: 0.7rem; font-weight: 600; }
.badge-gray { background: rgba(148,163,184,0.2); color: #cbd5e1; padding: 2px 8px; border-radius: 12px; font-size: 0.7rem; font-weight: 600; }

@media (max-width: 600px) {
  .task-view { padding: 1rem 1rem 6rem; }
  .task-card { flex-direction: column; align-items: flex-start; gap: 10px; }
  .task-meta { align-items: flex-start; flex-direction: row-reverse; width: 100%; justify-content: space-between; }
}

/* Light Mode Overrides */
:root.light-theme .task-header h1 {
  background: linear-gradient(135deg, #1e293b, #334155);
  background-clip: text;
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}
:root.light-theme .offline-banner {
  background: rgba(245, 158, 11, 0.1);
  color: #d97706;
  border-color: rgba(245, 158, 11, 0.3);
}
:root.light-theme .offline-text strong {
  color: #b45309;
}
:root.light-theme .loading-state,
:root.light-theme .empty-state {
  color: #64748b;
}
:root.light-theme .section-card {
  background: rgba(255, 255, 255, 0.7);
  border-color: rgba(0, 0, 0, 0.08);
  box-shadow: 0 4px 12px rgba(0,0,0,0.03);
}
:root.light-theme .section-title {
  color: #1e293b;
}
:root.light-theme .task-card {
  background: rgba(255, 255, 255, 0.6);
  border-color: rgba(0, 0, 0, 0.05);
}
:root.light-theme .task-card:hover {
  background: rgba(0, 0, 0, 0.02);
}
:root.light-theme .task-name {
  color: #1e293b;
}
:root.light-theme .task-course {
  color: #64748b;
}
:root.light-theme .task-ddl {
  color: #64748b;
}
:root.light-theme .badge-red { background: rgba(220, 38, 38, 0.1); color: #dc2626; }
:root.light-theme .badge-orange { background: rgba(234, 88, 12, 0.1); color: #ea580c; }
:root.light-theme .badge-blue { background: rgba(2, 132, 199, 0.1); color: #0284c7; }
:root.light-theme .badge-gray { background: rgba(100, 116, 139, 0.1); color: #475569; }

</style>
