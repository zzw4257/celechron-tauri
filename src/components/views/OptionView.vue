<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { LogOut, RefreshCw, Palette, SunMoon, Layers } from "lucide-vue-next";
import { useTheme, type ThemeType } from "../../composables/useTheme";
import { useAccounts, type SavedAccount } from "../../composables/useAccounts";
import { useBiometric } from "../../composables/useBiometric";

const { currentTheme, THEMES, setTheme, isLightMode, toggleLightMode, glassEffect, setGlassEffect } = useTheme();
const { accounts, removeAccount, updateNickname, getPassword, accountDisplayName } = useAccounts();
const { authenticate } = useBiometric();

const isRefreshing = ref(false);
const isSwitching = ref(false);
const switchStatus = ref("");

async function handleRefresh() {
  if (isRefreshing.value) return;
  isRefreshing.value = true;
  try {
    await invoke("fetch_scholar_data");
    await invoke("fetch_todos");
    await new Promise(r => setTimeout(r, 800));
  } catch (e) {
    console.error(e);
  } finally {
    isRefreshing.value = false;
  }
}

function handleLogout() {
  localStorage.removeItem("lastLogin");
  window.location.reload();
}

async function switchAccount(acc: SavedAccount) {
  if (isSwitching.value) return;
  
  const displayName = accountDisplayName(acc);
  switchStatus.value = `等待验证...`;
  
  const authOk = await authenticate(displayName);
  if (!authOk) {
    switchStatus.value = "系统生物验证取消或失败";
    setTimeout(() => { switchStatus.value = ""; }, 3000);
    return;
  }

  isSwitching.value = true;
  switchStatus.value = `正在切换至 ${displayName}...`;
  
  try {
    const plainPwd = await getPassword(acc);
    await invoke("login_zju_command", { username: acc.username, password: plainPwd });
    switchStatus.value = "切换成功，正在重新加载...";
    // Reload to refresh all data globally with the new cookies
    setTimeout(() => { window.location.reload(); }, 500);
  } catch (err: any) {
    switchStatus.value = typeof err === "string" ? err : (err.message || "切换失败");
    isSwitching.value = false;
  }
}

function promptEditNickname(acc: SavedAccount) {
  const newName = window.prompt("请输入新备注名 (留空可清除备注)", acc.nickname);
  if (newName !== null) {
    updateNickname(acc.id, newName);
  }
}

function deleteAccount(id: string) {
  if (window.confirm("确定要删除此保存的账户吗？\n删除后只能通过密码重新登录。")) {
    removeAccount(id);
  }
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
        <h3 class="group-title">账户管理</h3>
        <div class="settings-card">
          <!-- Current & Saved Accounts List -->
          <div class="account-mgmt-list">
            <div 
              v-for="acc in accounts" 
              :key="acc.id"
              class="account-row"
            >
              <div class="acc-info">
                <span class="acc-avatar">{{ (acc.nickname || acc.username).charAt(0).toUpperCase() }}</span>
                <div class="acc-details">
                  <span class="acc-title">{{ accountDisplayName(acc) }}</span>
                  <span class="acc-sub">{{ acc.username }}</span>
                </div>
              </div>
              <div class="acc-actions">
                <button class="btn-text btn-switch" @click.stop="switchAccount(acc)" :disabled="isSwitching">切换</button>
                <button class="btn-text btn-edit" @click.stop="promptEditNickname(acc)">备注</button>
                <button class="btn-text btn-delete" @click.stop="deleteAccount(acc.id)">删除</button>
              </div>
            </div>
            
            <div v-if="accounts.length === 0" class="no-accounts">
              没有保存的快速账户。<br/>下次登录成功后系统会提示保存。
            </div>
          </div>

          <!-- Status Indicator for Switching -->
          <div v-if="switchStatus" class="switch-status" :class="{ error: switchStatus.includes('失败') || switchStatus.includes('取消') }">
            {{ switchStatus }}
          </div>

          <div class="setting-item danger" @click="handleLogout" style="border-top: 1px solid rgba(0,0,0,0.05); margin-top: 0.5rem; padding-top: 1.2rem;">
            <div class="setting-info">
              <LogOut class="setting-icon" />
              <div class="setting-text">
                <span class="setting-name">退出当前登录</span>
                <span class="setting-desc">清除当期会话并返回登录页，不影响已保存的快速账户</span>
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

/* ─── Account Management ─── */
.account-mgmt-list {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.account-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 12px 14px;
  background: #f8fafc;
  border: 1px solid #e2e8f0;
  border-radius: 14px;
  transition: background 0.2s;
}
:global(.dark-theme) .account-row {
  background: #0f172a;
  border-color: #334155;
}
.acc-info {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}
.acc-avatar {
  width: 38px;
  height: 38px;
  border-radius: 50%;
  background: linear-gradient(135deg, #0ea5e9, #0284c7);
  color: white;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 1.1rem;
  font-weight: 700;
  flex-shrink: 0;
}
.acc-details {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
}
.acc-title {
  font-size: 0.95rem;
  font-weight: 600;
  color: #1e293b;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
:global(.dark-theme) .acc-title { color: #f1f5f9; }
.acc-sub {
  font-size: 0.8rem;
  color: #64748b;
}
:global(.dark-theme) .acc-sub { color: #94a3b8; }
.acc-actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}
.btn-text {
  background: transparent;
  border: none;
  font-size: 0.82rem;
  font-weight: 600;
  cursor: pointer;
  padding: 4px 8px;
  border-radius: 6px;
  transition: all 0.2s;
}
.btn-switch { color: #0ea5e9; }
.btn-switch:hover { background: rgba(14,165,233,0.1); }
.btn-switch:disabled { opacity: 0.5; cursor: not-allowed; }
.btn-edit { color: #64748b; }
.btn-edit:hover { background: rgba(100,116,139,0.1); }
:global(.dark-theme) .btn-edit { color: #94a3b8; }
.btn-delete { color: #dc2626; }
.btn-delete:hover { background: rgba(220,38,38,0.1); }
:global(.dark-theme) .btn-delete { color: #f87171; }
.no-accounts {
  text-align: center;
  padding: 24px;
  color: #64748b;
  font-size: 0.9rem;
  line-height: 1.5;
  background: #f8fafc;
  border-radius: 12px;
  border: 1px dashed #cbd5e1;
}
:global(.dark-theme) .no-accounts {
  background: #0f172a;
  border-color: #334155;
  color: #94a3b8;
}
.switch-status {
  margin-top: 12px;
  padding: 8px;
  border-radius: 8px;
  background: #f0f9ff;
  color: #0284c7;
  font-size: 0.85rem;
  text-align: center;
  font-weight: 500;
}
:global(.dark-theme) .switch-status { background: #0c4a6e; color: #38bdf8; }
.switch-status.error { background: #fef2f2; color: #dc2626; }
:global(.dark-theme) .switch-status.error { background: #450a0a; color: #f87171; }
</style>
