<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
import { CalendarDays, CheckSquare, Clock, GraduationCap, LibraryBig, Search, Settings } from 'lucide-vue-next';

import CalendarView from './views/CalendarView.vue';
import FlowView from './views/FlowView.vue';
import MaterialsView from './views/MaterialsView.vue';
import OptionView from './views/OptionView.vue';
import ScholarView from './views/ScholarView.vue';
import TaskView from './views/TaskView.vue';

const activeTab = ref('scholar');

const tabs = [
  { id: 'flow', label: '接下来', icon: Clock, component: FlowView },
  { id: 'calendar', label: '日程', icon: CalendarDays, component: CalendarView },
  { id: 'task', label: '任务', icon: CheckSquare, component: TaskView },
  { id: 'scholar', label: '学业', icon: GraduationCap, component: ScholarView },
  { id: 'materials', label: '资料', icon: LibraryBig, component: MaterialsView },
  { id: 'option', label: '设置', icon: Settings, component: OptionView },
];

function triggerGlobalSearch() {
  if (typeof (window as any).__toggleGlobalSearch === 'function') {
    (window as any).__toggleGlobalSearch();
  }
}

function handleNavigate(event: Event) {
  const detail = (event as CustomEvent<{ tab?: string }>).detail;
  if (!detail?.tab) return;
  if (tabs.some((tab) => tab.id === detail.tab)) {
    activeTab.value = detail.tab;
  }
}

onMounted(() => {
  window.addEventListener('celechron:navigate', handleNavigate as EventListener);
});

onUnmounted(() => {
  window.removeEventListener('celechron:navigate', handleNavigate as EventListener);
});
</script>

<template>
  <div class="main-layout">
    <div class="layout-content">
      <KeepAlive>
        <component :is="tabs.find((tab) => tab.id === activeTab)?.component" :key="activeTab" class="view-container" />
      </KeepAlive>
    </div>

    <nav class="bottom-nav" aria-label="主导航">
      <div class="nav-items-container">
        <button
          v-for="tab in tabs"
          :key="tab.id"
          type="button"
          class="nav-item"
          :class="{ active: activeTab === tab.id }"
          @click="activeTab = tab.id"
        >
          <component :is="tab.icon" class="nav-icon" :size="22" :stroke-width="activeTab === tab.id ? 2.4 : 2" />
          <span class="nav-label">{{ tab.label }}</span>
        </button>

        <button type="button" class="nav-item nav-search" title="全局搜索 (Cmd+K)" @click="triggerGlobalSearch">
          <Search class="nav-icon" :size="22" :stroke-width="2" />
          <span class="nav-label">搜索</span>
        </button>
      </div>
    </nav>
  </div>
</template>

<style scoped>
.main-layout {
  height: 100%;
  width: 100%;
  position: relative;
  color: var(--text-primary);
}

.layout-content {
  height: 100%;
  overflow-y: auto;
  overflow-x: hidden;
}

.view-container {
  min-height: 100%;
}

.bottom-nav {
  position: fixed;
  inset: auto 0 0;
  z-index: 25;
  min-height: var(--nav-height);
  padding: 0.75rem max(1rem, var(--safe-left)) calc(0.75rem + var(--safe-bottom)) max(1rem, var(--safe-right));
  background: color-mix(in srgb, var(--surface-1) 92%, transparent);
  border-top: 1px solid var(--nav-border);
  backdrop-filter: blur(24px) saturate(135%);
  box-shadow: var(--nav-shadow);
}

.nav-items-container {
  width: min(100%, 960px);
  margin: 0 auto;
  display: grid;
  grid-template-columns: repeat(7, minmax(0, 1fr));
  gap: 0.45rem;
}

.nav-item {
  border: 1px solid transparent;
  background: transparent;
  color: var(--nav-text);
  min-height: 3.2rem;
  border-radius: 1.15rem;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.28rem;
  cursor: pointer;
  transition: background 160ms ease, border-color 160ms ease, color 160ms ease, transform 160ms ease;
}

.nav-item:hover {
  background: var(--nav-hover-bg);
  color: var(--nav-text-hover);
}

.nav-item.active {
  color: var(--accent-text);
  background: var(--accent-soft);
  border-color: var(--accent-border);
  transform: translateY(-1px);
}

.nav-icon {
  flex-shrink: 0;
}

.nav-label {
  font-size: 0.75rem;
  font-weight: 600;
}

@media (max-width: 720px) {
  .nav-items-container {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }
}

@media (min-width: 900px) {
  .bottom-nav {
    inset: 0 auto 0 0;
    width: var(--desktop-sidebar-width);
    min-height: 100vh;
    padding: calc(1rem + var(--safe-top)) 0.8rem calc(1rem + var(--safe-bottom));
    border-top: none;
    border-right: 1px solid var(--nav-border);
  }

  .nav-items-container {
    height: 100%;
    grid-template-columns: 1fr;
    width: 100%;
    align-content: start;
  }

  .nav-search {
    margin-top: auto;
  }

  .layout-content {
    padding-left: calc(var(--desktop-sidebar-width) + 0.35rem);
  }
}
</style>
