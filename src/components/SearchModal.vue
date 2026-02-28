<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue';
import { invoke } from "@tauri-apps/api/core";
import { Search, Calendar, BookOpen, AlertCircle } from 'lucide-vue-next';

const isVisible = ref(false);
const searchQuery = ref('');
const selectedIndex = ref(-1);

const searchInput = ref<HTMLInputElement | null>(null);

// Raw Data
const coursesList = ref<any[]>([]);
const examsList = ref<any[]>([]);
const todosList = ref<any[]>([]);
const isLoading = ref(false);

async function loadData() {
  if (coursesList.value.length > 0) return; // already loaded
  
  isLoading.value = true;
  try {
    const data: any = await invoke("fetch_scholar_data");
    const todosData: any = await invoke("fetch_todos");

    // 1. Flatten all semesters' courses
    const allC: any[] = [];
    (data.semesters || []).forEach((sem: any) => {
      sem.grades.forEach((g: any) => {
        allC.push({
          type: 'course',
          id: g.xkkh,
          title: g.kcmc,
          subtitle: `教师: ${g.jsxm || '未知'} | 学分: ${g.xf} | 学期: ${sem.name}`,
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
  if (type === 'course') return '#3b82f6'; // blue
  if (type === 'exam') return '#f59e0b'; // orange
  if (type === 'todo') return '#10b981'; // green
  return '#94a3b8';
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
          <div class="item-icon" :style="{ color: getColor(item.type), background: getColor(item.type) + '22' }">
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
  background: rgba(0, 0, 0, 0.4);
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
  background: rgba(30, 41, 59, 0.9);
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 16px;
  box-shadow: 0 20px 40px rgba(0, 0, 0, 0.3);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.search-header {
  display: flex;
  align-items: center;
  padding: 16px 20px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  gap: 12px;
}

.search-icon {
  color: #94a3b8;
}

.search-input {
  flex: 1;
  background: transparent;
  border: none;
  font-size: 1.1rem;
  color: #f8fafc;
  outline: none;
}
.search-input::placeholder {
  color: #64748b;
}

.esc-kbd {
  font-size: 0.7rem;
  color: #94a3b8;
  background: rgba(255, 255, 255, 0.1);
  padding: 2px 6px;
  border-radius: 4px;
  border: 1px solid rgba(255, 255, 255, 0.1);
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
  color: #94a3b8;
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
  background: rgba(255, 255, 255, 0.08);
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
  color: #f8fafc;
  font-weight: 500;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.item-subtitle {
  font-size: 0.8rem;
  color: #94a3b8;
  margin-top: 2px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.item-score {
  font-family: 'JetBrains Mono', monospace;
  font-weight: 700;
  font-size: 1.1rem;
  color: #22c55e;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}
.loader {
  width: 16px; height: 16px;
  border: 2px solid transparent;
  border-top-color: #38bdf8;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

/* Light Mode Overrides */
:global(.light-theme) .search-modal {
  background: rgba(255, 255, 255, 0.95);
  border-color: rgba(0, 0, 0, 0.1);
}
:global(.light-theme) .search-header {
  border-bottom-color: rgba(0, 0, 0, 0.08);
}
:global(.light-theme) .search-input { color: #1e293b; }
:global(.light-theme) .search-input::placeholder { color: #94a3b8; }
:global(.light-theme) .item-title { color: #334155; }
:global(.light-theme) .item-subtitle { color: #64748b; }
:global(.light-theme) .no-results { color: #64748b; }
:global(.light-theme) .result-item.selected { background: rgba(0, 0, 0, 0.05); }
:global(.light-theme) .esc-kbd { 
  background: #f1f5f9; 
  color: #64748b; 
  border-color: #e2e8f0; 
}
</style>
