<script setup lang="ts">
import { ref, inject } from "vue";
import { Clock, CalendarDays, CheckSquare, GraduationCap, Settings, LogOut, Search } from "lucide-vue-next";

import FlowView from "./views/FlowView.vue";
import CalendarView from "./views/CalendarView.vue";
import TaskView from "./views/TaskView.vue";
import ScholarView from "./views/ScholarView.vue";
import OptionView from "./views/OptionView.vue";

const activeTab = ref("scholar"); // default to Scholar as requested

const tabs = [
  { id: "flow", label: "接下来", icon: Clock, component: FlowView },
  { id: "calendar", label: "日程", icon: CalendarDays, component: CalendarView },
  { id: "task", label: "任务", icon: CheckSquare, component: TaskView },
  { id: "scholar", label: "学业", icon: GraduationCap, component: ScholarView },
  { id: "option", label: "设置", icon: Settings, component: OptionView },
];

const logout = inject<() => void>('appLogout', () => { window.location.reload(); });

const triggerGlobalSearch = () => {
  if (typeof (window as any).__toggleGlobalSearch === 'function') {
    (window as any).__toggleGlobalSearch();
  }
};
</script>

<template>
  <div class="main-layout">
    <div class="layout-content">
      <!-- Dynamic View Rendering -->
      <KeepAlive>
        <component 
          :is="tabs.find(t => t.id === activeTab)?.component" 
          class="view-container fade-enter-active" 
          :key="activeTab"
        />
      </KeepAlive>
    </div>

    <!-- Bottom/Sidebar Navigation -->
    <nav class="bottom-nav glass-panel">
      <div class="nav-items-container">
        <button 
          v-for="tab in tabs" 
          :key="tab.id"
          class="nav-item"
          :class="{ active: activeTab === tab.id }"
          @click="activeTab = tab.id"
        >
          <component :is="tab.icon" class="nav-icon" :size="24" :stroke-width="activeTab === tab.id ? 2.5 : 2" />
          <span class="nav-label">{{ tab.label }}</span>
        </button>

        <div class="nav-item logout-btn" @click="triggerGlobalSearch" title="全局搜索 (Cmd+K)">
           <Search class="nav-icon" :size="24" />
           <span class="nav-label">搜索</span>
        </div>

        <div class="nav-item logout-btn" @click="logout" title="Log out">
           <LogOut class="nav-icon" :size="24" />
           <span class="nav-label">退出</span>
        </div>
      </div>
    </nav>
  </div>
</template>

<style scoped>
.main-layout {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
  color: var(--text-main);
  position: relative;
  z-index: 10;
}

.layout-content {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  padding-bottom: 80px; /* Space for bottom nav */
}

.view-container {
  min-height: 100%;
}

/* Glassmorphic Nav Bar */
.bottom-nav {
  position: fixed;
  bottom: 0;
  left: 0;
  width: 100%;
  height: 80px;
  background: var(--nav-bg, rgba(15, 23, 42, 0.4));
  backdrop-filter: blur(40px) saturate(150%);
  -webkit-backdrop-filter: blur(40px) saturate(150%);
  border-top: 1px solid var(--nav-border, rgba(255, 255, 255, 0.08));
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 100;
  box-shadow: 0 -4px 24px var(--nav-shadow, rgba(0, 0, 0, 0.1));
}

.nav-items-container {
  display: flex;
  width: 100%;
  max-width: 600px;
  justify-content: space-around;
  align-items: center;
  padding: 0 1rem;
}

.nav-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 4px;
  background: transparent;
  border: none;
  color: var(--nav-text, #94a3b8);
  cursor: pointer;
  padding: 8px 12px;
  border-radius: 12px;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.nav-item:hover {
  color: var(--nav-text-hover, #e2e8f0);
  background: var(--nav-hover-bg, rgba(255, 255, 255, 0.1));
}

.nav-item.active {
  color: var(--accent-blue, #38bdf8);
}

.nav-item.active .nav-icon {
  transform: translateY(-2px);
  filter: drop-shadow(0 0 8px var(--nav-icon-shadow, rgba(56, 189, 248, 0.5)));
}

.nav-label {
  font-size: 0.75rem;
  font-weight: 500;
  letter-spacing: 0.5px;
}

.logout-btn:hover {
  color: #ef4444;
}

/* Base animations */
.fade-enter-active {
  animation: fadeIn 0.4s ease-out forwards;
}

@keyframes fadeIn {
  from { opacity: 0; transform: translateY(10px); }
  to { opacity: 1; transform: translateY(0); }
}

/* Media query for Sidebar on Desktop */
@media (min-width: 768px) {
  .main-layout {
    flex-direction: row;
  }
  
  .layout-content {
    padding-bottom: 0;
    padding-left: 100px; /* Space for sidebar */
  }

  .bottom-nav {
    top: 0;
    left: 0;
    width: 100px;
    height: 100%;
    border-top: none;
    border-right: 1px solid var(--nav-border, rgba(255, 255, 255, 0.08));
    box-shadow: 4px 0 24px var(--nav-shadow, rgba(0, 0, 0, 0.1));
  }

  .nav-items-container {
    flex-direction: column;
    justify-content: flex-start;
    padding: 2rem 0;
    gap: 1.5rem;
    height: 100%;
  }

  .nav-item {
    width: 80%;
  }

  .logout-btn {
    margin-top: auto; /* Push to bottom */
  }
}
</style>
