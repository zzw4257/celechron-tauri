<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue';
import { Search, Calendar, BookOpen, AlertCircle } from 'lucide-vue-next';
import { fetchScholarData, fetchTodos } from '../services/api';

function formatSemesterName(name: string) {
  if (!name || !name.includes('-')) return name;
  const parts = name.split('-');
  if (parts.length < 3) return name;
  const startYear = parts[0].slice(-2);
  const endYear = parts[1].slice(-2);
  const semType = parts[2] === '1' ? '秋冬' : (parts[2] === '2' ? '春夏' : '短');
  return `${startYear}-${endYear} ${semType}`;
}

const isVisible = ref(false);
const searchQuery = ref('');
const selectedIndex = ref(-1);

const searchInput = ref<HTMLInputElement | null>(null);

// Raw Data
const coursesList = ref<any[]>([]);
const examsList = ref<any[]>([]);
const todosList = ref<any[]>([]);
const isLoading = ref(false);
const typeColors = ref<Record<string, string>>({
  course: "#3b82f6",
  exam: "#f59e0b",
  todo: "#10b981",
  default: "#94a3b8",
});

function readCssVar(name: string, fallback: string) {
  if (typeof window === "undefined") return fallback;
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

function syncTypeColors() {
  typeColors.value = {
    course: readCssVar("--accent-blue", "#3b82f6"),
    exam: readCssVar("--accent-amber", "#f59e0b"),
    todo: readCssVar("--accent-green", "#10b981"),
    default: readCssVar("--text-muted", "#94a3b8"),
  };
}

async function loadData() {
  if (coursesList.value.length > 0) return; // already loaded
  
  isLoading.value = true;
  try {
    const scholarEnv = await fetchScholarData();
    const todosEnv = await fetchTodos();
    const data: any = scholarEnv.data;
    const todosData: any = todosEnv.data;

    // 1. Flatten all semesters' courses
    const allC: any[] = [];
    (data.semesters || []).forEach((sem: any) => {
      sem.grades.forEach((g: any) => {
        allC.push({
          type: 'course',
          id: g.xkkh,
          title: g.kcmc,
          subtitle: `教师: ${g.jsxm || '未知'} | 学分: ${g.xf} | 学期: ${formatSemesterName(sem.name)}`,
          score: g.cj,
          raw: g
        });
      });
    });
    
    // De-duplicate courses by xkkh if needed
    const uniqueCourses = new Map();
    for (const c of allC) {
       uniqueCourses.set(c.id, c);
    }
    coursesList.value = Array.from(uniqueCourses.values());

    // 2. Exams
    examsList.value = (data.exams || []).map((e: any) => ({
      type: 'exam',
      id: e.xkkh || e.kcmc,
      title: `${e.kcmc} 考试`,
      subtitle: `时间: ${e.kssj || e.qzkssj || e.time?.[0] || '未知'} | 地点: ${e.ksdd || e.qzksdd || e.location?.[0] || '未知'}`,
      raw: e
    }));

    // 3. Todos
    todosList.value = (todosData.todo_list || []).map((t: any) => ({
      type: 'todo',
      id: t.id || t.title,
      title: t.title,
      subtitle: `所属课程: ${t.course_name} | 截止: ${new Date(t.end_time).toLocaleString()}`,
      raw: t
    }));

  } catch (e) {
    console.error("Search data load failed:", e);
  } finally {
    isLoading.value = false;
  }
}

// Global keyboard shortcut
function handleKeydown(e: KeyboardEvent) {
  if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
    e.preventDefault();
    toggleSearch();
  }
  
  if (!isVisible.value) return;

  if (e.key === 'Escape') {
    closeSearch();
  } else if (e.key === 'ArrowDown') {
    e.preventDefault();
    selectedIndex.value = Math.min(selectedIndex.value + 1, filteredResults.value.length - 1);
  } else if (e.key === 'ArrowUp') {
    e.preventDefault();
    selectedIndex.value = Math.max(selectedIndex.value - 1, 0);
  } else if (e.key === 'Enter' && selectedIndex.value >= 0) {
    const item = filteredResults.value[selectedIndex.value];
    onItemSelect(item);
  }
}

function toggleSearch() {
  isVisible.value = !isVisible.value;
  if (isVisible.value) {
    syncTypeColors();
    searchQuery.value = '';
    selectedIndex.value = -1;
    loadData();
    setTimeout(() => {
      searchInput.value?.focus();
    }, 100);
  }
}

function closeSearch() {
  isVisible.value = false;
}

const filteredResults = computed(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return [];
  
  const all = [...coursesList.value, ...examsList.value, ...todosList.value];
  return all.filter(item => 
    item.title.toLowerCase().includes(q) || 
    item.subtitle.toLowerCase().includes(q)
  ).slice(0, 15); // limit to 15 results
});

function onItemSelect(item: any) {
  // Can expand routing or popups later based on selection.
  // For now, it just closes the search acting as an info lookup.
  console.log("Selected:", item);
  closeSearch();
}

onMounted(() => {
  syncTypeColors();
  window.addEventListener('keydown', handleKeydown);
  // Optional: Expose toggle globally to be called from MainLayout buttons
  (window as any).__toggleGlobalSearch = toggleSearch;
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeydown);
});

// Icons mapping helper
const getIcon = (type: string) => {
  if (type === 'course') return BookOpen;
  if (type === 'exam') return Calendar;
  if (type === 'todo') return AlertCircle;
  return Search;
};

// Color mapping helper
const getColor = (type: string) => {
  if (type === 'course') return typeColors.value.course;
  if (type === 'exam') return typeColors.value.exam;
  if (type === 'todo') return typeColors.value.todo;
  return typeColors.value.default;
};

const getIconStyle = (type: string) => {
  const color = getColor(type);
  return {
    color,
    background: `${color}22`,
  };
};
</script>

<template>
  <div v-if="isVisible" class="search-overlay" @click.self="closeSearch">
    <div class="search-modal glass-panel">
      <div class="search-header">
        <Search class="search-icon" :size="20"/>
        <input 
          ref="searchInput"
          v-model="searchQuery" 
          type="text" 
          placeholder="搜索课程、考试、待办事项... (Cmd/Ctrl + K)"
          class="search-input"
          @input="selectedIndex = -1"
        />
        <div v-if="isLoading" class="loader"></div>
        <kbd class="esc-kbd">ESC</kbd>
      </div>

      <div class="search-results" v-if="searchQuery">
        <div v-if="filteredResults.length === 0" class="no-results">
          找不到匹配的内容
        </div>
        <div 
          v-else
          v-for="(item, idx) in filteredResults" 
          :key="item.id + idx"
          class="result-item"
          :class="{ 'selected': idx === selectedIndex }"
          @click="onItemSelect(item)"
          @mouseenter="selectedIndex = idx"
        >
          <div class="item-icon" :style="getIconStyle(item.type)">
             <component :is="getIcon(item.type)" :size="18"/>
          </div>
          <div class="item-content">
            <div class="item-title">{{ item.title }}</div>
            <div class="item-subtitle">{{ item.subtitle }}</div>
          </div>
          <div class="item-score" v-if="item.score && !['待录','无效','缓考'].includes(item.score.toString())">
            {{ item.score }}
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.search-overlay {
  --search-overlay-bg: rgba(0, 0, 0, 0.4);
  --search-modal-bg: rgba(30, 41, 59, 0.9);
  --search-modal-border: rgba(255, 255, 255, 0.15);
  --search-modal-shadow: 0 20px 40px rgba(0, 0, 0, 0.3);
  --search-header-border: rgba(255, 255, 255, 0.08);
  --search-icon: #94a3b8;
  --search-text: #f8fafc;
  --search-placeholder: #64748b;
  --search-kbd-bg: rgba(255, 255, 255, 0.1);
  --search-kbd-border: rgba(255, 255, 255, 0.1);
  --search-muted: #94a3b8;
  --search-selected-bg: rgba(255, 255, 255, 0.08);
  --search-success: var(--accent-green);
  --search-loader: var(--accent-blue);

  position: fixed;
  inset: 0;
  background: var(--search-overlay-bg);
  backdrop-filter: blur(4px);
  z-index: 99999;
  display: flex;
  justify-content: center;
  align-items: flex-start;
  padding-top: 15vh;
}

.search-modal {
  width: 90%;
  max-width: 600px;
  background: var(--search-modal-bg);
  border: 1px solid var(--search-modal-border);
  border-radius: 16px;
  box-shadow: var(--search-modal-shadow);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.search-header {
  display: flex;
  align-items: center;
  padding: 16px 20px;
  border-bottom: 1px solid var(--search-header-border);
  gap: 12px;
}

.search-icon {
  color: var(--search-icon);
}

.search-input {
  flex: 1;
  background: transparent;
  border: none;
  font-size: 1.1rem;
  color: var(--search-text);
  outline: none;
}
.search-input::placeholder {
  color: var(--search-placeholder);
}

.esc-kbd {
  font-size: 0.7rem;
  color: var(--search-muted);
  background: var(--search-kbd-bg);
  padding: 2px 6px;
  border-radius: 4px;
  border: 1px solid var(--search-kbd-border);
  font-family: monospace;
}

.search-results {
  max-height: 50vh;
  overflow-y: auto;
  padding: 8px;
}

.no-results {
  padding: 30px;
  text-align: center;
  color: var(--search-muted);
  font-size: 0.95rem;
}

.result-item {
  display: flex;
  align-items: center;
  padding: 12px 16px;
  gap: 16px;
  border-radius: 12px;
  cursor: pointer;
  transition: background 0.15s;
}

.result-item.selected {
  background: var(--search-selected-bg);
}

.item-icon {
  width: 36px;
  height: 36px;
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.item-content {
  flex: 1;
  overflow: hidden;
}

.item-title {
  font-size: 1rem;
  color: var(--search-text);
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.item-subtitle {
  font-size: 0.8rem;
  color: var(--search-muted);
  margin-top: 2px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.item-score {
  font-family: 'JetBrains Mono', monospace;
  font-weight: 700;
  font-size: 1.1rem;
  color: var(--search-success);
}

@keyframes spin {
  to { transform: rotate(360deg); }
}
.loader {
  width: 16px; height: 16px;
  border: 2px solid transparent;
  border-top-color: var(--search-loader);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

:global(.light-theme) .search-overlay {
  --search-modal-bg: rgba(255, 255, 255, 0.98);
  --search-modal-border: rgba(0, 0, 0, 0.08);
  --search-modal-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.15);
  --search-header-border: rgba(0, 0, 0, 0.05);
  --search-text: #0f172a;
  --search-placeholder: #94a3b8;
  --search-muted: #64748b;
  --search-selected-bg: rgba(0, 0, 0, 0.04);
  --search-kbd-bg: #f8fafc;
  --search-kbd-border: #cbd5e1;
}

:global(.light-theme) .esc-kbd {
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
}
</style>
