<script setup lang="ts">
import { convertFileSrc } from '@tauri-apps/api/core';
import { computed, onMounted, ref, watch } from 'vue';
import ActionPill from '../ui/ActionPill.vue';
import InlineStat from '../ui/InlineStat.vue';
import SectionCard from '../ui/SectionCard.vue';
import SegmentedFilter from '../ui/SegmentedFilter.vue';
import StatusBanner from '../ui/StatusBanner.vue';
import type { MaterialAsset, MaterialsPayload, RemoteMaterialAsset } from '../../types/api';
import {
  cacheRemoteMaterial,
  downloadMaterialAsset,
  fetchMaterials,
  openMaterialAsset,
  readMaterialText,
  removeMaterialCache,
  runAiAnalysis,
  syncMaterialsIndex,
} from '../../services/api';
import { usePreferences } from '../../composables/usePreferences';

const {
  accountScope,
  zeroClawApiKey,
  zeroClawEndpoint,
} = usePreferences();

const isLoading = ref(true);
const isSyncing = ref(false);
const isSubmitting = ref(false);
const previewLoading = ref(false);
const aiLoading = ref(false);
const errorMsg = ref('');
const actionStatus = ref('');
const aiError = ref('');
const aiMarkdown = ref('');
const previewText = ref('');
const previewTruncated = ref(false);
const items = ref<MaterialAsset[]>([]);
const remoteItems = ref<RemoteMaterialAsset[]>([]);
const warnings = ref<string[]>([]);
const lastSyncedAt = ref<number | null>(null);
const weekLabel = ref('');
const defaultScope = ref<'current-week' | 'all'>('current-week');
const sourcePriority = ref<string[]>(['classroom', 'activity', 'homework']);
const courseFilters = ref<{ id: string; label: string; count: number }[]>([]);
const selectedScope = ref<'current-week' | 'all'>('current-week');
const selectedSource = ref('all');
const selectedCourse = ref('all');
const search = ref('');
const selectedRemoteId = ref('');
const selectedLocalPath = ref('');
const showAddForm = ref(false);
const draft = ref({
  courseName: '',
  title: '',
  url: '',
  fileName: '',
});

const sourceLabelMap: Record<string, string> = {
  classroom: '智云课堂',
  activity: '课程活动',
  homework: '作业附件',
};

function navigateToSettings() {
  window.dispatchEvent(new CustomEvent('celechron:navigate', { detail: { tab: 'option' } }));
}

function hydrate(payload: MaterialsPayload) {
  items.value = Array.isArray(payload.items) ? payload.items : [];
  remoteItems.value = Array.isArray(payload.remoteItems) ? payload.remoteItems : [];
  warnings.value = Array.isArray(payload.warnings) ? payload.warnings : [];
  lastSyncedAt.value = typeof payload.lastSyncedAt === 'number' ? payload.lastSyncedAt : null;
  weekLabel.value = payload.weekLabel || '';
  defaultScope.value = payload.defaultScope === 'all' ? 'all' : 'current-week';
  sourcePriority.value = Array.isArray(payload.sourcePriority) && payload.sourcePriority.length
    ? payload.sourcePriority
    : ['classroom', 'activity', 'homework'];
  courseFilters.value = Array.isArray(payload.courseFilters) ? payload.courseFilters : [];

  if (!selectedRemoteId.value || !remoteItems.value.some((item) => item.id === selectedRemoteId.value)) {
    selectedRemoteId.value = remoteItems.value[0]?.id || '';
  }

  if (!selectedLocalPath.value || !items.value.some((item) => item.relativePath === selectedLocalPath.value)) {
    selectedLocalPath.value = items.value[0]?.relativePath || '';
  }
}

function formatTime(ts?: number | null) {
  if (!ts) return '未同步';
  return new Date(ts * 1000).toLocaleString('zh-CN', { hour12: false });
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function sourceRank(type: string) {
  const index = sourcePriority.value.indexOf(type);
  return index === -1 ? sourcePriority.value.length : index;
}

const sourceOptions = computed(() => {
  const counts = new Map<string, number>();
  for (const item of remoteItems.value) {
    counts.set(item.sourceType, (counts.get(item.sourceType) || 0) + 1);
  }
  const options = [{ value: 'all', label: '全部来源', badge: remoteItems.value.length }];
  for (const source of sourcePriority.value) {
    if (!counts.has(source)) continue;
    options.push({
      value: source,
      label: sourceLabelMap[source] || source,
      badge: counts.get(source) || 0,
    });
  }
  return options;
});

const courseOptions = computed(() => [
  { value: 'all', label: '我的课程', badge: remoteItems.value.length },
  ...courseFilters.value.map((item) => ({ value: item.id, label: item.label, badge: item.count })),
]);

const visibleRemoteItems = computed(() => {
  const keyword = search.value.trim().toLowerCase();
  return [...remoteItems.value]
    .filter((item) => selectedScope.value === 'all' || item.weekBucket === 'current')
    .filter((item) => selectedSource.value === 'all' || item.sourceType === selectedSource.value)
    .filter((item) => selectedCourse.value === 'all' || item.courseName === selectedCourse.value)
    .filter((item) => {
      if (!keyword) return true;
      return [item.courseName, item.title, item.fileName, sourceLabelMap[item.sourceType] || item.sourceType]
        .join(' ')
        .toLowerCase()
        .includes(keyword);
    })
    .sort((left, right) => {
      return sourceRank(left.sourceType) - sourceRank(right.sourceType) || right.updatedAt - left.updatedAt;
    });
});

const visibleLocalItems = computed(() => {
  const keyword = search.value.trim().toLowerCase();
  return [...items.value].filter((item) => {
    const matchKeyword = !keyword || [item.courseName, item.title, item.fileName].join(' ').toLowerCase().includes(keyword);
    const matchCourse = selectedCourse.value === 'all' || item.courseName === selectedCourse.value;
    return matchKeyword && matchCourse;
  });
});

const summaryStats = computed(() => ({
  currentWeek: remoteItems.value.filter((item) => item.weekBucket === 'current').length,
  cached: items.value.length,
  remoteTotal: remoteItems.value.length,
  lastSynced: formatTime(lastSyncedAt.value),
}));

const selectedRemoteItem = computed(() => {
  return visibleRemoteItems.value.find((item) => item.id === selectedRemoteId.value)
    || remoteItems.value.find((item) => item.id === selectedRemoteId.value)
    || visibleRemoteItems.value[0]
    || null;
});

const selectedPreviewAsset = computed(() => {
  const localFromSelection = items.value.find((item) => item.relativePath === selectedLocalPath.value);
  if (localFromSelection) return localFromSelection;
  const remote = selectedRemoteItem.value;
  if (!remote?.downloaded || !remote.localRelativePath) return null;
  return items.value.find((item) => item.relativePath === remote.localRelativePath) || null;
});

const previewMode = computed(() => {
  const asset = selectedPreviewAsset.value;
  if (!asset) return 'empty';
  const fileName = asset.fileName.toLowerCase();
  const mime = (asset.mimeType || '').toLowerCase();
  if (mime.includes('html') || fileName.endsWith('.html') || fileName.endsWith('.htm')) return 'web';
  if (mime.startsWith('image/') || /\.(png|jpg|jpeg|gif|webp|svg)$/i.test(fileName)) return 'image';
  if (mime.includes('pdf') || fileName.endsWith('.pdf')) return 'pdf';
  if (mime.startsWith('text/') || /\.(txt|md|markdown|json|csv|tsv|yaml|yml|xml)$/i.test(fileName)) return 'text';
  return 'unsupported';
});

const previewUrl = computed(() => {
  const asset = selectedPreviewAsset.value;
  if (!asset || previewMode.value === 'text' || previewMode.value === 'empty') return '';
  return convertFileSrc(asset.absolutePath);
});

async function loadMaterials() {
  isLoading.value = true;
  errorMsg.value = '';
  actionStatus.value = '';
  try {
    const env = await fetchMaterials();
    hydrate(env.data);
    selectedScope.value = env.data.defaultScope === 'all' ? 'all' : 'current-week';
  } catch (error: any) {
    errorMsg.value = error?.message || String(error);
  } finally {
    isLoading.value = false;
  }
}

async function syncRemoteMaterials() {
  isSyncing.value = true;
  errorMsg.value = '';
  actionStatus.value = '';
  try {
    const env = await syncMaterialsIndex();
    hydrate(env.data);
    if (!selectedScope.value) selectedScope.value = env.data.defaultScope === 'all' ? 'all' : 'current-week';
    actionStatus.value = '资料索引已刷新，并回到了新的过滤结果。';
  } catch (error: any) {
    errorMsg.value = error?.message || String(error);
  } finally {
    isSyncing.value = false;
  }
}

async function addMaterial() {
  isSubmitting.value = true;
  errorMsg.value = '';
  actionStatus.value = '';
  try {
    await downloadMaterialAsset({
      courseName: draft.value.courseName.trim() || '手动资料',
      title: draft.value.title.trim() || draft.value.fileName.trim() || '手动添加资料',
      url: draft.value.url.trim(),
      fileName: draft.value.fileName.trim() || undefined,
      source: 'manual',
    });
    draft.value = { courseName: '', title: '', url: '', fileName: '' };
    showAddForm.value = false;
    await loadMaterials();
    actionStatus.value = '资料已加入本地缓存。';
  } catch (error: any) {
    errorMsg.value = error?.message || String(error);
  } finally {
    isSubmitting.value = false;
  }
}

async function cacheRemote(item: RemoteMaterialAsset) {
  actionStatus.value = '';
  errorMsg.value = '';
  try {
    await cacheRemoteMaterial({ remoteId: item.id });
    await loadMaterials();
    selectedRemoteId.value = item.id;
    actionStatus.value = `已缓存 ${item.title}`;
  } catch (error: any) {
    errorMsg.value = error?.message || String(error);
  }
}

async function openSelectedAsset() {
  const asset = selectedPreviewAsset.value;
  if (!asset) return;
  try {
    await openMaterialAsset(asset.relativePath);
  } catch (error: any) {
    errorMsg.value = error?.message || String(error);
  }
}

async function removeSelectedAsset() {
  const asset = selectedPreviewAsset.value;
  if (!asset) return;
  try {
    await removeMaterialCache(asset.relativePath);
    await loadMaterials();
    actionStatus.value = `已删除 ${asset.title}`;
  } catch (error: any) {
    errorMsg.value = error?.message || String(error);
  }
}

async function analyzeSelected() {
  const asset = selectedPreviewAsset.value;
  if (!asset) {
    aiError.value = '请先选择已缓存的资料。';
    return;
  }
  if (!zeroClawEndpoint.value) {
    navigateToSettings();
    return;
  }

  aiLoading.value = true;
  aiError.value = '';
  aiMarkdown.value = '';
  try {
    const textEnv = previewMode.value === 'text'
      ? await readMaterialText(asset.relativePath, 22000)
      : null;
    const env = await runAiAnalysis({
      baseUrl: zeroClawEndpoint.value,
      apiKey: zeroClawApiKey.value || undefined,
      prompt: '请基于当前资料生成中文学习摘要、重点概念和复习建议。',
      context: {
        title: asset.title,
        courseName: asset.courseName,
        fileName: asset.fileName,
        previewMode: previewMode.value,
        content: textEnv?.data?.content || '',
      },
    });
    aiMarkdown.value = env.data.markdown || '';
    if (!aiMarkdown.value) {
      aiError.value = 'ZeroClaw 已响应，但没有返回可展示内容。';
    }
  } catch (error: any) {
    aiError.value = error?.message || String(error);
  } finally {
    aiLoading.value = false;
  }
}

watch(selectedPreviewAsset, async (asset) => {
  previewText.value = '';
  previewTruncated.value = false;
  if (!asset || previewMode.value !== 'text') return;
  previewLoading.value = true;
  try {
    const env = await readMaterialText(asset.relativePath, 30000);
    previewText.value = env.data.content || '';
    previewTruncated.value = Boolean(env.data.truncated);
  } catch (error: any) {
    errorMsg.value = error?.message || String(error);
  } finally {
    previewLoading.value = false;
  }
}, { immediate: true });

watch([visibleRemoteItems, visibleLocalItems], ([remoteList, localList]) => {
  if (!remoteList.some((item) => item.id === selectedRemoteId.value)) {
    selectedRemoteId.value = remoteList[0]?.id || '';
  }
  if (!localList.some((item) => item.relativePath === selectedLocalPath.value)) {
    selectedLocalPath.value = localList[0]?.relativePath || '';
  }
}, { immediate: true });

onMounted(loadMaterials);
watch(accountScope, loadMaterials);
</script>

<template>
  <div class="page-shell materials-view">
    <header class="page-header">
      <div>
        <h1>资料</h1>
        <p class="page-subtitle">默认落到 {{ weekLabel || '本周' }}，来源顺序按智云课堂优先聚合。</p>
      </div>
      <ActionPill tone="accent" :disabled="isSyncing" @click="syncRemoteMaterials">
        {{ isSyncing ? '同步中…' : '同步远程资料' }}
      </ActionPill>
    </header>

    <div class="materials-stats">
      <InlineStat label="本周可看" :value="String(summaryStats.currentWeek)" emphasis />
      <InlineStat label="已缓存" :value="String(summaryStats.cached)" />
      <InlineStat label="远程总数" :value="String(summaryStats.remoteTotal)" />
      <InlineStat label="上次同步" :value="summaryStats.lastSynced" />
    </div>

    <StatusBanner v-if="errorMsg" tone="danger" title="资料失败">
      {{ errorMsg }}
    </StatusBanner>
    <StatusBanner v-else-if="actionStatus" tone="success" title="操作完成">
      {{ actionStatus }}
    </StatusBanner>
    <StatusBanner v-for="warning in warnings" :key="warning" tone="warning" title="同步警告">
      {{ warning }}
    </StatusBanner>

    <SectionCard title="过滤" subtitle="先筛来源，再看内容。" dense>
      <div class="filters-layout">
        <SegmentedFilter
          v-model="selectedScope"
          :options="[
            { value: 'current-week', label: weekLabel || '本周' },
            { value: 'all', label: '全部资料' },
          ]"
        />
        <SegmentedFilter v-model="selectedSource" :options="sourceOptions" />
        <SegmentedFilter v-model="selectedCourse" :options="courseOptions" />
      </div>
      <div class="filters-search-row">
        <input v-model="search" class="input-field" placeholder="搜索课程名、标题、来源" />
        <ActionPill @click="showAddForm = !showAddForm">
          {{ showAddForm ? '收起手动添加' : '手动添加资料' }}
        </ActionPill>
      </div>
      <form v-if="showAddForm" class="manual-form" @submit.prevent="addMaterial">
        <input v-model="draft.courseName" class="input-field" placeholder="课程名" />
        <input v-model="draft.title" class="input-field" placeholder="标题" />
        <input v-model="draft.fileName" class="input-field" placeholder="文件名（可选）" />
        <input v-model="draft.url" class="input-field manual-form__wide" placeholder="资料链接" required />
        <div class="manual-form__actions">
          <ActionPill type="submit" tone="accent" :disabled="isSubmitting">{{ isSubmitting ? '添加中…' : '添加并缓存' }}</ActionPill>
        </div>
      </form>
    </SectionCard>

    <div v-if="isLoading" class="materials-layout">
      <SectionCard title="加载中" subtitle="正在读取资料索引。">
        <div class="state-card">请稍候，正在恢复本地缓存与远程列表。</div>
      </SectionCard>
    </div>

    <div v-else class="materials-layout">
      <div class="materials-content">
        <SectionCard title="远程资料" subtitle="内容优先，缓存作为操作结果。">
          <div v-if="visibleRemoteItems.length === 0" class="state-card">当前过滤条件下没有远程资料。</div>
          <div v-else class="remote-list">
            <article
              v-for="item in visibleRemoteItems"
              :key="item.id"
              class="remote-item"
              :class="{ active: item.id === selectedRemoteId }"
              @click="selectedRemoteId = item.id"
            >
              <div class="remote-item__main">
                <div class="remote-item__head">
                  <div class="remote-item__title-block">
                    <span class="badge accent">{{ sourceLabelMap[item.sourceType] || item.sourceType }}</span>
                    <strong>{{ item.title }}</strong>
                  </div>
                  <div class="remote-item__badges">
                    <span class="badge" :class="item.weekBucket === 'current' ? 'success' : ''">
                      {{ item.weekBucket === 'current' ? (weekLabel || '本周') : item.weekBucket === 'other' ? '其他周' : '未知周' }}
                    </span>
                    <span class="badge" :class="item.downloaded ? 'accent' : ''">{{ item.downloaded ? '已缓存' : '远程' }}</span>
                  </div>
                </div>
                <p>{{ item.courseName }} · {{ formatTime(item.updatedAt) }} · {{ formatBytes(item.sizeBytes || 0) }}</p>
                <small>{{ item.fileName }}</small>
              </div>
              <div class="remote-item__actions">
                <ActionPill v-if="item.downloaded" @click.stop="selectedLocalPath = item.localRelativePath || ''">查看缓存</ActionPill>
                <ActionPill v-else tone="accent" @click.stop="cacheRemote(item)">缓存</ActionPill>
              </div>
            </article>
          </div>
        </SectionCard>

        <SectionCard title="本地缓存" subtitle="次级区，只收已经落盘的资料。" dense>
          <details class="local-cache" :open="visibleLocalItems.length <= 8">
            <summary>已缓存 {{ visibleLocalItems.length }} 项</summary>
            <div v-if="visibleLocalItems.length === 0" class="state-card">暂无本地缓存资料。</div>
            <div v-else class="local-list">
              <button
                v-for="item in visibleLocalItems"
                :key="item.relativePath"
                type="button"
                class="local-item"
                :class="{ active: item.relativePath === selectedLocalPath }"
                @click="selectedLocalPath = item.relativePath"
              >
                <strong>{{ item.title }}</strong>
                <span>{{ item.courseName }}</span>
                <small>{{ item.fileName }}</small>
              </button>
            </div>
          </details>
        </SectionCard>
      </div>

      <div class="materials-preview-column">
        <SectionCard title="预览" subtitle="桌面端固定在右侧，移动端下沉到列表之后。">
          <div class="preview-actions">
            <span class="badge" :class="selectedPreviewAsset ? 'accent' : ''">
              {{ selectedPreviewAsset ? '已选中缓存资料' : '尚未缓存' }}
            </span>
            <div class="preview-actions__buttons">
              <ActionPill v-if="selectedPreviewAsset" @click="openSelectedAsset">外部打开</ActionPill>
              <ActionPill v-if="selectedPreviewAsset" tone="danger" @click="removeSelectedAsset">删除缓存</ActionPill>
            </div>
          </div>

          <div v-if="selectedPreviewAsset" class="preview-shell">
            <header class="preview-shell__meta">
              <strong>{{ selectedPreviewAsset.title }}</strong>
              <p>{{ selectedPreviewAsset.courseName }} · {{ selectedPreviewAsset.fileName }}</p>
            </header>

            <div v-if="previewMode === 'image'" class="preview-body">
              <img :src="previewUrl" class="preview-image" alt="资料预览" />
            </div>
            <div v-else-if="previewMode === 'pdf' || previewMode === 'web'" class="preview-body">
              <iframe :src="previewUrl" class="preview-frame" title="资料预览"></iframe>
            </div>
            <div v-else-if="previewMode === 'text'" class="preview-body">
              <div v-if="previewLoading" class="state-card">正在读取文本内容…</div>
              <pre v-else class="preview-text">{{ previewText || '文本内容为空。' }}</pre>
              <p v-if="previewTruncated" class="helper-text">文本过长，当前仅展示前 30000 个字符。</p>
            </div>
            <div v-else class="state-card">当前文件类型不支持内嵌预览，请用“外部打开”。</div>
          </div>
          <div v-else-if="selectedRemoteItem?.previewImageUrls?.length" class="preview-remote-gallery">
            <p class="helper-text">该智云课堂资料尚未缓存，先展示可用预览图。</p>
            <img
              v-for="url in selectedRemoteItem.previewImageUrls.slice(0, 6)"
              :key="url"
              :src="url"
              class="preview-remote-image"
              alt="远程预览图"
            />
          </div>
          <div v-else class="state-card">选中远程资料后，可在这里查看缓存内容或预览状态。</div>
        </SectionCard>

        <SectionCard title="AI 综合分析" subtitle="未配置 ZeroClaw 时直接去设置。">
          <div class="ai-actions">
            <ActionPill v-if="zeroClawEndpoint" tone="accent" :disabled="aiLoading" @click="analyzeSelected">
              {{ aiLoading ? '分析中…' : '结合资料分析' }}
            </ActionPill>
            <ActionPill v-else tone="warning" @click="navigateToSettings">去设置</ActionPill>
          </div>
          <StatusBanner v-if="aiError" tone="warning" title="AI 提示">
            {{ aiError }}
          </StatusBanner>
          <article v-if="aiMarkdown" class="ai-markdown">{{ aiMarkdown }}</article>
          <div v-else class="state-card">
            {{ zeroClawEndpoint ? '选择一份已缓存资料后即可生成学习摘要、重点概念和复习建议。' : '尚未配置 ZeroClaw Endpoint，点击“去设置”完成接入。' }}
          </div>
        </SectionCard>
      </div>
    </div>
  </div>
</template>

<style scoped>
.materials-view {
  gap: 1rem;
}

.materials-stats {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 0.75rem;
}

.filters-layout,
.filters-search-row,
.preview-actions,
.preview-actions__buttons,
.ai-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.7rem;
  align-items: center;
}

.filters-search-row {
  margin-top: 0.8rem;
}

.filters-search-row .input-field {
  flex: 1;
}

.manual-form {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 0.7rem;
  margin-top: 0.8rem;
}

.manual-form__wide,
.manual-form__actions {
  grid-column: 1 / -1;
}

.materials-layout {
  display: grid;
  grid-template-columns: minmax(0, 1.2fr) minmax(360px, 0.8fr);
  gap: 1rem;
  align-items: start;
}

.materials-content,
.materials-preview-column,
.remote-list,
.local-list {
  display: flex;
  flex-direction: column;
  gap: 0.8rem;
}

.remote-item {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 0.85rem;
  align-items: center;
  padding: 0.9rem 0.2rem;
  border-bottom: 1px solid var(--border-subtle);
  cursor: pointer;
}

.remote-item:last-child {
  border-bottom: none;
  padding-bottom: 0;
}

.remote-item.active {
  background: var(--surface-2);
  border-radius: var(--radius-card-sm);
  padding: 0.9rem;
  border-bottom-color: transparent;
}

.remote-item__main,
.remote-item__title-block,
.preview-shell__meta {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.remote-item__head,
.remote-item__badges {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 0.55rem;
}

.remote-item__main p,
.remote-item__main small,
.preview-shell__meta p {
  margin: 0;
  color: var(--text-secondary);
}

.local-cache summary {
  cursor: pointer;
  color: var(--text-primary);
  font-weight: 600;
}

.local-list {
  margin-top: 0.8rem;
}

.local-item {
  border: 1px solid var(--border-subtle);
  background: var(--surface-2);
  color: var(--text-primary);
  border-radius: var(--radius-card-sm);
  padding: 0.85rem 0.95rem;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.2rem;
  cursor: pointer;
}

.local-item.active {
  border-color: var(--accent-border);
  background: var(--surface-accent);
}

.local-item span,
.local-item small {
  color: var(--text-secondary);
}

.preview-shell,
.preview-body,
.preview-remote-gallery {
  display: flex;
  flex-direction: column;
  gap: 0.8rem;
}

.preview-frame {
  width: 100%;
  min-height: 420px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: var(--surface-3);
}

.preview-image,
.preview-remote-image {
  width: 100%;
  border-radius: var(--radius-card-sm);
  border: 1px solid var(--border-subtle);
  object-fit: contain;
  background: var(--surface-3);
}

.preview-text,
.ai-markdown {
  white-space: pre-wrap;
  word-break: break-word;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: var(--surface-2);
  padding: 0.95rem;
  color: var(--text-primary);
  max-height: 420px;
  overflow: auto;
}

@media (max-width: 980px) {
  .materials-layout {
    grid-template-columns: 1fr;
  }

  .materials-stats {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 720px) {
  .manual-form,
  .materials-stats {
    grid-template-columns: 1fr;
  }

  .remote-item {
    grid-template-columns: 1fr;
  }
}
</style>
