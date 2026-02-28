<script setup lang="ts">
import { ref, onMounted, provide } from "vue";
import Login from "./components/Login.vue";
import MainLayout from "./components/MainLayout.vue";
import SearchModal from "./components/SearchModal.vue";
import { useTheme } from "./composables/useTheme";

const { applyTheme, currentTheme } = useTheme();

const isLoggedIn = ref(false);
const layoutKey = ref(0); // Force remount MainLayout on account switch

onMounted(() => {
  applyTheme(currentTheme.value);
  if (localStorage.getItem("lastLogin")) {
    isLoggedIn.value = true;
  }
});

function handleLoginSuccess() {
  localStorage.setItem("lastLogin", "true");
  isLoggedIn.value = true;
}

function handleLogout() {
  localStorage.removeItem("lastLogin");
  isLoggedIn.value = false;
}

function handleAccountSwitch() {
  // Force all child views to re-mount and refetch data
  layoutKey.value++;
}

// Provide to all descendants so OptionView can call these
provide('appLogout', handleLogout);
provide('appAccountSwitch', handleAccountSwitch);
</script>

<template>
  <main class="app-container">
    <!-- Global Draggable Titlebar -->
    <div data-tauri-drag-region class="titlebar">
      Celechron
    </div>

    <div class="mesh-background" aria-hidden="true">
      <div class="blob blob-1"></div>
      <div class="blob blob-2"></div>
      <div class="blob blob-3"></div>
      <div class="blob blob-4"></div>
    </div>
    <div class="app-ui-layer">
      <Login v-if="!isLoggedIn" @login-success="handleLoginSuccess" />
      <MainLayout v-else :key="layoutKey" />
    </div>

    <!-- Global Search Modal (Requires Auth) -->
    <SearchModal v-if="isLoggedIn" />
  </main>
</template>

<style scoped>
.app-container {
  height: 100vh;
  width: 100vw;
  position: relative;
  overflow: hidden;
  background-color: var(--bg-main);
}

.titlebar {
  height: 28px;
  width: 100%;
  position: absolute;
  top: 0;
  left: 0;
  z-index: 9999;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.8rem;
  font-weight: 600;
  color: var(--text-muted);
  user-select: none;
}

.titlebar:hover {
  cursor: default;
}

.mesh-background {
  position: absolute;
  inset: 0;
  z-index: 0;
  pointer-events: none;
}

.blob {
  position: absolute;
  border-radius: 50%;
  filter: blur(80px);
  animation: float 20s infinite ease-in-out alternate;
}

.blob-1 {
  width: 60vw; height: 60vw;
  background: radial-gradient(circle, var(--blob-1) 0%, transparent 70%);
  top: -20vh; left: -10vw;
  opacity: 0.7;
  animation-delay: 0s;
}
.blob-2 {
  width: 50vw; height: 50vw;
  background: radial-gradient(circle, var(--blob-2) 0%, transparent 70%);
  bottom: -20vh; right: -10vw;
  opacity: 0.7;
  animation-delay: -5s;
}
.blob-3 {
  width: 55vw; height: 55vw;
  background: radial-gradient(circle, var(--blob-3) 0%, transparent 70%);
  top: 30vh; left: 40vw;
  opacity: 0.6;
  animation-delay: -10s;
  animation-duration: 25s;
}
.blob-4 {
  width: 45vw; height: 45vw;
  background: radial-gradient(circle, var(--blob-4) 0%, transparent 70%);
  top: 10vh; right: 20vw;
  opacity: 0.5;
  animation-delay: -15s;
  animation-duration: 22s;
}

.app-ui-layer {
  position: absolute;
  inset: 0;
  z-index: 10;
  display: flex;
  flex-direction: column;
}

@keyframes float {
  0%   { transform: translate(0, 0) scale(1); }
  33%  { transform: translate(4vw, 4vh) scale(1.08); }
  66%  { transform: translate(-4vw, 6vh) scale(0.94); }
  100% { transform: translate(-2vw, -3vh) scale(1.04); }
}
</style>