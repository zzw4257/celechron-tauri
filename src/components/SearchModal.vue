<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { Search, Calendar, BookOpen, AlertCircle, LibraryBig } from 'lucide-vue-next';
import { fetchMaterials, fetchScholarData, fetchTodos } from '../services/api';
import { formatTermDisplayName } from '../utils/semester';

interface SearchResultItem {
  type: 'course' | 'exam' | 'todo' | 'material';
  id: string;
  title: string;
  subtitle: string;
  score?: string | number;
  routeTab: 'scholar' | 'task' | 'materials';
  relativePath?: string;
  remoteId?: string;
}

const isVisible = ref(false);
const searchQuery = ref('');
const selectedIndex = ref(-1);
const searchInput = ref<HTMLInputElement | null>(null);
const coursesList = ref<SearchResultItem[]>([]);
const examsList = ref<SearchResultItem[]>([]);
const todosList = ref<SearchResultItem[]>([]);
const materialsList = ref<SearchResultItem[]>([]);
const isLoading = ref(false);
const typeColors = ref<Record<string, string>>({
  course: '#3b82f6',
  exam: '#f59e0b',
  todo: '#10b981',
  material: '#8b5cf6',
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
    material: readCssVar('--accent-purple', '#8b5cf6'),
    default: readCssVar('--text-muted', '#94a3b8'),
  };
}

async function loadData() {
  isLoading.value = true;
  try {
    const [scholarEnv, todosEnv, materialsEnv] = await Promise.all([
      fetchScholarData(),
      fetchTodos(),
      fetchMaterials(),
    ]);
    const scholar = scholarEnv.data;
    const todosData = todosEnv.data;
    const materialsData = materialsEnv.data;

    const flattenedCourses = (scholar.semesters || []).flatMap((semester) =>
      (semester.grades || []).map((grade: any) => ({
        type: 'course' as const,
        id: grade.xkkh || `${semester.name}-${grade.kcmc}`,
        title: grade.kcmc,
        subtitle: `教师: ${grade.jsxm || '未知'} | 学分: ${grade.credit ?? grade.xf ?? 0} | 学期: ${formatTermDisplayName(semester.term, semester.name)}`,
        score: grade.cj,
        routeTab: 'scholar' as const,
      })),
    );

    const uniqueCourses = new Map<string, SearchResultItem>();
    for (const item of flattenedCourses) {
      uniqueCourses.set(item.id, item);
    }
    coursesList.value = Array.from(uniqueCourses.values());

    examsList.value = (scholar.exams || []).map((exam: any) => ({
      type: 'exam' as const,
      id: exam.xkkh || exam.kcmc,
      title: `${exam.kcmc} 考试`,
      subtitle: `时间: ${exam.kssj || exam.qzkssj || exam.time?.[0] || '未知'} | 地点: ${exam.ksdd || exam.qzksdd || exam.location?.[0] || '未知'}`,
      routeTab: 'scholar' as const,
    }));

    todosList.value = (todosData.todo_list || []).map((todo: any) => ({
      type: 'todo' as const,
      id: todo.id || todo.title,
      title: todo.title,
      subtitle: `所属课程: ${todo.course_name} | 截止: ${new Date(todo.end_time).toLocaleString('zh-CN', { hour12: false })}`,
      routeTab: 'task' as const,
    }));

    const localMaterials = (materialsData.items || []).map((item) => ({
      type: 'material' as const,
      id: `local-${item.relativePath}`,
      title: item.title,
      subtitle: `本地资料 | ${item.courseName} | ${item.fileName}`,
      routeTab: 'materials' as const,
      relativePath: item.relativePath,
    }));

    const remoteMaterials = (materialsData.remoteItems || [])
      .filter((item) => !item.downloaded)
      .map((item) => ({
        type: 'material' as const,
        id: item.id,
        title: item.title,
        subtitle: `远程资料 | ${item.courseName} | ${item.fileName}`,
        routeTab: 'materials' as const,
        remoteId: item.id,
        relativePath: item.localRelativePath || undefined,
      }));

    materialsList.value = [...localMaterials, ...remoteMaterials];
  } catch (error) {
    console.error('Search data load failed:', error);
  } finally {
    isLoading.value = false;
  }
}

function handleKeydown(event: KeyboardEvent) {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
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

  return [...coursesList.value, ...examsList.value, ...todosList.value, ...materialsList.value]
    .filter((item) => item.title.toLowerCase().includes(query) || item.subtitle.toLowerCase().includes(query))
    .slice(0, 18);
});

function onItemSelect(item: SearchResultItem) {
  window.dispatchEvent(new CustomEvent('celechron:navigate', { detail: { tab: item.routeTab } }));
  if (item.routeTab === 'materials' && (item.relativePath || item.remoteId)) {
    window.dispatchEvent(
      new CustomEvent('celechron:materials-focus', {
        detail: {
          relativePath: item.relativePath,
          remoteId: item.remoteId,
        },
      }),
    );
  }
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
  if (type === 'material') return LibraryBig;
  return Search;
};

const getColor = (type: string) => {
  if (type === 'course') return typeColors.value.course;
  if (type === 'exam') return typeColors.value.exam;
  if (type === 'todo') return typeColors.value.todo;
  if (type === 'material') return typeColors.value.material;
  return typeColors.value.default;
};

const getIconStyle = (type: string) => {
  const color = getColor(type);
  return {
    color,
    background: `color-mix(in srgb, ${color} 14%, transparent)`,
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
          placeholder="搜索课程、考试、待办、资料... (Cmd/Ctrl + K)"
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
          :class="{ selected: idx === selectedIndex }"
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
  position: fixed;
  inset: 0;
  background: color-mix(in srgb, var(--bg-main) 58%, transparent);
  backdrop-filter: blur(8px);
  z-index: 99999;
  display: flex;
  justify-content: center;
  align-items: flex-start;
  padding-top: 12vh;
}

.search-modal {
  width: min(92vw, 720px);
  background: var(--card-bg);
  border: 1px solid var(--card-border);
  border-radius: 20px;
  box-shadow: 0 24px 56px color-mix(in srgb, var(--bg-main) 28%, transparent);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.search-header {
  display: flex;
  align-items: center;
  padding: 16px 20px;
  border-bottom: 1px solid var(--card-border);
  gap: 12px;
}

.search-icon {
  color: var(--text-muted);
}

.search-input {
  flex: 1;
  border: none;
  background: transparent;
  color: var(--text-main);
  font-size: 1rem;
  outline: none;
}

.search-input::placeholder {
  color: var(--text-muted);
}

.esc-kbd {
  border-radius: 10px;
  border: 1px solid var(--panel-border);
  padding: 0.3rem 0.5rem;
  color: var(--text-muted);
  background: var(--panel-bg);
  font-size: 0.78rem;
}

.search-results {
  max-height: min(68vh, 620px);
  overflow-y: auto;
  padding: 0.75rem;
}

.result-item {
  display: flex;
  align-items: center;
  gap: 0.85rem;
  padding: 0.85rem 0.9rem;
  border-radius: 16px;
  cursor: pointer;
  transition: background 0.2s ease, transform 0.2s ease;
}

.result-item:hover,
.result-item.selected {
  background: var(--panel-bg);
}

.item-icon {
  width: 38px;
  height: 38px;
  border-radius: 12px;
  display: grid;
  place-items: center;
  flex-shrink: 0;
}

.item-content {
  min-width: 0;
  flex: 1;
}

.item-title {
  color: var(--text-main);
  font-weight: 600;
}

.item-subtitle {
  color: var(--text-muted);
  font-size: 0.88rem;
  margin-top: 0.2rem;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.item-score {
  color: var(--accent-green);
  font-weight: 700;
  flex-shrink: 0;
}

.no-results {
  color: var(--text-muted);
  text-align: center;
  padding: 2.5rem 1rem;
}

.loader {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  border: 2px solid color-mix(in srgb, var(--accent-blue) 20%, transparent);
  border-top-color: var(--accent-blue);
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

@media (max-width: 640px) {
  .search-overlay {
    padding-top: 8vh;
  }

  .search-header {
    padding: 14px 16px;
  }

  .item-subtitle {
    white-space: normal;
  }
}
</style>
