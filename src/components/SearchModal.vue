<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue';
import { Search, Calendar, BookOpen, AlertCircle } from 'lucide-vue-next';
import { fetchScholarData, fetchTodos } from '../services/api';
import { formatTermDisplayName } from '../utils/semester';

const isVisible = ref(false);
const searchQuery = ref('');
const selectedIndex = ref(-1);
const searchInput = ref<HTMLInputElement | null>(null);

const coursesList = ref<any[]>([]);
const examsList = ref<any[]>([]);
const todosList = ref<any[]>([]);
const isLoading = ref(false);
const typeColors = ref<Record<string, string>>({
  course: '#3b82f6',
  exam: '#f59e0b',
  todo: '#10b981',
  default: '#94a3b8',
});

function readCssVar(name: string, fallback: string) {
  if (typeof window === 'undefined') return fallback;
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

function syncTypeColors() {
  typeColors.value = {
    course: readCssVar('--accent-blue', '#3b82f6'),
    exam: readCssVar('--accent-amber', '#f59e0b'),
    todo: readCssVar('--accent-green', '#10b981'),
    default: readCssVar('--text-muted', '#94a3b8'),
  };
}

async function loadData() {
  isLoading.value = true;
  try {
    const [scholarEnv, todosEnv] = await Promise.all([fetchScholarData(), fetchTodos()]);
    const scholar = scholarEnv.data;
    const todosData = todosEnv.data;

    const flattenedCourses = (scholar.semesters || []).flatMap((semester) =>
      (semester.grades || []).map((grade: any) => ({
        type: 'course',
        id: grade.xkkh || `${semester.name}-${grade.kcmc}`,
        title: grade.kcmc,
        subtitle: `教师: ${grade.jsxm || '未知'} | 学分: ${grade.credit ?? grade.xf ?? 0} | 学期: ${formatTermDisplayName(semester.term, semester.name)}`,
        score: grade.cj,
        raw: grade,
      })),
    );

    const uniqueCourses = new Map<string, any>();
    for (const item of flattenedCourses) {
      uniqueCourses.set(item.id, item);
    }
    coursesList.value = Array.from(uniqueCourses.values());

    examsList.value = (scholar.exams || []).map((exam: any) => ({
      type: 'exam',
      id: exam.xkkh || exam.kcmc,
      title: `${exam.kcmc} 考试`,
      subtitle: `时间: ${exam.kssj || exam.qzkssj || exam.time?.[0] || '未知'} | 地点: ${exam.ksdd || exam.qzksdd || exam.location?.[0] || '未知'}`,
      raw: exam,
    }));

    todosList.value = (todosData.todo_list || []).map((todo: any) => ({
      type: 'todo',
      id: todo.id || todo.title,
      title: todo.title,
      subtitle: `所属课程: ${todo.course_name} | 截止: ${new Date(todo.end_time).toLocaleString('zh-CN', { hour12: false })}`,
      raw: todo,
    }));
  } catch (error) {
    console.error('Search data load failed:', error);
  } finally {
    isLoading.value = false;
  }
}

function handleKeydown(event: KeyboardEvent) {
  if ((event.metaKey || event.ctrlKey) && event.key === 'k') {
    event.preventDefault();
    toggleSearch();
  }

  if (!isVisible.value) return;

  if (event.key === 'Escape') {
    closeSearch();
  } else if (event.key === 'ArrowDown') {
    event.preventDefault();
    selectedIndex.value = Math.min(selectedIndex.value + 1, filteredResults.value.length - 1);
  } else if (event.key === 'ArrowUp') {
    event.preventDefault();
    selectedIndex.value = Math.max(selectedIndex.value - 1, 0);
  } else if (event.key === 'Enter' && selectedIndex.value >= 0) {
    onItemSelect(filteredResults.value[selectedIndex.value]);
  }
}

function toggleSearch() {
  isVisible.value = !isVisible.value;
  if (isVisible.value) {
    syncTypeColors();
    searchQuery.value = '';
    selectedIndex.value = -1;
    loadData();
    setTimeout(() => searchInput.value?.focus(), 100);
  }
}

function closeSearch() {
  isVisible.value = false;
}

const filteredResults = computed(() => {
  const query = searchQuery.value.trim().toLowerCase();
  if (!query) return [];

  return [...coursesList.value, ...examsList.value, ...todosList.value]
    .filter((item) => item.title.toLowerCase().includes(query) || item.subtitle.toLowerCase().includes(query))
    .slice(0, 15);
});

function onItemSelect(item: any) {
  console.log('Selected:', item);
  closeSearch();
}

onMounted(() => {
  syncTypeColors();
  window.addEventListener('keydown', handleKeydown);
  (window as any).__toggleGlobalSearch = toggleSearch;
});

onUnmounted(() => {
  window.removeEventListener('keydown', handleKeydown);
});

const getIcon = (type: string) => {
  if (type === 'course') return BookOpen;
  if (type === 'exam') return Calendar;
  if (type === 'todo') return AlertCircle;
  return Search;
};

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

:global(html[data-theme='light']) .search-overlay {
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

:global(html[data-theme='light']) .esc-kbd {
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.05);
}
</style>
