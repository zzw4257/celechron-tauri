<script setup lang="ts">
import { computed, onMounted, provide, ref } from 'vue';
import Login from './components/Login.vue';
import MainLayout from './components/MainLayout.vue';
import SearchModal from './components/SearchModal.vue';
import { useTheme } from './composables/useTheme';
import { usePreferences } from './composables/usePreferences';

const { applyTheme, currentTheme } = useTheme();
const { accountScope, bumpAccountScope } = usePreferences();

const isLoggedIn = ref(false);
const layoutKey = ref(0);
const isMacDesktop = computed(() => {
  if (typeof navigator === 'undefined') return false;
  const ua = navigator.userAgent || '';
  return /Macintosh|Mac OS X/i.test(ua) && !/iPhone|iPad|Android/i.test(ua);
});

onMounted(() => {
  applyTheme(currentTheme.value);
  if (localStorage.getItem('lastLogin')) {
    isLoggedIn.value = true;
  }
});

function refreshSessionScope() {
  bumpAccountScope();
  layoutKey.value += 1;
}

function handleLoginSuccess() {
  localStorage.setItem('lastLogin', 'true');
  isLoggedIn.value = true;
  refreshSessionScope();
}

function handleLogout() {
  localStorage.removeItem('lastLogin');
  localStorage.removeItem('celechron_active_username');
  isLoggedIn.value = false;
  refreshSessionScope();
}

function handleAccountSwitch() {
  refreshSessionScope();
}

provide('appLogout', handleLogout);
provide('appAccountSwitch', handleAccountSwitch);
</script>

<template>
  <main class="app-shell" :class="{ 'has-titlebar': isMacDesktop }">
    <div v-if="isMacDesktop" data-tauri-drag-region class="titlebar">
      <span class="titlebar__label">Celechron</span>
    </div>

    <div class="mesh-background" aria-hidden="true">
      <div class="blob blob-1"></div>
      <div class="blob blob-2"></div>
      <div class="blob blob-3"></div>
      <div class="blob blob-4"></div>
    </div>

    <div class="app-ui-layer" :style="{ '--titlebar-height': isMacDesktop ? '30px' : '0px' }">
      <Login v-if="!isLoggedIn" @login-success="handleLoginSuccess" />
      <MainLayout v-else :key="layoutKey" />
    </div>

    <SearchModal v-if="isLoggedIn" :key="`search-${accountScope}`" />
  </main>
</template>

<style scoped>
.app-shell {
  height: 100vh;
  width: 100vw;
  position: relative;
  overflow: hidden;
  background: var(--bg-main);
}

.titlebar {
  position: absolute;
  inset: 0 0 auto;
  height: 30px;
  z-index: 30;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-secondary);
  user-select: none;
}

.titlebar__label {
  font-size: 0.8rem;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-transform: uppercase;
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
  filter: blur(88px);
  animation: float 22s infinite ease-in-out alternate;
}

.blob-1 {
  width: 56vw;
  height: 56vw;
  background: radial-gradient(circle, var(--blob-1) 0%, transparent 68%);
  left: -10vw;
  top: -18vh;
}

.blob-2 {
  width: 44vw;
  height: 44vw;
  background: radial-gradient(circle, var(--blob-2) 0%, transparent 68%);
  right: -8vw;
  bottom: -16vh;
  animation-delay: -5s;
}

.blob-3 {
  width: 38vw;
  height: 38vw;
  background: radial-gradient(circle, var(--blob-3) 0%, transparent 70%);
  right: 14vw;
  top: 14vh;
  animation-delay: -10s;
}

.blob-4 {
  width: 34vw;
  height: 34vw;
  background: radial-gradient(circle, var(--blob-4) 0%, transparent 70%);
  left: 24vw;
  bottom: 10vh;
  animation-delay: -14s;
}

.app-ui-layer {
  position: absolute;
  inset: 0;
  z-index: 10;
  padding-top: calc(var(--safe-top) + var(--titlebar-height));
}

@keyframes float {
  0% {
    transform: translate3d(0, 0, 0) scale(1);
  }

  50% {
    transform: translate3d(4vw, 3vh, 0) scale(1.04);
  }

  100% {
    transform: translate3d(-3vw, 5vh, 0) scale(0.94);
  }
}
</style>
