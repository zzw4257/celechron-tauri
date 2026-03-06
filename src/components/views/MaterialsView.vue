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

type PreviewMode = 'image' | 'pdf' | 'web' | 'text' | 'none';

const { accountScope, zeroClawApiKey, zeroClawEndpoint } = usePreferences();

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
const autoSyncedOnce = ref(false);
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

function formatTime(ts?: number | null) {
  if (!ts) return '未同步';
  return new Date(ts * 1000).toLocaleString('zh-CN', { hour12: false });
}

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function sourceLabel(type: string) {
  return sourceLabelMap[type] || type;
}

function weekBucketLabel(value: string) {
  if (value === 'current') return '本周';
  if (value === 'other') return '其他周';
  return '未分周';
}

function sourceRank(type: string) {
  const index = sourcePriority.value.indexOf(type);
  return index === -1 ? sourcePriority.value.length : index;
}

function shouldAutoSync(payload: MaterialsPayload) {
  if (!payload.lastSyncedAt) return true;
  if (!Array.isArray(payload.remoteItems) || payload.remoteItems.length === 0) return true;
  return !payload.remoteItems.some((item) => item.sourceType === 'classroom');
}

function hydrate(payload: MaterialsPayload, resetFilters = false) {
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

  const availableSources = new Set(remoteItems.value.map((item) => item.sourceType));
  const defaultSource = availableSources.has('classroom')
    ? 'classroom'
    : sourcePriority.value.find((item) => availableSources.has(item)) || 'all';

  if (resetFilters || !['current-week', 'all'].includes(selectedScope.value)) {
    selectedScope.value = defaultScope.value;
  }
  if (resetFilters || (selectedSource.value !== 'all' && !availableSources.has(selectedSource.value))) {
    selectedSource.value = defaultSource;
  }
  if (resetFilters || !courseFilters.value.some((item) => item.id === selectedCourse.value)) {
    selectedCourse.value = 'all';
  }
  if (resetFilters) {
    search.value = '';
    aiMarkdown.value = '';
    aiError.value = '';
  }

  if (!selectedRemoteId.value || !remoteItems.value.some((item) => item.id === selectedRemoteId.value)) {
    selectedRemoteId.value = remoteItems.value[0]?.id || '';
  }
  if (!selectedLocalPath.value || !items.value.some((item) => item.relativePath === selectedLocalPath.value)) {
    const selectedRemote = remoteItems.value.find((item) => item.id === selectedRemoteId.value);
    selectedLocalPath.value = selectedRemote?.downloaded && selectedRemote.localRelativePath
      ? selectedRemote.localRelativePath
      : (remoteItems.value.length === 0 ? items.value[0]?.relativePath || '' : '');
  }
}

const sourceOptions = computed(() => {
  const counts = new Map<string, number>();
  for (const item of remoteItems.value) {
    counts.set(item.sourceType, (counts.get(item.sourceType) || 0) + 1);
  }
  const options = [{ value: 'all', label: '全部来源', badge: remoteItems.value.length }];
  for (const source of sourcePriority.value) {
    const count = counts.get(source);
    if (!count) continue;
    options.push({ value: source, label: sourceLabel(source), badge: count });
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
      return [item.courseName, item.title, item.fileName, sourceLabel(item.sourceType)]
        .join(' ')
        .toLowerCase()
        .includes(keyword);
    })
    .sort((left, right) => {
      return sourceRank(left.sourceType) - sourceRank(right.sourceType)
        || (left.weekBucket === 'current' ? -1 : 1) - (right.weekBucket === 'current' ? -1 : 1)
        || right.updatedAt - left.updatedAt
        || left.courseName.localeCompare(right.courseName);
    });
});

const remoteCourseGroups = computed(() => {
  const grouped = new Map<string, RemoteMaterialAsset[]>();
  for (const item of visibleRemoteItems.value) {
    const bucket = grouped.get(item.courseName) || [];
    bucket.push(item);
    grouped.set(item.courseName, bucket);
  }

  return [...grouped.entries()]
    .map(([courseName, entries]) => ({
      courseName,
      currentCount: entries.filter((item) => item.weekBucket === 'current').length,
      downloadedCount: entries.filter((item) => item.downloaded).length,
      entries,
    }))
    .sort((left, right) => right.currentCount - left.currentCount || right.entries.length - left.entries.length || left.courseName.localeCompare(right.courseName));
});

const visibleLocalItems = computed(() => {
  const keyword = search.value.trim().toLowerCase();
  return [...items.value]
    .filter((item) => selectedCourse.value === 'all' || item.courseName === selectedCourse.value)
    .filter((item) => {
      if (!keyword) return true;
      return [item.courseName, item.title, item.fileName].join(' ').toLowerCase().includes(keyword);
    })
    .sort((left, right) => right.updatedAt - left.updatedAt || left.courseName.localeCompare(right.courseName));
});

const selectedRemoteItem = computed(() => remoteItems.value.find((item) => item.id === selectedRemoteId.value) || null);

const selectedPreviewAsset = computed(() => {
  if (selectedLocalPath.value) {
    const direct = items.value.find((item) => item.relativePath === selectedLocalPath.value);
    if (direct) return direct;
  }

  const remote = selectedRemoteItem.value;
  if (remote?.downloaded && remote.localRelativePath) {
    return items.value.find((item) => item.relativePath === remote.localRelativePath) || null;
  }

  return null;
});

const previewImageUrls = computed(() => selectedRemoteItem.value?.previewImageUrls?.slice(0, 8) || []);

const previewMode = computed<PreviewMode>(() => {
  const asset = selectedPreviewAsset.value;
  if (!asset) return 'none';

  const mime = String(asset.mimeType || '').toLowerCase();
  const path = asset.fileName.toLowerCase();
  if (mime.startsWith('image/') || /\.(png|jpe?g|gif|webp|bmp|svg)$/.test(path)) return 'image';
  if (mime.includes('pdf') || path.endsWith('.pdf')) return 'pdf';
  if (mime.includes('html') || /\.(html|htm)$/.test(path)) return 'web';
  if (mime.startsWith('text/') || /\.(txt|md|markdown|json|csv|tsv|log|ya?ml|xml|js|ts|py|rs)$/.test(path)) return 'text';
  return 'none';
});

const previewUrl = computed(() => {
  if (!selectedPreviewAsset.value) return '';
  return convertFileSrc(selectedPreviewAsset.value.absolutePath);
});

const summaryStats = computed(() => ({
  currentWeek: remoteItems.value.filter((item) => item.weekBucket === 'current').length,
  classroom: remoteItems.value.filter((item) => item.sourceType === 'classroom').length,
  cached: items.value.length,
  remote: remoteItems.value.length,
}));

const warningSummary = computed(() => {
  if (warnings.value.length === 0) return '';
  if (warnings.value.length === 1) return warnings.value[0];
  return `已合并 ${warnings.value.length} 条同步提示，优先保留可用资料结果。`;
});

const activePreviewTitle = computed(() => {
  if (selectedPreviewAsset.value) return selectedPreviewAsset.value.title;
  if (selectedRemoteItem.value) return selectedRemoteItem.value.title;
  return '预览';
});

const activePreviewMeta = computed(() => {
  if (selectedPreviewAsset.value) {
    return `${selectedPreviewAsset.value.courseName} · ${selectedPreviewAsset.value.fileName}`;
  }
  if (selectedRemoteItem.value) {
    return `${selectedRemoteItem.value.courseName} · ${sourceLabel(selectedRemoteItem.value.sourceType)}`;
  }
  return '选择一份资料后，在这里查看缓存内容或远程预览。';
});

async function loadPreviewText() {
  previewText.value = '';
  previewTruncated.value = false;
  const asset = selectedPreviewAsset.value;
  if (!asset || previewMode.value !== 'text') return;

  previewLoading.value = true;
  try {
    const env = await readMaterialText(asset.relativePath, 30000);
    previewText.value = env.data.content || '';
    previewTruncated.value = Boolean(env.data.truncated);
  } catch (error: any) {
    previewText.value = error?.message || String(error);
  } finally {
    previewLoading.value = false;
  }
}

async function loadMaterials(options?: { resetFilters?: boolean; autoSync?: boolean }) {
  isLoading.value = true;
  errorMsg.value = '';
  actionStatus.value = '';

  try {
    const env = await fetchMaterials();
    hydrate(env.data, Boolean(options?.resetFilters));

    if (options?.autoSync !== false && !autoSyncedOnce.value && shouldAutoSync(env.data)) {
      autoSyncedOnce.value = true;
      await syncRemote({ silent: true, resetFilters: Boolean(options?.resetFilters) });
    }
  } catch (error: any) {
    errorMsg.value = error?.message || String(error);
  } finally {
    isLoading.value = false;
  }
}

async function syncRemote(options?: { silent?: boolean; resetFilters?: boolean }) {
  isSyncing.value = true;
  errorMsg.value = '';
  if (!options?.silent) {
    actionStatus.value = '';
  }

  try {
    const env = await syncMaterialsIndex();
    hydrate(env.data, Boolean(options?.resetFilters));
    if (!options?.silent) {
      actionStatus.value = '资料索引已刷新。';
    }
  } catch (error: any) {
    errorMsg.value = error?.message || String(error);
  } finally {
    isSyncing.value = false;
  }
}

function selectRemoteItem(item: RemoteMaterialAsset) {
  selectedRemoteId.value = item.id;
  selectedLocalPath.value = item.downloaded && item.localRelativePath ? item.localRelativePath : '';
}

function selectLocalItem(item: MaterialAsset) {
  selectedLocalPath.value = item.relativePath;
  const linkedRemote = remoteItems.value.find((remote) => remote.localRelativePath === item.relativePath);
  if (linkedRemote) {
    selectedRemoteId.value = linkedRemote.id;
  }
}

async function cacheItem(item: RemoteMaterialAsset) {
  isSubmitting.value = true;
  errorMsg.value = '';
  actionStatus.value = '';

  try {
    const env = await cacheRemoteMaterial({ remoteId: item.id });
    hydrate(env.data, false);
    selectedRemoteId.value = item.id;
    const refreshed = remoteItems.value.find((remote) => remote.id === item.id);
    selectedLocalPath.value = refreshed?.localRelativePath || selectedLocalPath.value;
    actionStatus.value = '资料已缓存到本地。';
  } catch (error: any) {
    errorMsg.value = error?.message || String(error);
  } finally {
    isSubmitting.value = false;
  }
}

async function openCachedAsset(relativePath: string) {
  try {
    await openMaterialAsset(relativePath);
  } catch (error: any) {
    errorMsg.value = error?.message || String(error);
  }
}

async function removeCachedAsset(relativePath: string) {
  isSubmitting.value = true;
  errorMsg.value = '';
  actionStatus.value = '';

  try {
    await removeMaterialCache(relativePath);
    if (selectedLocalPath.value === relativePath) {
      selectedLocalPath.value = '';
    }
    await loadMaterials({ autoSync: false });
    actionStatus.value = '本地缓存已移除。';
  } catch (error: any) {
    errorMsg.value = error?.message || String(error);
  } finally {
    isSubmitting.value = false;
  }
}

async function addRemoteMaterial() {
  const url = draft.value.url.trim();
  if (!url) return;

  isSubmitting.value = true;
  errorMsg.value = '';
  actionStatus.value = '';

  try {
    await downloadMaterialAsset({
      url,
      courseName: draft.value.courseName.trim() || '手动添加',
      title: draft.value.title.trim() || draft.value.fileName.trim() || '外部资料',
      fileName: draft.value.fileName.trim() || undefined,
      source: 'manual',
    });
    draft.value = { courseName: '', title: '', url: '', fileName: '' };
    showAddForm.value = false;
    await loadMaterials({ autoSync: false });
    actionStatus.value = '资料已添加到本地缓存。';
  } catch (error: any) {
    errorMsg.value = error?.message || String(error);
  } finally {
    isSubmitting.value = false;
  }
}

async function analyzeSelected() {
  if (!zeroClawEndpoint.value) {
    navigateToSettings();
    return;
  }

  const asset = selectedPreviewAsset.value;
  const remote = selectedRemoteItem.value;
  if (!asset && !remote) {
    aiError.value = '请先选择一份资料。';
    return;
  }

  aiLoading.value = true;
  aiError.value = '';
  aiMarkdown.value = '';

  try {
    const env = await runAiAnalysis({
      baseUrl: zeroClawEndpoint.value,
      apiKey: zeroClawApiKey.value || undefined,
      prompt: '请基于资料标题、课程上下文与可用正文输出中文学习摘要，包含重点、难点和复习建议。',
      context: {
        material: asset ? {
          courseName: asset.courseName,
          title: asset.title,
          fileName: asset.fileName,
          mimeType: asset.mimeType,
          content: previewMode.value === 'text' ? previewText.value.slice(0, 16000) : undefined,
        } : null,
        remoteMaterial: remote ? {
          courseName: remote.courseName,
          title: remote.title,
          fileName: remote.fileName,
          sourceType: remote.sourceType,
          previewImageCount: remote.previewImageUrls?.length || 0,
        } : null,
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

watch(visibleRemoteItems, (value) => {
  if (!value.some((item) => item.id === selectedRemoteId.value)) {
    const next = value[0] || null;
    selectedRemoteId.value = next?.id || '';
    selectedLocalPath.value = next?.downloaded && next.localRelativePath
      ? next.localRelativePath
      : (value.length === 0 ? visibleLocalItems.value[0]?.relativePath || '' : '');
  }
}, { immediate: true });

watch(visibleLocalItems, (value) => {
  if (!value.some((item) => item.relativePath === selectedLocalPath.value)) {
    const linkedRemote = visibleRemoteItems.value.find((item) => item.id === selectedRemoteId.value);
    selectedLocalPath.value = linkedRemote?.downloaded && linkedRemote.localRelativePath
      ? linkedRemote.localRelativePath
      : (visibleRemoteItems.value.length === 0 ? value[0]?.relativePath || '' : '');
  }
}, { immediate: true });

watch([selectedRemoteId, selectedLocalPath], () => {
  aiMarkdown.value = '';
  aiError.value = '';
});

watch(() => selectedPreviewAsset.value?.relativePath, loadPreviewText, { immediate: true });

watch(accountScope, () => {
  autoSyncedOnce.value = false;
  loadMaterials({ resetFilters: true, autoSync: true });
});

onMounted(() => {
  loadMaterials({ resetFilters: true, autoSync: true });
});
</script>

<template>
  <div class="page-shell materials-view">
    <header class="page-header">
      <div>
        <h1>资料</h1>
        <p class="page-subtitle">默认按本周与智云课堂优先收口，远程资料按课程分组展示。</p>
      </div>
      <div class="materials-header-actions">
        <ActionPill tone="accent" :disabled="isSyncing" @click="syncRemote({ resetFilters: false })">
          {{ isSyncing ? '同步中…' : '同步远程资料' }}
        </ActionPill>
        <ActionPill @click="showAddForm = !showAddForm">{{ showAddForm ? '收起添加' : '手动添加' }}</ActionPill>
      </div>
    </header>

    <StatusBanner v-if="errorMsg" tone="danger" title="资料异常">
      {{ errorMsg }}
    </StatusBanner>
    <StatusBanner v-else-if="warningSummary" tone="warning" title="同步提示">
      {{ warningSummary }}
    </StatusBanner>
    <StatusBanner v-if="actionStatus" tone="success" title="完成">
      {{ actionStatus }}
    </StatusBanner>

    <SectionCard v-if="isLoading" title="加载中" subtitle="正在整理远程资料与本地缓存。">
      <div class="state-card">请稍候，正在拉取资料索引。</div>
    </SectionCard>

    <template v-else>
      <div class="materials-stats">
        <InlineStat label="本周可看" :value="String(summaryStats.currentWeek)" :hint="weekLabel || '未分周'" emphasis />
        <InlineStat label="智云课堂" :value="String(summaryStats.classroom)" hint="优先来源" />
        <InlineStat label="已缓存" :value="String(summaryStats.cached)" hint="本地可离线" />
        <InlineStat label="远程总数" :value="String(summaryStats.remote)" :hint="formatTime(lastSyncedAt)" />
      </div>

      <SectionCard dense title="筛选与范围" subtitle="减少一股脑堆叠，先看本周、再按课程收束。">
        <div class="filters-layout">
          <SegmentedFilter v-model="selectedScope" :options="[
            { value: 'current-week', label: weekLabel ? `本周 ${weekLabel}` : '本周' },
            { value: 'all', label: '全部' },
          ]" />
          <SegmentedFilter v-model="selectedSource" :options="sourceOptions" />
          <select v-model="selectedCourse" class="select-field filters-select">
            <option v-for="item in courseOptions" :key="item.value" :value="item.value">{{ item.label }} ({{ item.badge }})</option>
          </select>
          <input v-model.trim="search" class="input-field filters-search" type="search" placeholder="搜索课程名、标题或文件名" />
        </div>

        <div v-if="showAddForm" class="manual-form">
          <input v-model.trim="draft.courseName" class="input-field" placeholder="课程名（可选）" />
          <input v-model.trim="draft.title" class="input-field" placeholder="标题（可选）" />
          <input v-model.trim="draft.fileName" class="input-field" placeholder="文件名（可选）" />
          <input v-model.trim="draft.url" class="input-field manual-form__wide" placeholder="资料链接" />
          <div class="manual-form__actions">
            <ActionPill tone="accent" :disabled="isSubmitting" @click="addRemoteMaterial">
              {{ isSubmitting ? '保存中…' : '保存到本地缓存' }}
            </ActionPill>
          </div>
        </div>
      </SectionCard>

      <div class="materials-layout">
        <div class="materials-main">
          <SectionCard title="远程资料" :subtitle="selectedScope === 'current-week' ? '优先显示当前周条目；本地缓存区下沉到次级。' : '按课程分组查看全部远程资料。'">
            <div v-if="remoteCourseGroups.length === 0" class="state-card">
              当前筛选下没有远程资料。先点“同步远程资料”，或切换到全部范围查看。
            </div>

            <div v-else class="course-groups">
              <article v-for="group in remoteCourseGroups" :key="group.courseName" class="course-group">
                <header class="course-group__header">
                  <div>
                    <h3>{{ group.courseName }}</h3>
                    <p>{{ group.currentCount }} 份本周可看 · {{ group.downloadedCount }} 份已缓存</p>
                  </div>
                  <span class="badge">{{ group.entries.length }} 项</span>
                </header>

                <div class="course-group__list">
                  <div
                    v-for="item in group.entries"
                    :key="item.id"
                    class="remote-row"
                    :class="{ active: selectedRemoteId === item.id }"
                  >
                    <button type="button" class="remote-row__select" @click="selectRemoteItem(item)">
                      <div class="remote-row__headline">
                        <strong>{{ item.title }}</strong>
                        <div class="remote-row__badges">
                          <span class="badge accent">{{ sourceLabel(item.sourceType) }}</span>
                          <span class="badge" :class="item.weekBucket === 'current' ? 'success' : ''">{{ weekBucketLabel(item.weekBucket) }}</span>
                          <span v-if="item.downloaded" class="badge success">已缓存</span>
                        </div>
                      </div>
                      <p>{{ item.fileName }}</p>
                      <small>{{ formatTime(item.updatedAt) }}</small>
                    </button>

                    <div class="remote-row__actions">
                      <button
                        v-if="!item.downloaded"
                        type="button"
                        class="mini-action"
                        :disabled="isSubmitting"
                        @click.stop="cacheItem(item)"
                      >
                        缓存
                      </button>
                      <button
                        v-else-if="item.localRelativePath"
                        type="button"
                        class="mini-action"
                        @click.stop="openCachedAsset(item.localRelativePath)"
                      >
                        打开
                      </button>
                    </div>
                  </div>
                </div>
              </article>
            </div>
          </SectionCard>

          <SectionCard title="本地缓存" subtitle="作为次级区保留，供离线阅读与快速打开。" dense>
            <div v-if="visibleLocalItems.length === 0" class="state-card">当前没有匹配的本地缓存。</div>
            <div v-else class="local-list">
              <div
                v-for="item in visibleLocalItems"
                :key="item.relativePath"
                class="local-row"
                :class="{ active: selectedPreviewAsset?.relativePath === item.relativePath }"
              >
                <button type="button" class="local-row__select" @click="selectLocalItem(item)">
                  <strong>{{ item.title }}</strong>
                  <p>{{ item.courseName }} · {{ item.fileName }}</p>
                  <small>{{ formatBytes(item.sizeBytes) }} · {{ formatTime(item.updatedAt) }}</small>
                </button>
                <div class="local-row__actions">
                  <button type="button" class="mini-action" @click.stop="openCachedAsset(item.relativePath)">打开</button>
                  <button type="button" class="mini-action danger" :disabled="isSubmitting" @click.stop="removeCachedAsset(item.relativePath)">移除</button>
                </div>
              </div>
            </div>
          </SectionCard>
        </div>

        <div class="materials-side">
          <SectionCard :title="activePreviewTitle" :subtitle="activePreviewMeta">
            <template #header>
              <div class="preview-actions">
                <ActionPill
                  v-if="selectedRemoteItem && !selectedRemoteItem.downloaded"
                  tone="accent"
                  :disabled="isSubmitting"
                  @click="cacheItem(selectedRemoteItem)"
                >
                  {{ isSubmitting ? '缓存中…' : '缓存到本地' }}
                </ActionPill>
                <ActionPill
                  v-if="selectedPreviewAsset"
                  :disabled="isSubmitting"
                  @click="openCachedAsset(selectedPreviewAsset.relativePath)"
                >
                  外部打开
                </ActionPill>
                <ActionPill
                  v-if="selectedPreviewAsset"
                  tone="danger"
                  :disabled="isSubmitting"
                  @click="removeCachedAsset(selectedPreviewAsset.relativePath)"
                >
                  移除缓存
                </ActionPill>
                <ActionPill
                  v-if="selectedPreviewAsset || selectedRemoteItem"
                  :tone="zeroClawEndpoint ? 'accent' : 'warning'"
                  :disabled="aiLoading"
                  @click="analyzeSelected"
                >
                  {{ zeroClawEndpoint ? (aiLoading ? '分析中…' : 'AI 摘要') : '去设置' }}
                </ActionPill>
              </div>
            </template>

            <div v-if="selectedPreviewAsset && previewMode === 'image'" class="preview-body">
              <img :src="previewUrl" class="preview-image" alt="资料预览" />
            </div>
            <div v-else-if="selectedPreviewAsset && (previewMode === 'pdf' || previewMode === 'web')" class="preview-body">
              <iframe :src="previewUrl" class="preview-frame" title="资料预览"></iframe>
            </div>
            <div v-else-if="selectedPreviewAsset && previewMode === 'text'" class="preview-body">
              <div v-if="previewLoading" class="state-card">正在读取文本内容…</div>
              <pre v-else class="preview-text">{{ previewText || '文本内容为空。' }}</pre>
              <p v-if="previewTruncated" class="helper-text">文本过长，当前仅展示前 30000 个字符。</p>
            </div>
            <div v-else-if="selectedRemoteItem && previewImageUrls.length" class="preview-gallery">
              <p class="helper-text">该条目尚未缓存，先展示可用预览图。</p>
              <img v-for="url in previewImageUrls" :key="url" :src="url" class="preview-remote-image" alt="远程预览图" />
            </div>
            <div v-else-if="selectedRemoteItem" class="state-card">该远程资料暂无可用预览，缓存后可直接在本地打开。</div>
            <div v-else class="state-card">从左侧选择资料后，这里会展示预览与后续动作。</div>

            <StatusBanner v-if="aiError" tone="warning" title="AI 提示">
              {{ aiError }}
            </StatusBanner>
            <article v-if="aiMarkdown" class="ai-markdown">{{ aiMarkdown }}</article>
          </SectionCard>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.materials-view {
  gap: 1rem;
}

.materials-header-actions,
.preview-actions,
.filters-layout {
  display: flex;
  flex-wrap: wrap;
  gap: 0.7rem;
  align-items: center;
}

.materials-stats {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 0.75rem;
}

.filters-layout {
  align-items: stretch;
}

.filters-select {
  width: min(100%, 220px);
}

.filters-search {
  flex: 1;
  min-width: 220px;
}

.manual-form {
  margin-top: 0.8rem;
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 0.7rem;
}

.manual-form__wide,
.manual-form__actions {
  grid-column: 1 / -1;
}

.materials-layout {
  display: grid;
  grid-template-columns: minmax(0, 1.2fr) minmax(320px, 0.8fr);
  gap: 1rem;
  align-items: start;
}

.materials-main,
.materials-side,
.course-groups,
.course-group__list,
.local-list,
.preview-body,
.preview-gallery {
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
}

.course-group {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: var(--surface-2);
  padding: 0.9rem;
}

.course-group__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.75rem;
  padding-bottom: 0.7rem;
  border-bottom: 1px solid var(--border-subtle);
}

.course-group__header h3 {
  margin: 0;
  font-size: 0.98rem;
  color: var(--text-primary);
}

.course-group__header p,
.remote-row__select p,
.local-row__select p,
.helper-text {
  margin: 0.2rem 0 0;
  color: var(--text-secondary);
}

.remote-row,
.local-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 0.8rem;
  align-items: center;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: var(--surface-1);
  padding: 0.2rem;
}

.remote-row.active,
.local-row.active {
  border-color: var(--accent-border);
  background: var(--surface-accent);
}

.remote-row__select,
.local-row__select {
  border: none;
  background: transparent;
  text-align: left;
  width: 100%;
  padding: 0.75rem 0.8rem;
  border-radius: calc(var(--radius-card-sm) - 6px);
  color: var(--text-primary);
  cursor: pointer;
}

.remote-row__headline {
  display: flex;
  flex-direction: column;
  gap: 0.45rem;
}

.remote-row__headline strong,
.local-row__select strong {
  color: var(--text-primary);
}

.remote-row__badges,
.remote-row__actions,
.local-row__actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.45rem;
  align-items: center;
  justify-content: flex-end;
}

.remote-row__select small,
.local-row__select small {
  color: var(--text-secondary);
}

.mini-action {
  min-height: 2rem;
  border-radius: var(--radius-pill);
  border: 1px solid var(--border-subtle);
  background: var(--surface-2);
  color: var(--text-primary);
  padding: 0.45rem 0.8rem;
  cursor: pointer;
}

.mini-action.danger {
  color: var(--danger-text);
  border-color: var(--danger-border);
  background: var(--danger-soft);
}

.preview-frame {
  width: 100%;
  min-height: 460px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: var(--surface-3);
}

.preview-image,
.preview-remote-image {
  width: 100%;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: var(--surface-3);
  object-fit: contain;
}

.preview-text,
.ai-markdown {
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 420px;
  overflow: auto;
  margin: 0;
  padding: 0.95rem;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: var(--surface-2);
  color: var(--text-primary);
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
  .materials-stats,
  .manual-form {
    grid-template-columns: 1fr;
  }

  .remote-row,
  .local-row {
    grid-template-columns: 1fr;
  }

  .remote-row__actions,
  .local-row__actions {
    justify-content: flex-start;
    padding: 0 0.8rem 0.8rem;
  }
}
</style>
