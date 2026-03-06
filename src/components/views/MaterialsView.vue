<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import type { MaterialAsset } from '../../types/api';
import {
  downloadMaterialAsset,
  fetchMaterials,
  openMaterialAsset,
  removeMaterialCache,
} from '../../services/api';

const isLoading = ref(true);
const errorMsg = ref('');
const items = ref<MaterialAsset[]>([]);
const search = ref('');
const showAddForm = ref(false);
const draft = ref({
  courseName: '',
  title: '',
  url: '',
  fileName: '',
});
const actionStatus = ref('');
const isSubmitting = ref(false);

const filteredItems = computed(() => {
  const keyword = search.value.trim().toLowerCase();
  if (!keyword) {
    return items.value;
  }
  return items.value.filter((item) =>
    [item.courseName, item.title, item.fileName]
      .join(' ')
      .toLowerCase()
      .includes(keyword)
  );
});

const groupedItems = computed(() => {
  const groups = new Map<string, MaterialAsset[]>();
  for (const item of filteredItems.value) {
    const key = item.courseName || '未分组课程';
    const bucket = groups.get(key) || [];
    bucket.push(item);
    groups.set(key, bucket);
  }
  return [...groups.entries()].map(([courseName, assets]) => ({
    courseName,
    assets,
  }));
});

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

function formatTime(ts: number) {
  return new Date(ts * 1000).toLocaleString('zh-CN', { hour12: false });
}

async function loadMaterials() {
  isLoading.value = true;
  errorMsg.value = '';
  try {
    const env = await fetchMaterials();
    items.value = env.data.items || [];
  } catch (error: any) {
    errorMsg.value = error?.message || String(error);
  } finally {
    isLoading.value = false;
  }
}

async function handleDownload() {
  if (!draft.value.courseName || !draft.value.title || !draft.value.url || isSubmitting.value) {
    return;
  }
  isSubmitting.value = true;
  actionStatus.value = '正在下载资料...';
  try {
    await downloadMaterialAsset({
      courseName: draft.value.courseName,
      title: draft.value.title,
      url: draft.value.url,
      fileName: draft.value.fileName || undefined,
      source: 'manual',
    });
    draft.value = { courseName: '', title: '', url: '', fileName: '' };
    showAddForm.value = false;
    actionStatus.value = '资料已缓存到本地';
    await loadMaterials();
  } catch (error: any) {
    actionStatus.value = error?.message || String(error);
  } finally {
    isSubmitting.value = false;
  }
}

async function handleOpen(item: MaterialAsset) {
  actionStatus.value = '';
  try {
    await openMaterialAsset(item.relativePath);
  } catch (error: any) {
    actionStatus.value = error?.message || String(error);
  }
}

async function handleRemove(item: MaterialAsset) {
  actionStatus.value = '';
  try {
    await removeMaterialCache(item.relativePath);
    actionStatus.value = '已删除本地缓存';
    await loadMaterials();
  } catch (error: any) {
    actionStatus.value = error?.message || String(error);
  }
}

onMounted(loadMaterials);
</script>

<template>
  <div class="materials-view">
    <header class="materials-header">
      <div>
        <h1>资料</h1>
        <p>统一管理课程课件、讲稿和本地缓存。当前已先接入本地缓存链路，后续继续对接 zju-learning-assistant 的资料源。</p>
      </div>
      <div class="materials-actions">
        <button class="action-btn" @click="loadMaterials">刷新</button>
        <button class="action-btn primary" @click="showAddForm = !showAddForm">
          {{ showAddForm ? '收起' : '添加资料' }}
        </button>
      </div>
    </header>

    <section class="panel search-panel">
      <input v-model="search" class="search-input" placeholder="搜索课程名、资料标题或文件名" />
      <span class="search-meta">共 {{ filteredItems.length }} 份资料</span>
    </section>

    <section v-if="showAddForm" class="panel form-panel">
      <div class="form-grid">
        <input v-model="draft.courseName" class="form-input" placeholder="课程名" />
        <input v-model="draft.title" class="form-input" placeholder="资料标题" />
        <input v-model="draft.fileName" class="form-input" placeholder="文件名（可选）" />
        <input v-model="draft.url" class="form-input form-input-wide" placeholder="可下载 URL" />
      </div>
      <div class="form-actions">
        <span class="hint">支持把 donor 链路解析出的附件 URL 直接缓存到本地目录。</span>
        <button class="action-btn primary" :disabled="isSubmitting" @click="handleDownload">
          {{ isSubmitting ? '下载中...' : '下载并缓存' }}
        </button>
      </div>
    </section>

    <div v-if="actionStatus" class="status-banner">{{ actionStatus }}</div>
    <div v-if="errorMsg" class="status-banner error">{{ errorMsg }}</div>

    <section v-if="isLoading" class="panel empty-panel">
      <p>资料索引加载中...</p>
    </section>

    <section v-else-if="groupedItems.length === 0" class="panel empty-panel">
      <h3>暂无资料</h3>
      <p>当前资料页已经接入本地缓存与打开/删除链路。你可以先手动加入附件 URL，后续再接 donor 侧课程资料源。</p>
    </section>

    <section v-else class="materials-groups">
      <article v-for="group in groupedItems" :key="group.courseName" class="panel course-group">
        <div class="group-header">
          <h2>{{ group.courseName }}</h2>
          <span>{{ group.assets.length }} 份</span>
        </div>
        <div class="asset-list">
          <div v-for="item in group.assets" :key="item.relativePath" class="asset-row">
            <div class="asset-main">
              <h3>{{ item.title }}</h3>
              <p>{{ item.fileName }}</p>
              <div class="asset-meta">
                <span>{{ formatBytes(item.sizeBytes) }}</span>
                <span>{{ formatTime(item.updatedAt) }}</span>
                <span v-if="item.sourceUrl">已记录来源</span>
              </div>
            </div>
            <div class="asset-actions">
              <button class="mini-btn" @click="handleOpen(item)">打开</button>
              <button class="mini-btn danger" @click="handleRemove(item)">删除</button>
            </div>
          </div>
        </div>
      </article>
    </section>
  </div>
</template>

<style scoped>
.materials-view {
  padding: 2rem 2.5rem 6rem;
  max-width: 960px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 1rem;
  color: var(--text-main);
}

.materials-header {
  display: flex;
  justify-content: space-between;
  gap: 1rem;
  align-items: flex-start;
}

.materials-header h1 {
  margin: 0;
  font-size: 1.9rem;
}

.materials-header p {
  margin: 0.5rem 0 0;
  color: var(--text-muted);
  max-width: 680px;
  line-height: 1.6;
}

.materials-actions,
.form-actions,
.asset-actions {
  display: flex;
  gap: 0.75rem;
  align-items: center;
}

.panel {
  background: var(--card-bg);
  border: 1px solid var(--card-border);
  border-radius: 20px;
  padding: 1.1rem 1.25rem;
  backdrop-filter: blur(14px);
}

.search-panel {
  display: flex;
  justify-content: space-between;
  gap: 1rem;
  align-items: center;
}

.search-input,
.form-input {
  border: 1px solid var(--input-border);
  background: var(--input-bg);
  color: var(--input-color);
  border-radius: 14px;
  padding: 0.85rem 1rem;
  font-size: 0.95rem;
}

.search-input {
  flex: 1;
}

.search-meta,
.hint,
.asset-meta,
.asset-main p {
  color: var(--text-muted);
}

.form-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 0.8rem;
}

.form-input-wide {
  grid-column: 1 / -1;
}

.status-banner {
  padding: 0.9rem 1rem;
  border-radius: 16px;
  background: color-mix(in srgb, var(--accent-blue) 12%, transparent);
  color: var(--accent-blue);
  border: 1px solid color-mix(in srgb, var(--accent-blue) 24%, transparent);
}

.status-banner.error {
  background: color-mix(in srgb, var(--accent-red) 10%, transparent);
  color: var(--accent-red);
  border-color: color-mix(in srgb, var(--accent-red) 22%, transparent);
}

.empty-panel {
  text-align: center;
  padding: 2rem 1.25rem;
}

.empty-panel h3 {
  margin: 0 0 0.5rem;
}

.materials-groups {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.group-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 1rem;
  margin-bottom: 0.8rem;
}

.group-header h2,
.asset-main h3 {
  margin: 0;
}

.asset-list {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.asset-row {
  display: flex;
  justify-content: space-between;
  gap: 1rem;
  align-items: center;
  padding: 1rem;
  border: 1px solid var(--panel-border);
  border-radius: 16px;
  background: var(--panel-bg);
}

.asset-main {
  min-width: 0;
}

.asset-main p {
  margin: 0.35rem 0;
}

.asset-meta {
  display: flex;
  gap: 0.8rem;
  flex-wrap: wrap;
  font-size: 0.84rem;
}

.action-btn,
.mini-btn {
  border: none;
  cursor: pointer;
  border-radius: 14px;
  padding: 0.75rem 1rem;
  background: color-mix(in srgb, var(--card-bg) 65%, transparent);
  color: var(--text-main);
}

.action-btn.primary,
.mini-btn {
  background: var(--accent-blue-light);
  color: var(--accent-blue);
}

.mini-btn.danger {
  background: color-mix(in srgb, var(--accent-red) 12%, transparent);
  color: var(--accent-red);
}

@media (max-width: 820px) {
  .materials-view {
    padding: 1.4rem 1rem 6rem;
  }

  .materials-header,
  .search-panel,
  .asset-row,
  .form-actions {
    flex-direction: column;
    align-items: stretch;
  }

  .form-grid {
    grid-template-columns: 1fr;
  }
}
</style>
