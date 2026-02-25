<script setup lang="ts">
import { ref, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { LiquidGlass } from '@wxperia/liquid-glass-vue';

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

const periods = [
  { label: "1-2节", time: "08:00", ms: 8 * 3600000 },
  { label: "3-4节", time: "09:50", ms: 9 * 3600000 + 50 * 60000 },
  { label: "5-6节", time: "13:15", ms: 13 * 3600000 + 15 * 60000 },
  { label: "7-8节", time: "15:05", ms: 15 * 3600000 + 5 * 60000 },
  { label: "9-10节", time: "18:50", ms: 18 * 3600000 + 50 * 60000 },
  { label: "11-12节", time: "20:40", ms: 20 * 3600000 + 40 * 60000 },
];

const colors = ["#06b6d4", "#8b5cf6", "#f97316", "#22c55e", "#ec4899", "#eab308"];

async function loadFlow() {
  isLoading.value = true;
  const newItems: FlowItem[] = [];
  
  try {
    const today = new Date();
    today.setHours(0,0,0,0);
    const todayMs = today.getTime();
    
    // 1. Fetch Todos
    const todoRes: any = await invoke("fetch_todos");
    if (todoRes._meta && todoRes._meta.source === "cache") isOffline.value = true;
    
    const todos = todoRes.todo_list || [];
    todos.forEach((t: any) => {
      const ms = new Date(t.end_time).getTime();
      // Keep tasks due in the next 7 days
      if (ms > todayMs && ms < todayMs + 86400000 * 7) {
        newItems.push({
          id: `task-${t.id}`,
          type: 'task',
          title: t.title,
          subtitle: t.course_name || '学在浙大',
          timeLabel: new Date(t.end_time).toLocaleString('zh-CN', { month:'short', day:'numeric', hour:'2-digit', minute:'2-digit'}),
          timeMs: ms,
          color: '#ef4444' // Red for tasks
        });
      }
    });

    // Determine current semester
    const now = new Date();
    const month = now.getMonth() + 1;
    const year = now.getFullYear();
    let zjuYearStr = '';
    let zjuSemStr = '';
    
    if (month >= 2 && month <= 8) {
      zjuSemStr = "2"; // 春夏
      zjuYearStr = `${year - 1}-${year}`;
    } else {
      zjuSemStr = "1"; // 秋冬
      const startYear = month === 1 ? year - 1 : year;
      zjuYearStr = `${startYear}-${startYear + 1}`;
    }

    // 2. Fetch Timetable
    const ttRes: any = await invoke("fetch_timetable", { year: zjuYearStr, semester: zjuSemStr });
    const timetable = ttRes.timetable || [];
    
    const startDateMs = parseInt(localStorage.getItem('semester_start_ms') || '0');
    let colorIdx = 0;
    const courseColors: any = {};

    // 7-day lookahead for courses
    for (let dayOffset = 0; dayOffset < 7; dayOffset++) {
      const d = new Date(todayMs + dayOffset * 86400000);
      let dayStr = d.getDay().toString();
      if (dayStr === "0") dayStr = "7"; // Sunday

      let activeWeek = 1;
      if (startDateMs > 0) {
        activeWeek = Math.floor((d.getTime() - startDateMs) / (7 * 24 * 3600000)) + 1;
      }

      timetable.forEach((session: any) => {
        const sDayStr = session.xqj || session.xq;
        const periodStr = session.jcs || session.jc;
        const zcsStr = session.zcs || session.zc || "1-16";
        
        if (sDayStr === dayStr) {
           let isActive = false;
           const parts = zcsStr.split(",");
           parts.forEach((p: string) => {
               const range = p.split("-");
               if (range.length === 2 && activeWeek >= parseInt(range[0]) && activeWeek <= parseInt(range[1])) isActive = true;
               else if (range.length === 1 && parseInt(range[0]) === activeWeek) isActive = true;
           });

           // Apply odd/even filtering
           const dsz = (session.dsz || '').trim();
           if (dsz === '1' && activeWeek % 2 === 0) isActive = false;
           if (dsz === '0' && activeWeek % 2 !== 0) isActive = false;

           if (isActive) {
             const pParts = periodStr.split("-");
             if (pParts.length > 0) {
                let startP = parseInt(pParts[0]);
                let mappedIdx = startP > 10 ? 5 : Math.floor((startP - 1) / 2);
                if (mappedIdx >= 0 && mappedIdx < periods.length) {
                  const name = session.kcmc || "";
                  if (!courseColors[name]) courseColors[name] = colors[colorIdx++ % colors.length];
                  
                  const courseMs = d.getTime() + periods[mappedIdx].ms;

                  // Label formatting
                  let dayLabel = dayOffset === 0 ? "今天" : dayOffset === 1 ? "明天" : `${d.getMonth()+1}月${d.getDate()}日`;

                  newItems.push({
                    id: `course-${name}-${startP}-${dayOffset}`,
                    type: 'course',
                    title: name,
                    subtitle: session.cdmc || '无地点',
                    timeLabel: `${dayLabel} ${periods[mappedIdx].time}`,
                    timeMs: courseMs,
                    color: courseColors[name]
                  });
                }
             }
           }
        }
      });
    }

    // Sort all by absolute time
    newItems.sort((a, b) => a.timeMs - b.timeMs);
    items.value = newItems;

  } catch (e) {
    console.error(e);
  } finally {
    isLoading.value = false;
  }
}

onMounted(() => {
  loadFlow();
});
</script>

<template>
  <div class="flow-view">
    <header class="section-header">
      <h2>接下来 <span style="color:#64748b; font-weight:400; font-size:1.2rem;">(Flow)</span></h2>
      <div v-if="isOffline" class="offline-badge">离线模式</div>
    </header>

    <div v-if="isLoading" class="loading-state">
      <div class="loader"></div>
    </div>

    <div v-else-if="items.length === 0" class="empty-state">
      <span style="font-size: 3rem; margin-bottom: 1rem;">🌴</span>
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
  color: #e2e8f0;
}
.offline-badge {
  background: rgba(245, 158, 11, 0.2);
  color: #fbbf24;
  padding: 4px 12px;
  border-radius: 20px;
  font-size: 0.85rem;
  border: 1px solid rgba(245, 158, 11, 0.4);
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
  background: linear-gradient(to bottom, rgba(255,255,255,0.1), transparent);
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
  border: 2px solid #0f172a;
  z-index: 2;
}
.time-text {
  font-size: 0.9rem;
  font-weight: 600;
  color: #94a3b8;
}

.card-wrapper {
  width: 100%;
}
.item-card {
  padding: 1.2rem;
  display: flex;
  align-items: flex-start;
  gap: 15px;
  background: color-mix(in srgb, var(--accent) 5%, transparent);
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
  color: #f8fafc;
}
.content p {
  margin: 0;
  color: #94a3b8;
  font-size: 0.85rem;
}

.loading-state, .empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 400px;
  color: #64748b;
  text-align: center;
}

.loader {
  width: 30px;
  height: 30px;
  border: 3px solid rgba(255, 255, 255, 0.1);
  border-radius: 50%;
  border-top-color: #38bdf8;
  animation: spin 1s linear infinite;
}

@keyframes spin { to { transform: rotate(360deg); } }
@keyframes fade-in {
  from { opacity: 0; transform: translateX(-10px); }
  to { opacity: 1; transform: translateX(0); }
}

/* Light Mode Overrides */
:root.light-theme .section-header h2 {
  color: #1e293b;
}
:root.light-theme .offline-badge {
  background: rgba(245, 158, 11, 0.1);
  color: #d97706;
  border-color: rgba(245, 158, 11, 0.3);
}
:root.light-theme .timeline::before {
  background: linear-gradient(to bottom, rgba(0,0,0,0.1), transparent);
}
:root.light-theme .time-dot {
  border-color: #f0f4f8;
}
:root.light-theme .time-text {
  color: #334155;
}
:root.light-theme .item-card {
  background: rgba(255, 255, 255, 0.75);
  box-shadow: 0 4px 12px rgba(0,0,0,0.05);
}
:root.light-theme .content h4 {
  color: #1e293b;
}
:root.light-theme .content p {
  color: #64748b;
}
:root.light-theme .badge.course {
  background: rgba(2, 132, 199, 0.1);
  color: #0284c7;
}
:root.light-theme .badge.task {
  background: rgba(220, 38, 38, 0.1);
  color: #dc2626;
}
:root.light-theme .empty-state,
:root.light-theme .loading-state {
  color: #64748b;
}
:root.light-theme .loader {
  border: 3px solid rgba(0, 0, 0, 0.1);
  border-top-color: #0284c7;
}

</style>
