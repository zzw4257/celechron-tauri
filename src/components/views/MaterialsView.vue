<script setup lang="ts">
import { convertFileSrc } from '@tauri-apps/api/core';
import { computed, onMounted, ref, watch } from 'vue';
import ActionPill from '../ui/ActionPill.vue';
import InlineStat from '../ui/InlineStat.vue';
import SectionCard from '../ui/SectionCard.vue';
import SegmentedFilter from '../ui/SegmentedFilter.vue';
import StatusBanner from '../ui/StatusBanner.vue';
import type { MaterialAsset, MaterialsPayload, MaterialSourceSummary, RemoteMaterialAsset } from '../../types/api';
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
const defaultScope = ref<'current-week' | 'current-term' | 'all'>('current-week');
const sourcePriority = ref<string[]>(['classroom', 'activity', 'homework']);
const sourceSummaries = ref<MaterialSourceSummary[]>([]);
const courseFilters = ref<{ id: string; label: string; count: number }[]>([]);
const selectedScope = ref<'current-week' | 'current-term' | 'all'>('current-week');
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

const sourceDescriptionMap: Record<string, string> = {
  classroom: '优先展示本周课堂讲稿与 PPT 预览，贴近 zju-learning-assistant 的进入逻辑。',
  activity: '课程活动附件只保留当前学期窗口，避免旧学期内容一股脑混进来。',
  homework: '作业附件作为次级来源，适合补找老师随作业下发的讲义与材料。',
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

function sourceDescription(type: string) {
  return sourceDescriptionMap[type] || '当前来源资料会按时间与课程过滤后展示。';
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
  const age = Math.floor(Date.now() / 1000) - payload.lastSyncedAt;
  if (age >= 6 * 3600) return true;
  return !payload.remoteItems.some((item) => item.sourceType === 'classroom');
}

function hydrate(payload: MaterialsPayload, resetFilters = false) {
  items.value = Array.isArray(payload.items) ? payload.items : [];
  remoteItems.value = Array.isArray(payload.remoteItems) ? payload.remoteItems : [];
  warnings.value = Array.isArray(payload.warnings) ? payload.warnings : [];
  lastSyncedAt.value = typeof payload.lastSyncedAt === 'number' ? payload.lastSyncedAt : null;
  weekLabel.value = payload.weekLabel || '';
  defaultScope.value = payload.defaultScope === 'all' ? 'all' : payload.defaultScope === 'current-term' ? 'current-term' : 'current-week';
  sourcePriority.value = Array.isArray(payload.sourcePriority) && payload.sourcePriority.length
    ? payload.sourcePriority
    : ['classroom', 'activity', 'homework'];
  sourceSummaries.value = Array.isArray(payload.sourceSummaries) ? payload.sourceSummaries : [];
  courseFilters.value = Array.isArray(payload.courseFilters) ? payload.courseFilters : [];

  const availableSources = new Set(remoteItems.value.map((item) => item.sourceType));
  const defaultSource = availableSources.has('classroom')
    ? 'classroom'
    : sourcePriority.value.find((item) => availableSources.has(item)) || 'all';

  if (resetFilters || !['current-week', 'current-term', 'all'].includes(selectedScope.value)) {
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

const sourceSummaryMap = computed(() => new Map(sourceSummaries.value.map((item) => [item.sourceType, item])));

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
    .filter((item) => selectedScope.value === 'all' || (selectedScope.value === 'current-week'
      ? item.weekBucket === 'current'
      : item.weekBucket === 'current' || item.weekBucket === 'other'))
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

const remoteSourceSections = computed(() => {
  const grouped = new Map<string, RemoteMaterialAsset[]>();
  for (const item of visibleRemoteItems.value) {
    const bucket = grouped.get(item.sourceType) || [];
    bucket.push(item);
    grouped.set(item.sourceType, bucket);
  }

  return [...grouped.entries()]
    .sort((left, right) => sourceRank(left[0]) - sourceRank(right[0]) || sourceLabel(left[0]).localeCompare(sourceLabel(right[0])))
    .map(([sourceType, entries]) => ({
      sourceType,
      sourceLabel: sourceLabel(sourceType),
      sourceDescription: sourceDescription(sourceType),
      currentCount: entries.filter((item) => item.weekBucket === 'current').length,
      downloadedCount: entries.filter((item) => item.downloaded).length,
      courseCount: new Set(entries.map((item) => item.courseName)).size,
      entries: [...entries].sort((left, right) => {
        return (left.weekBucket === 'current' ? -1 : 1) - (right.weekBucket === 'current' ? -1 : 1)
          || right.updatedAt - left.updatedAt
          || left.courseName.localeCompare(right.courseName)
          || left.title.localeCompare(right.title);
      }),
    }));
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
  currentTerm: remoteItems.value.filter((item) => item.weekBucket === 'current' || item.weekBucket === 'other').length,
  classroom: remoteItems.value.filter((item) => item.sourceType === 'classroom').length,
  cached: items.value.length,
  remote: remoteItems.value.length,
}));

const classroomMissing = computed(() => summaryStats.value.classroom === 0);
const scopeSubtitle = computed(() => {
  if (selectedScope.value === 'current-week') return '优先显示本周课堂资料与本周新附件，默认先看智云课堂。';
  if (selectedScope.value === 'current-term') return '仅展示当前学期窗口内的远程资料，并维持智云课堂优先。';
  return '这里只展开当前账号已同步过的个人资料，不代表平台公共全量。';
});

const emptyRemoteHint = computed(() => {
  if (selectedSource.value === 'classroom') {
    return '当前筛选下没有智云课堂资料。可先同步当前周，或切到“本学期 / 全部个人”查看已经归档到当前账号的课堂资料。';
  }
  if (selectedScope.value === 'current-week') {
    return '当前筛选下没有本周资料。先同步当前周，或切到“本学期 / 全部个人”查看更完整的个人资料。';
  }
  return '当前筛选下没有远程资料。先点“同步并刷新资料”，或调整来源、课程和搜索条件。';
});

const warningSummary = computed(() => {
  if (warnings.value.length === 0) return '';
  if (classroomMissing.value && warnings.value.some((item) => item.includes('智云课堂'))) {
    return '智云课堂本次未成功返回资料，当前先展示学在浙大课程活动与作业附件；稍后会继续自动重试。';
  }
  if (warnings.value.length === 1) return warnings.value[0];
  return `已合并 ${warnings.value.length} 条同步提示，资料仍按当前学期与你已选课程优先展示。`;
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
      aiError.value = 'AI 网关已响应，但没有返回可展示内容。';
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
        <p class="page-subtitle">默认按本周与本学期收口；切到“全部个人”时，也只展开当前账号已同步过的个人资料，不是平台公共全量。</p>
      </div>
      <div class="materials-header-actions">
        <ActionPill tone="accent" :disabled="isSyncing" @click="syncRemote({ resetFilters: false })">
          {{ isSyncing ? '同步中…' : '同步并刷新资料' }}
        </ActionPill>
        <ActionPill @click="navigateToSettings">抓取规则</ActionPill>
        <ActionPill @click="showAddForm = !showAddForm">{{ showAddForm ? '收起添加' : '手动添加' }}</ActionPill>
      </div>
    </header>

    <StatusBanner v-if="errorMsg" tone="danger" title="资料异常">
      {{ errorMsg }}
    </StatusBanner>
    <StatusBanner v-else-if="warningSummary" tone="warning" title="同步提示">
      {{ warningSummary }}
    </StatusBanner>
    <StatusBanner v-else-if="classroomMissing" tone="info" title="智云课堂暂缺">
      当前只显示学在浙大课程活动与作业附件；“全部个人”也只会展开当前账号已经同步进索引的个人资料。
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
        <InlineStat label="本学期资料" :value="String(summaryStats.currentTerm)" hint="已过滤旧学期课程活动" />
        <InlineStat label="已缓存" :value="String(summaryStats.cached)" hint="本地可离线" />
        <InlineStat label="全部个人" :value="String(summaryStats.remote)" hint="当前账号个人资料" />
      </div>

      <SectionCard dense title="筛选与范围" subtitle="减少一股脑堆叠，先看本周、再按课程收束。">
        <div class="filters-layout">
          <span class="helper-text filters-note">智云课堂优先按本周入场；课程活动与作业附件默认只保留当前学期窗口；“全部个人” 也只代表当前账号自己的同步资料，不是平台公共全量。</span>
          <SegmentedFilter v-model="selectedScope" :options="[
            { value: 'current-week', label: weekLabel ? `本周 ${weekLabel}` : '本周' },
            { value: 'current-term', label: '本学期' },
            { value: 'all', label: '全部个人' },
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
          <SectionCard title="远程资料" :subtitle="scopeSubtitle">
            <div v-if="remoteSourceSections.length === 0" class="state-card">
              {{ emptyRemoteHint }}
            </div>

            <div v-else class="course-groups source-groups">
              <article v-for="section in remoteSourceSections" :key="section.sourceType" class="course-group source-group">
                <header class="course-group__header source-group__header">
                  <div>
                    <div class="source-group__title-row">
                      <h3>{{ section.sourceLabel }}</h3>
                      <span class="badge accent">{{ section.entries.length }} 项</span>
                    </div>
                    <p>{{ section.sourceDescription }}</p>
                    <small v-if="sourceSummaryMap.get(section.sourceType)?.warning" class="source-group__warning">{{ sourceSummaryMap.get(section.sourceType)?.warning }}</small>
                  </div>
                  <div class="source-group__stats">
                    <span class="badge" :class="section.currentCount ? 'success' : ''">{{ section.currentCount }} 本周</span>
                    <span class="badge">{{ section.courseCount }} 门课</span>
                    <span class="badge">{{ section.downloadedCount }} 已缓存</span>
                  </div>
                </header>

                <div class="course-group__list source-group__list">
                  <div
                    v-for="item in section.entries"
                    :key="item.id"
                    class="remote-row"
                    :class="{ active: selectedRemoteId === item.id }"
                  >
                    <button type="button" class="remote-row__select" @click="selectRemoteItem(item)">
                      <div class="remote-row__headline">
                        <div class="remote-row__title-line">
                          <strong>{{ item.title }}</strong>
                          <div class="remote-row__badges">
                            <span class="badge" :class="item.weekBucket === 'current' ? 'success' : ''">{{ weekBucketLabel(item.weekBucket) }}</span>
                            <span v-if="item.downloaded" class="badge success">已缓存</span>
                          </div>
                        </div>
                        <p>{{ item.courseName }}</p>
                      </div>
                      <small>{{ item.fileName }} · {{ formatTime(item.updatedAt) }}</small>
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

.filters-note {
  flex: 1 1 100%;
  display: block;
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
.preview-gallery,
.source-group__stats,
.source-group__title-row {
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
}

.course-group {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: linear-gradient(180deg, color-mix(in srgb, var(--surface-2) 90%, white) 0%, var(--surface-1) 100%);
  padding: 0.9rem;
}

.source-groups {
  gap: 1rem;
}

.source-group {
  padding: 1rem;
}

.course-group__header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.75rem;
  padding-bottom: 0.7rem;
  border-bottom: 1px solid var(--border-subtle);
}

.source-group__header {
  gap: 1rem;
}

.source-group__title-row {
  flex-direction: row;
  align-items: center;
  gap: 0.55rem;
}

.source-group__stats {
  align-items: center;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 0.45rem;
}

.source-group__warning {
  display: block;
  margin-top: 0.35rem;
  color: var(--warning-text);
}

.course-group__header h3 {
  margin: 0;
  font-size: 0.98rem;
  color: var(--text-primary);
}

.course-group__header p,
.remote-row__select p,
.local-row__select p,
.helper-text,
.source-group__warning {
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
  gap: 0.42rem;
}

.remote-row__title-line {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.7rem;
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

  .source-group__header,
  .remote-row__title-line {
    flex-direction: column;
    align-items: flex-start;
  }

  .remote-row,
  .local-row {
    grid-template-columns: 1fr;
  }

  .source-group__stats,
  .remote-row__badges,
  .remote-row__actions,
  .local-row__actions {
    justify-content: flex-start;
  }

  .remote-row__actions,
  .local-row__actions {
    padding: 0 0.8rem 0.8rem;
  }
}
</style>
