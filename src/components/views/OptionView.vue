<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { LogOut, RefreshCw, Palette, SunMoon, Layers } from "lucide-vue-next";
import { useTheme, type ThemeType } from "../../composables/useTheme";

const { currentTheme, THEMES, setTheme, isLightMode, toggleLightMode, glassEffect, setGlassEffect } = useTheme();

const isRefreshing = ref(false);

async function handleRefresh() {
  if (isRefreshing.value) return;
  isRefreshing.value = true;
  try {
    // We could technically just trigger all network calls again or clear cache
    // A simple refresh strategy is to refetch Scholar. Because caching logic wraps failures, 
    // real forcing might require a dedicated backend command if cache overrides are disabled,
    // but right now it fetches network first anyway.
    await invoke("fetch_scholar_data");
    await invoke("fetch_todos");
    // Show a success toast or just spin for 1s
    await new Promise(r => setTimeout(r, 800));
  } catch (e) {
    console.error(e);
  } finally {
    isRefreshing.value = false;
  }
}

function handleLogout() {
  // Clear any stored flags
  localStorage.removeItem("lastLogin");
  // Hard reload the window to reset all vue state and drop to Login screen
  window.location.reload();
}
</script>

<template>
  <div class="option-view">
    <header class="option-header">
      <h1>设置</h1>
    </header>

    <div class="settings-container">
      <!-- Theme Selection -->
      <section class="settings-group">
        <h3 class="group-title">外观与主题</h3>
        
        <div class="settings-card">
          <!-- Light / Dark Mode -->
          <div class="setting-item" style="cursor: default;">
            <div class="setting-info" style="align-items: center; justify-content: space-between; width: 100%;">
              <div style="display: flex; gap: 1rem; align-items: center;">
                <SunMoon class="setting-icon" />
                <div class="setting-text">
                  <span class="setting-name">显示模式</span>
                  <span class="setting-desc">切换深色或浅色界面</span>
                </div>
              </div>
              <div class="toggle-switch" @click="toggleLightMode(!isLightMode)" :class="{ active: isLightMode }">
                <div class="toggle-knob"></div>
              </div>
            </div>
          </div>

          <!-- Glass Effect Level -->
          <div class="setting-item" style="cursor: default;">
            <div class="setting-info" style="align-items: center; justify-content: space-between; width: 100%;">
              <div style="display: flex; gap: 1rem; align-items: center;">
                <Layers class="setting-icon" />
                <div class="setting-text">
                  <span class="setting-name">玻璃视效 (渲染引击)</span>
                  <span class="setting-desc">LiquidGlass 提供动态流体壁纸，如机器卡顿发热可切换静态毛玻璃</span>
                </div>
              </div>
              <div class="btn-group">
                <button 
                  class="btn-segment" 
                  :class="{ active: glassEffect === 'liquid' }" 
                  @click="setGlassEffect('liquid')"
                >WebGL 流体视效</button>
                <button 
                  class="btn-segment" 
                  :class="{ active: glassEffect === 'frosted' }" 
                  @click="setGlassEffect('frosted')"
                >CSS 静态玻璃</button>
              </div>
            </div>
          </div>

          <!-- Color Palette -->
          <div class="setting-item" style="cursor: default; border-bottom: none;">
            <div class="setting-info" style="align-items: flex-start;">
              <Palette class="setting-icon" />
              <div class="setting-text" style="width: 100%;">
                <span class="setting-name">主题色盘 (Color Palette)</span>
                <span class="setting-desc">应用全局液态玻璃渲染风格</span>
                
                <div class="theme-grid">
                  <div 
                    v-for="(themeData, key) in THEMES" 
                    :key="key"
                    class="theme-swatch"
                    :class="{ active: currentTheme === key }"
                    @click="setTheme(key as ThemeType)"
                  >
                    <div class="swatch-colors">
                      <div class="swatch-color" :style="{ background: themeData.darkBlobs[0] }"></div>
                      <div class="swatch-color" :style="{ background: themeData.darkBlobs[1] }"></div>
                      <div class="swatch-color" :style="{ background: themeData.darkBlobs[2] }"></div>
                    </div>
                    <span class="swatch-name">{{ themeData.name }}</span>
                  </div>
                </div>

              </div>
            </div>
          </div>
        </div>
      </section>

      <!-- Data Management -->
      <section class="settings-group">
        <h3 class="group-title">数据管理</h3>
        <div class="settings-card">
          <div class="setting-item" @click="handleRefresh">
            <div class="setting-info">
              <RefreshCw class="setting-icon" :class="{ 'spinning': isRefreshing }" />
              <div class="setting-text">
                <span class="setting-name">强制同步数据</span>
                <span class="setting-desc">立即从学在浙大与教务网拉取最新数据</span>
              </div>
            </div>
          </div>
        </div>
      </section>

      <!-- Account Settings -->
      <section class="settings-group">
        <h3 class="group-title">账户</h3>
        <div class="settings-card">
          <div class="setting-item danger" @click="handleLogout">
            <div class="setting-info">
              <LogOut class="setting-icon" />
              <div class="setting-text">
                <span class="setting-name">退出登录</span>
                <span class="setting-desc">清除本地缓存并返回登录页</span>
              </div>
            </div>
          </div>
        </div>
      </section>
      
      <!-- About -->
      <section class="settings-group">
        <div class="about-card">
          <LayoutTemplate class="app-icon" />
          <h2>Celechron</h2>
          <p>Version 0.2.0-beta</p>
          <span class="footer-note">Designed with Liquid Glass</span>
        </div>
      </section>

    </div>
  </div>
</template>

<style scoped>
.option-view {
  padding: 2rem 2.5rem 6rem;
  max-width: 950px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.option-header h1 {
  font-size: 1.8rem;
  font-weight: 700;
  margin: 0;
  background: linear-gradient(135deg, #e2e8f0, #f8fafc);
  background-clip: text;
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}

.settings-container {
  display: flex;
  flex-direction: column;
  gap: 2rem;
  margin-top: 1rem;
}

.settings-group {
  display: flex;
  flex-direction: column;
  gap: 0.8rem;
}

.group-title {
  font-size: 0.9rem;
  font-weight: 700;
  color: #94a3b8;
  margin: 0;
  padding-left: 10px;
  letter-spacing: 0.5px;
}

.settings-card {
  background: rgba(255,255,255,0.03);
  border: 1px solid rgba(255,255,255,0.06);
  border-radius: 20px;
  overflow: hidden;
  backdrop-filter: blur(12px);
}

.setting-item {
  padding: 1.2rem 1.5rem;
  display: flex;
  align-items: center;
  justify-content: space-between;
  cursor: pointer;
  transition: background 0.2s;
}

.setting-item:hover {
  background: rgba(255,255,255,0.05);
}

.setting-info {
  display: flex;
  align-items: center;
  gap: 1rem;
}

.setting-icon {
  width: 24px;
  height: 24px;
  color: #38bdf8;
}

.setting-item.danger .setting-icon {
  color: #ef4444;
}

.setting-item.danger:hover {
  background: rgba(239, 68, 68, 0.1);
}

.setting-text {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.setting-name {
  font-size: 1.05rem;
  font-weight: 600;
  color: #f8fafc;
}

.setting-item.danger .setting-name {
  color: #fca5a5;
}

.setting-desc {
  font-size: 0.8rem;
  color: #94a3b8;
}

.spinning {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

.about-card {
  margin-top: 2rem;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  color: #64748b;
}

/* UI Controls */
.toggle-switch {
  width: 50px;
  height: 28px;
  background: rgba(255, 255, 255, 0.2);
  border-radius: 14px;
  position: relative;
  cursor: pointer;
  transition: background 0.3s;
  flex-shrink: 0;
}
.toggle-switch.active {
  background: #38bdf8;
}
.toggle-knob {
  width: 24px;
  height: 24px;
  background: #fff;
  border-radius: 50%;
  position: absolute;
  top: 2px;
  left: 2px;
  transition: transform 0.3s cubic-bezier(0.4, 0.0, 0.2, 1);
  box-shadow: 0 2px 4px rgba(0,0,0,0.2);
}
.toggle-switch.active .toggle-knob {
  transform: translateX(22px);
}

.btn-group {
  display: flex;
  flex-wrap: wrap;
  background: rgba(255, 255, 255, 0.05);
  border-radius: 12px;
  padding: 4px;
  gap: 4px;
}
.btn-segment {
  flex: 1;
  min-width: fit-content;
  background: transparent;
  border: none;
  color: #94a3b8;
  padding: 6px 10px;
  border-radius: 8px;
  font-size: 0.8rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
  white-space: nowrap;
}
.btn-segment:hover {
  color: #f8fafc;
}
.btn-segment.active {
  background: rgba(56, 189, 248, 0.2);
  color: #38bdf8;
}

:root.light-theme .btn-segment.active {
  background: #fff;
  color: #0284c7;
  box-shadow: 0 2px 8px rgba(0,0,0,0.05);
}
:root.light-theme .btn-group {
  background: rgba(0,0,0,0.05);
}
:root.light-theme .toggle-switch {
  background: rgba(0,0,0,0.15);
}

/* ── Comprehensive Light Mode for OptionView ── */
:root.light-theme .option-view {
  color: #1e293b;
}
:root.light-theme .option-header h1 {
  background: linear-gradient(135deg, #1e293b, #334155);
  background-clip: text;
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}
:root.light-theme .group-title {
  color: #334155;
}
:root.light-theme .settings-card {
  background: rgba(255,255,255,0.7);
  border-color: rgba(0,0,0,0.08);
}
:root.light-theme .setting-name {
  color: #1e293b;
}
:root.light-theme .setting-desc {
  color: #64748b;
}
:root.light-theme .setting-icon {
  color: #0284c7;
}
:root.light-theme .swatch-name {
  color: #334155;
}
:root.light-theme .theme-swatch {
  background: rgba(0,0,0,0.04);
  border-color: rgba(0,0,0,0.06);
}
:root.light-theme .theme-swatch:hover {
  background: rgba(0,0,0,0.06);
}
:root.light-theme .theme-swatch.active {
  background: rgba(2,132,199,0.08);
  border-color: #0284c7;
}
:root.light-theme .btn-segment {
  color: #64748b;
}
:root.light-theme .btn-segment:hover {
  color: #1e293b;
}

.theme-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: 12px;
  margin-top: 16px;
}
.theme-swatch {
  background: rgba(0,0,0,0.2);
  border: 2px solid transparent;
  border-radius: 12px;
  padding: 12px;
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 10px;
  transition: all 0.2s;
}
.theme-swatch:hover {
  background: rgba(255,255,255,0.05);
}
.theme-swatch.active {
  background: rgba(56, 189, 248, 0.1);
  border-color: #38bdf8;
}
.swatch-colors {
  display: flex;
  height: 24px;
  border-radius: 6px;
  overflow: hidden;
}
.swatch-color {
  flex: 1;
}
.swatch-name {
  font-size: 0.85rem;
  color: #e2e8f0;
  font-weight: 600;
  text-align: center;
}

@media (max-width: 600px) {
  .option-view { padding: 1rem 1rem 6rem; }
}
</style>
