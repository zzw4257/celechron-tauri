<script setup lang="ts">
import { convertFileSrc } from '@tauri-apps/api/core';
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import type { MaterialAsset, MaterialsPayload, RemoteMaterialAsset } from '../../types/api';
import {
  cacheRemoteMaterial,
  downloadMaterialAsset,
  fetchMaterials,
  openMaterialAsset,
  readMaterialText,
  removeMaterialCache,
  runAiAnalysis,
  sendDingtalkTest,
  syncMaterialsIndex,
} from '../../services/api';
import { usePreferences } from '../../composables/usePreferences';

const {
  accountScope,
  dingtalkWebhookEnabled,
  dingtalkWebhookSecret,
  dingtalkWebhookUrl,
  zeroClawApiKey,
  zeroClawEndpoint,
} = usePreferences();

const isLoading = ref(true);
const isSyncing = ref(false);
const isSubmitting = ref(false);
const remoteDownloadingId = ref('');
const errorMsg = ref('');
const actionStatus = ref('');
const items = ref<MaterialAsset[]>([]);
const remoteItems = ref<RemoteMaterialAsset[]>([]);
const lastSyncedAt = ref<number | null>(null);
const warnings = ref<string[]>([]);
const search = ref('');
const showAddForm = ref(false);
const selectedLocalPath = ref('');
const previewText = ref('');
const previewTruncated = ref(false);
const previewLoading = ref(false);
const aiMarkdown = ref('');
const aiError = ref('');
const aiLoading = ref(false);
const draft = ref({
  courseName: '',
  title: '',
  url: '',
  fileName: '',
});

const selectedItem = computed(() => items.value.find((item) => item.relativePath === selectedLocalPath.value) || null);

const localKeywordItems = computed(() => {
  const keyword = search.value.trim().toLowerCase();
  if (!keyword) return items.value;
  return items.value.filter((item) =>
    [item.courseName, item.title, item.fileName].join(' ').toLowerCase().includes(keyword),
  );
});

const remoteKeywordItems = computed(() => {
  const keyword = search.value.trim().toLowerCase();
  if (!keyword) return remoteItems.value;
  return remoteItems.value.filter((item) =>
    [item.courseName, item.title, item.fileName, item.sourceType].join(' ').toLowerCase().includes(keyword),
  );
});

const groupedLocalItems = computed(() => {
  const groups = new Map<string, MaterialAsset[]>();
  for (const item of localKeywordItems.value) {
    const key = item.courseName || '未分组课程';
    const bucket = groups.get(key) || [];
    bucket.push(item);
    groups.set(key, bucket);
  }
  return [...groups.entries()].map(([courseName, assets]) => ({ courseName, assets }));
});

const groupedRemoteItems = computed(() => {
  const groups = new Map<string, RemoteMaterialAsset[]>();
  for (const item of remoteKeywordItems.value) {
    const key = item.courseName || '未分组课程';
    const bucket = groups.get(key) || [];
    bucket.push(item);
    groups.set(key, bucket);
  }
  return [...groups.entries()].map(([courseName, assets]) => ({ courseName, assets }));
});

const stats = computed(() => ({
  localCount: items.value.length,
  remoteCount: remoteItems.value.length,
  availableRemoteCount: remoteItems.value.filter((item) => !item.downloaded).length,
}));

const previewMode = computed(() => {
  const item = selectedItem.value;
  if (!item) return 'empty';
  const fileName = item.fileName.toLowerCase();
  const mime = (item.mimeType || '').toLowerCase();
  if (mime.startsWith('image/') || /\.(png|jpg|jpeg|gif|webp|svg)$/i.test(fileName)) return 'image';
  if (mime.includes('pdf') || fileName.endsWith('.pdf')) return 'pdf';
  if (
    mime.startsWith('text/') ||
    mime.includes('json') ||
    mime.includes('xml') ||
    /\.(txt|md|markdown|json|csv|tsv|yaml|yml|xml|html|htm|js|ts|jsx|tsx|py|rs|java|c|cpp|h|hpp)$/i.test(fileName)
  ) {
    return 'text';
  }
  return 'unsupported';
});

const previewUrl = computed(() => {
  if (!selectedItem.value || previewMode.value === 'text' || previewMode.value === 'empty') {
    return '';
  }
  return convertFileSrc(selectedItem.value.absolutePath);
});

function hydrate(payload: MaterialsPayload) {
  items.value = Array.isArray(payload.items) ? payload.items : [];
  remoteItems.value = Array.isArray(payload.remoteItems) ? payload.remoteItems : [];
  lastSyncedAt.value = typeof payload.lastSyncedAt === 'number' ? payload.lastSyncedAt : null;
  warnings.value = Array.isArray(payload.warnings) ? payload.warnings : [];

  if (!selectedLocalPath.value || !items.value.some((item) => item.relativePath === selectedLocalPath.value)) {
    selectedLocalPath.value = items.value[0]?.relativePath || '';
  }
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

function formatTime(ts?: number | null) {
  if (!ts) return '未同步';
  return new Date(ts * 1000).toLocaleString('zh-CN', { hour12: false });
}

function materialSourceLabel(sourceType: string) {
  if (sourceType === 'activity') return '课程活动';
  if (sourceType === 'homework') return '作业附件';
  return '远程资料';
}

async function loadMaterials() {
  isLoading.value = true;
  errorMsg.value = '';
  try {
    const env = await fetchMaterials();
    hydrate(env.data);
  } catch (error: any) {
    errorMsg.value = error?.message || String(error);
  } finally {
    isLoading.value = false;
  }
}

async function loadTextPreview(item: MaterialAsset | null) {
  previewText.value = '';
  previewTruncated.value = false;
  aiMarkdown.value = '';
  aiError.value = '';

  if (!item || previewMode.value !== 'text') {
    return;
  }

  previewLoading.value = true;
  try {
    const env = await readMaterialText(item.relativePath, 30000);
    previewText.value = env.data.content || '';
    previewTruncated.value = Boolean(env.data.truncated);
  } catch (error: any) {
    previewText.value = '';
    previewTruncated.value = false;
    actionStatus.value = error?.message || String(error);
  } finally {
    previewLoading.value = false;
  }
}

function selectLocalItem(item: MaterialAsset) {
  selectedLocalPath.value = item.relativePath;
  actionStatus.value = '';
}

async function handleDownload() {
  if (!draft.value.courseName || !draft.value.title || !draft.value.url || isSubmitting.value) {
    return;
  }
  isSubmitting.value = true;
  actionStatus.value = '正在下载资料...';
  try {
    const env = await downloadMaterialAsset({
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
    const item = env.data.item as MaterialAsset | undefined;
    if (item?.relativePath) {
      selectedLocalPath.value = item.relativePath;
    }
  } catch (error: any) {
    actionStatus.value = error?.message || String(error);
  } finally {
    isSubmitting.value = false;
  }
}

async function handleSyncRemote() {
  if (isSyncing.value) return;
  isSyncing.value = true;
  actionStatus.value = '正在同步学在浙大资料索引...';
  errorMsg.value = '';
  const knownAvailable = new Set(remoteItems.value.filter((item) => !item.downloaded).map((item) => item.id));
  try {
    const env = await syncMaterialsIndex();
    hydrate(env.data);
    const newlyAvailable = remoteItems.value.filter((item) => !item.downloaded && !knownAvailable.has(item.id));
    actionStatus.value = `远程资料已同步，当前 ${remoteItems.value.length} 份，可直接缓存 ${stats.value.availableRemoteCount} 份。`;

    if (
      newlyAvailable.length > 0 &&
      dingtalkWebhookEnabled.value &&
      dingtalkWebhookUrl.value
    ) {
      const sample = newlyAvailable.slice(0, 5).map((item) => `- ${item.courseName} / ${item.fileName}`).join('\n');
      try {
        await sendDingtalkTest({
          webhookUrl: dingtalkWebhookUrl.value,
          secret: dingtalkWebhookSecret.value || undefined,
          title: 'Celechron 资料同步提醒',
          text: `### Celechron 资料同步提醒\n\n- 新增未缓存资料：${newlyAvailable.length} 份\n- 同步时间：${new Date().toLocaleString('zh-CN', { hour12: false })}\n${sample || '- 无示例'}`,
        });
        actionStatus.value += ' 已向 DingTalk 发送同步提醒。';
      } catch (notifyError: any) {
        actionStatus.value += ` DingTalk 提醒失败：${notifyError?.message || String(notifyError)}`;
      }
    }
  } catch (error: any) {
    errorMsg.value = error?.message || String(error);
  } finally {
    isSyncing.value = false;
  }
}

async function handleCacheRemote(item: RemoteMaterialAsset) {
  if (remoteDownloadingId.value) return;
  remoteDownloadingId.value = item.id;
  actionStatus.value = `正在缓存 ${item.fileName}...`;
  try {
    const env = await cacheRemoteMaterial({
      uploadId: item.uploadId,
      referenceId: item.referenceId,
      courseName: item.courseName,
      title: item.title,
      fileName: item.fileName,
      sourceType: item.sourceType,
    });
    actionStatus.value = `${item.fileName} 已缓存到本地`;
    await loadMaterials();
    const created = env.data.item as MaterialAsset | undefined;
    if (created?.relativePath) {
      selectedLocalPath.value = created.relativePath;
    } else if (item.localRelativePath) {
      selectedLocalPath.value = item.localRelativePath;
    }
  } catch (error: any) {
    actionStatus.value = error?.message || String(error);
  } finally {
    remoteDownloadingId.value = '';
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

async function handleOpenRemote(item: RemoteMaterialAsset) {
  if (item.localRelativePath) {
    const local = items.value.find((asset) => asset.relativePath === item.localRelativePath);
    if (local) {
      selectLocalItem(local);
      await handleOpen(local);
      return;
    }
  }
  await handleCacheRemote(item);
}

async function handleRemove(item: MaterialAsset) {
  actionStatus.value = '';
  try {
    await removeMaterialCache(item.relativePath);
    actionStatus.value = '已删除本地缓存';
    if (selectedLocalPath.value === item.relativePath) {
      selectedLocalPath.value = '';
    }
    await loadMaterials();
  } catch (error: any) {
    actionStatus.value = error?.message || String(error);
  }
}

async function handleAnalyzeSelected() {
  const item = selectedItem.value;
  if (!item || aiLoading.value) return;
  if (!zeroClawEndpoint.value) {
    aiError.value = '请先在设置页填写 ZeroClaw Endpoint';
    return;
  }
  aiLoading.value = true;
  aiError.value = '';
  try {
    let excerpt = '';
    if (previewMode.value === 'text') {
      if (!previewText.value) {
        await loadTextPreview(item);
      }
      excerpt = previewText.value;
    }

    const env = await runAiAnalysis({
      baseUrl: zeroClawEndpoint.value,
      apiKey: zeroClawApiKey.value || undefined,
      prompt: previewMode.value === 'text'
        ? '请阅读给定课程资料片段，输出中文学习摘要、重点概念、可能考点和建议的复习顺序。引用时只引用必要短句。'
        : '请基于资料元数据，给出中文学习建议、推荐预习方法和需要人工打开原文件确认的风险点。',
      context: {
        material: {
          courseName: item.courseName,
          title: item.title,
          fileName: item.fileName,
          mimeType: item.mimeType,
          sizeBytes: item.sizeBytes,
          updatedAt: item.updatedAt,
          previewMode: previewMode.value,
          truncated: previewTruncated.value,
        },
        excerpt,
      },
    });
    aiMarkdown.value = env.data.markdown || '';
    if (!aiMarkdown.value) {
      aiError.value = 'ZeroClaw 已响应，但未返回可展示的 markdown 内容';
    }
  } catch (error: any) {
    aiError.value = error?.message || String(error);
  } finally {
    aiLoading.value = false;
  }
}

function handleMaterialsFocus(event: Event) {
  const detail = (event as CustomEvent<{ relativePath?: string; remoteId?: string }>).detail || {};
  if (detail.relativePath) {
    const local = items.value.find((item) => item.relativePath === detail.relativePath);
    if (local) {
      selectLocalItem(local);
      return;
    }
  }
  if (detail.remoteId) {
    const remote = remoteItems.value.find((item) => item.id === detail.remoteId);
    if (remote?.localRelativePath) {
      const local = items.value.find((item) => item.relativePath === remote.localRelativePath);
      if (local) {
        selectLocalItem(local);
      }
    }
  }
}

watch(selectedItem, (item) => {
  loadTextPreview(item);
}, { immediate: true });

watch(accountScope, () => {
  loadMaterials();
});

onMounted(() => {
  loadMaterials();
  window.addEventListener('celechron:materials-focus', handleMaterialsFocus as EventListener);
});

onUnmounted(() => {
  window.removeEventListener('celechron:materials-focus', handleMaterialsFocus as EventListener);
});
</script>

<template>
  <div class="materials-view">
    <header class="materials-header">
      <div>
        <h1>资料</h1>
        <p>统一管理课程课件、讲稿和本地缓存。现已接入学在浙大资料索引同步、本地预览，以及结合 ZeroClaw 的资料分析。</p>
      </div>
      <div class="materials-actions">
        <button class="action-btn" @click="loadMaterials">刷新本地</button>
        <button class="action-btn" :disabled="isSyncing" @click="handleSyncRemote">
          {{ isSyncing ? '同步中...' : '同步远程资料' }}
        </button>
        <button class="action-btn primary" @click="showAddForm = !showAddForm">
          {{ showAddForm ? '收起' : '添加资料' }}
        </button>
      </div>
    </header>

    <section class="stats-grid">
      <article class="panel stat-card">
        <span class="stat-label">本地缓存</span>
        <strong>{{ stats.localCount }}</strong>
        <span class="stat-desc">已下载并可离线查看</span>
      </article>
      <article class="panel stat-card">
        <span class="stat-label">远程索引</span>
        <strong>{{ stats.remoteCount }}</strong>
        <span class="stat-desc">来自学在浙大当前课程</span>
      </article>
      <article class="panel stat-card">
        <span class="stat-label">待缓存</span>
        <strong>{{ stats.availableRemoteCount }}</strong>
        <span class="stat-desc">上次同步 {{ formatTime(lastSyncedAt) }}</span>
      </article>
    </section>

    <section class="panel search-panel">
      <input v-model="search" class="search-input" placeholder="搜索课程名、资料标题或文件名" />
      <span class="search-meta">本地 {{ localKeywordItems.length }} 份 / 远程 {{ remoteKeywordItems.length }} 份</span>
    </section>

    <section v-if="showAddForm" class="panel form-panel">
      <div class="form-grid">
        <input v-model="draft.courseName" class="form-input" placeholder="课程名" />
        <input v-model="draft.title" class="form-input" placeholder="资料标题" />
        <input v-model="draft.fileName" class="form-input" placeholder="文件名（可选）" />
        <input v-model="draft.url" class="form-input form-input-wide" placeholder="可下载 URL" />
      </div>
      <div class="form-actions">
        <span class="hint">手动 URL 仍可直接缓存；donor 风格的学在浙大资料请点“同步远程资料”。</span>
        <button class="action-btn primary" :disabled="isSubmitting" @click="handleDownload">
          {{ isSubmitting ? '下载中...' : '下载并缓存' }}
        </button>
      </div>
    </section>

    <div v-if="actionStatus" class="status-banner">{{ actionStatus }}</div>
    <div v-if="errorMsg" class="status-banner error">{{ errorMsg }}</div>
    <div v-if="warnings.length" class="status-banner warning">
      <strong>同步提示：</strong>
      <span>{{ warnings.join('；') }}</span>
    </div>

    <section v-if="isLoading" class="panel empty-panel">
      <p>资料索引加载中...</p>
    </section>

    <section v-else class="materials-layout">
      <div class="materials-column">
        <article class="panel list-panel">
          <div class="list-header">
            <h2>本地缓存</h2>
            <span>{{ localKeywordItems.length }} 份</span>
          </div>
          <div v-if="groupedLocalItems.length === 0" class="empty-inline">
            暂无本地资料，可先同步远程索引或手动添加 URL。
          </div>
          <div v-else class="materials-groups">
            <article v-for="group in groupedLocalItems" :key="group.courseName" class="course-group">
              <div class="group-header">
                <h3>{{ group.courseName }}</h3>
                <span>{{ group.assets.length }} 份</span>
              </div>
              <div class="asset-list">
                <div
                  v-for="item in group.assets"
                  :key="item.relativePath"
                  class="asset-row local"
                  :class="{ active: item.relativePath === selectedLocalPath }"
                  @click="selectLocalItem(item)"
                >
                  <div class="asset-main">
                    <strong>{{ item.title }}</strong>
                    <p>{{ item.fileName }}</p>
                    <div class="asset-meta">
                      <span>{{ formatBytes(item.sizeBytes) }}</span>
                      <span>{{ formatTime(item.updatedAt) }}</span>
                      <span v-if="item.sourceUrl">已记录来源</span>
                    </div>
                  </div>
                  <div class="asset-actions" @click.stop>
                    <button class="mini-btn" @click="handleOpen(item)">打开</button>
                    <button class="mini-btn danger" @click="handleRemove(item)">删除</button>
                  </div>
                </div>
              </div>
            </article>
          </div>
        </article>

        <article class="panel list-panel">
          <div class="list-header">
            <h2>远程资料</h2>
            <span>{{ remoteKeywordItems.length }} 份</span>
          </div>
          <div v-if="groupedRemoteItems.length === 0" class="empty-inline">
            还没有远程资料索引。请先登录并点击“同步远程资料”。
          </div>
          <div v-else class="materials-groups">
            <article v-for="group in groupedRemoteItems" :key="`${group.courseName}-remote`" class="course-group">
              <div class="group-header">
                <h3>{{ group.courseName }}</h3>
                <span>{{ group.assets.length }} 份</span>
              </div>
              <div class="asset-list">
                <div v-for="item in group.assets" :key="item.id" class="asset-row remote">
                  <div class="asset-main">
                    <strong>{{ item.title }}</strong>
                    <p>{{ item.fileName }}</p>
                    <div class="asset-meta">
                      <span>{{ materialSourceLabel(item.sourceType) }}</span>
                      <span>{{ formatBytes(item.sizeBytes) }}</span>
                      <span>{{ formatTime(item.updatedAt) }}</span>
                    </div>
                  </div>
                  <div class="asset-actions">
                    <span class="badge" :class="item.downloaded ? 'success' : 'muted'">
                      {{ item.downloaded ? '已缓存' : '远程' }}
                    </span>
                    <button class="mini-btn" :disabled="remoteDownloadingId === item.id" @click="handleOpenRemote(item)">
                      {{ item.downloaded ? '打开' : (remoteDownloadingId === item.id ? '缓存中...' : '缓存') }}
                    </button>
                  </div>
                </div>
              </div>
            </article>
          </div>
        </article>
      </div>

      <aside class="panel preview-panel">
        <div class="preview-header">
          <div>
            <h2>{{ selectedItem?.title || '预览与分析' }}</h2>
            <p v-if="selectedItem">{{ selectedItem.courseName }} / {{ selectedItem.fileName }}</p>
            <p v-else>选择一份本地资料后，可直接预览并结合 ZeroClaw 做学习分析。</p>
          </div>
          <div v-if="selectedItem" class="preview-actions">
            <button class="mini-btn" @click="handleOpen(selectedItem)">外部打开</button>
            <button class="mini-btn primary" :disabled="aiLoading || !selectedItem" @click="handleAnalyzeSelected">
              {{ aiLoading ? '分析中...' : '结合资料分析' }}
            </button>
          </div>
        </div>

        <div v-if="!selectedItem" class="empty-inline large">
          当前没有选中本地资料。
        </div>

        <template v-else>
          <div class="preview-card">
            <template v-if="previewMode === 'image'">
              <img :src="previewUrl" class="preview-image" alt="资料预览" />
            </template>
            <template v-else-if="previewMode === 'pdf'">
              <iframe :src="previewUrl" class="preview-frame" title="PDF 预览"></iframe>
            </template>
            <template v-else-if="previewMode === 'text'">
              <div v-if="previewLoading" class="empty-inline">文本预览加载中...</div>
              <pre v-else class="preview-text">{{ previewText || '文本预览为空' }}</pre>
              <div v-if="previewTruncated" class="preview-hint">文本过长，当前仅展示前 30000 个字符，AI 分析也将基于该截断片段。</div>
            </template>
            <template v-else>
              <div class="empty-inline large">当前文件类型暂不支持内嵌预览，请使用“外部打开”。</div>
            </template>
          </div>

          <div v-if="aiError" class="status-banner error">{{ aiError }}</div>
          <article class="analysis-panel">
            <div class="analysis-header">
              <h3>ZeroClaw 综合分析</h3>
              <span>{{ zeroClawEndpoint ? '已配置' : '未配置 Endpoint' }}</span>
            </div>
            <div v-if="aiMarkdown" class="analysis-markdown">{{ aiMarkdown }}</div>
            <div v-else class="empty-inline large">
              {{ zeroClawEndpoint ? '点击“结合资料分析”后将在这里展示学习摘要、重点概念和复习建议。' : '请先到设置页配置 ZeroClaw Endpoint。' }}
            </div>
          </article>
        </template>
      </aside>
    </section>
  </div>
</template>

<style scoped>
.materials-view {
  padding: 2rem 2.5rem 6rem;
  max-width: 1400px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 1rem;
  color: var(--text-main);
}

.materials-header,
.list-header,
.group-header,
.preview-header,
.analysis-header,
.materials-actions,
.form-actions,
.asset-actions,
.preview-actions {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
}

.materials-header {
  align-items: flex-start;
}

.materials-header h1,
.list-header h2,
.group-header h3,
.preview-header h2,
.analysis-header h3 {
  margin: 0;
}

.materials-header p,
.preview-header p,
.search-meta,
.hint,
.asset-meta,
.asset-main p,
.stat-desc,
.stat-label,
.preview-hint,
.analysis-header span {
  color: var(--text-muted);
}

.stats-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 1rem;
}

.stat-card {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.stat-card strong {
  font-size: 2rem;
  line-height: 1;
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

.status-banner.warning {
  background: color-mix(in srgb, var(--accent-amber) 12%, transparent);
  color: var(--accent-amber);
  border-color: color-mix(in srgb, var(--accent-amber) 24%, transparent);
}

.materials-layout {
  display: grid;
  grid-template-columns: minmax(0, 1.15fr) minmax(360px, 0.85fr);
  gap: 1rem;
  align-items: start;
}

.materials-column,
.materials-groups,
.asset-list,
.preview-panel,
.analysis-panel {
  display: flex;
  flex-direction: column;
  gap: 0.9rem;
}

.list-panel {
  min-height: 280px;
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

.asset-row.local {
  width: 100%;
  text-align: left;
  cursor: pointer;
  border: 1px solid var(--panel-border);
}

.asset-row.local.active {
  border-color: color-mix(in srgb, var(--accent-blue) 45%, var(--panel-border));
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--accent-blue) 18%, transparent);
}

.asset-main {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.asset-main strong,
.asset-main p {
  margin: 0;
}

.asset-main p {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.asset-meta {
  display: flex;
  gap: 0.75rem;
  flex-wrap: wrap;
  font-size: 0.84rem;
}

.badge {
  border-radius: 999px;
  padding: 0.25rem 0.7rem;
  font-size: 0.78rem;
  border: 1px solid transparent;
}

.badge.success {
  background: color-mix(in srgb, var(--accent-green) 14%, transparent);
  color: var(--accent-green);
  border-color: color-mix(in srgb, var(--accent-green) 24%, transparent);
}

.badge.muted {
  background: color-mix(in srgb, var(--text-muted) 14%, transparent);
  color: var(--text-muted);
  border-color: color-mix(in srgb, var(--text-muted) 22%, transparent);
}

.action-btn,
.mini-btn {
  border: none;
  cursor: pointer;
  border-radius: 12px;
  padding: 0.72rem 1rem;
  background: var(--panel-bg);
  color: var(--text-main);
  transition: transform 0.2s ease, opacity 0.2s ease;
}

.mini-btn {
  padding: 0.55rem 0.85rem;
  border: 1px solid var(--panel-border);
}

.action-btn:hover,
.mini-btn:hover {
  transform: translateY(-1px);
}

.action-btn:disabled,
.mini-btn:disabled {
  cursor: not-allowed;
  opacity: 0.6;
  transform: none;
}

.action-btn.primary,
.mini-btn.primary {
  background: var(--accent-blue);
  color: var(--text-inverse);
}

.mini-btn.danger {
  color: var(--accent-red);
}

.preview-card {
  border: 1px solid var(--panel-border);
  border-radius: 18px;
  background: var(--panel-bg);
  min-height: 280px;
  overflow: hidden;
}

.preview-image,
.preview-frame {
  display: block;
  width: 100%;
  min-height: 440px;
  border: none;
  background: var(--bg-main);
}

.preview-image {
  object-fit: contain;
  max-height: 560px;
}

.preview-text {
  margin: 0;
  padding: 1rem 1.1rem;
  white-space: pre-wrap;
  word-break: break-word;
  color: var(--text-main);
  font-size: 0.92rem;
  line-height: 1.6;
}

.analysis-panel {
  border-top: 1px solid var(--panel-border);
  padding-top: 1rem;
}

.analysis-markdown {
  white-space: pre-wrap;
  word-break: break-word;
  line-height: 1.7;
  color: var(--text-main);
}

.empty-panel,
.empty-inline {
  text-align: center;
  color: var(--text-muted);
}

.empty-inline.large {
  min-height: 140px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px dashed var(--panel-border);
  border-radius: 16px;
}

@media (max-width: 1080px) {
  .materials-layout {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 780px) {
  .materials-view {
    padding: 1.25rem 1rem 6rem;
  }

  .materials-header,
  .search-panel,
  .preview-header,
  .materials-actions,
  .form-actions,
  .asset-row,
  .asset-actions,
  .preview-actions {
    flex-direction: column;
    align-items: stretch;
  }

  .stats-grid,
  .form-grid {
    grid-template-columns: 1fr;
  }

  .preview-image,
  .preview-frame {
    min-height: 320px;
  }
}
</style>
