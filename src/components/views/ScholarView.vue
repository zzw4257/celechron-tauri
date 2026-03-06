<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue';
import ActionPill from '../ui/ActionPill.vue';
import InlineStat from '../ui/InlineStat.vue';
import SectionCard from '../ui/SectionCard.vue';
import SegmentedFilter from '../ui/SegmentedFilter.vue';
import StatusBanner from '../ui/StatusBanner.vue';
import type { GpaSummary, ScholarPayload, ScholarSemester } from '../../types/api';
import { calculateGpaPreview, fetchScholarData, fetchTodos, runAiAnalysis } from '../../services/api';
import { usePreferences } from '../../composables/usePreferences';
import { formatTermDisplayName } from '../../utils/semester';

const EMPTY_GPA: GpaSummary = {
  fivePoint: 0,
  fourPoint: 0,
  fourPointLegacy: 0,
  hundredPoint: 0,
  totalCredits: 0,
  majorGpa: 0,
  majorGpaLegacy: 0,
  majorCredits: 0,
};

const { accountScope, courseIdMappings, hideGpa, retakePolicy, setRetakePolicy, zeroClawApiKey, zeroClawEndpoint } = usePreferences();

const isLoading = ref(true);
const isOffline = ref(false);
const errorMsg = ref('');
const offlineTime = ref('');
const scholar = ref<ScholarPayload | null>(null);
const selectedSemesterName = ref('');
const upcomingTodoCount = ref(0);
const upcomingExamCount = ref(0);
const aiLoading = ref(false);
const aiError = ref('');
const aiMarkdown = ref('');
const customMode = ref(false);
const customLoading = ref(false);
const customPreview = ref<GpaSummary | null>(null);
const customScores = ref<Record<string, string>>({});

function navigateToSettings() {
  window.dispatchEvent(new CustomEvent('celechron:navigate', { detail: { tab: 'option' } }));
}

function mask(value: number, digits = 2) {
  return hideGpa.value ? '****' : value.toFixed(digits);
}

function numeric(value: unknown, fallback = 0) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

const majorCourseIds = computed(() => new Set(scholar.value?.majorCourseIds || []));

const semesters = computed(() => scholar.value?.semesters || []);
const selectedSemester = computed<ScholarSemester | null>(() => {
  return semesters.value.find((item) => item.name === selectedSemesterName.value) || semesters.value[0] || null;
});

const displayGpa = computed(() => {
  const payload = scholar.value;
  if (!payload) return EMPTY_GPA;
  return payload.gpaByPolicy?.[retakePolicy.value] || payload.gpaByPolicy?.first || EMPTY_GPA;
});

const summaryMetrics = computed(() => [
  { label: '总五分制', value: mask(displayGpa.value.fivePoint), hint: `策略 ${retakePolicy.value}` },
  { label: '总 4.3', value: mask(displayGpa.value.fourPoint) },
  { label: '获得学分', value: displayGpa.value.totalCredits.toFixed(1) },
  { label: '平均分', value: mask(displayGpa.value.hundredPoint) },
  { label: '主修 4.3', value: mask(displayGpa.value.majorGpa), hint: `${displayGpa.value.majorCredits.toFixed(1)} 学分` },
]);

const extraMetrics = computed(() => [
  { label: '总 4.0', value: mask(displayGpa.value.fourPointLegacy) },
  { label: '主修 4.0', value: mask(displayGpa.value.majorGpaLegacy) },
  { label: '未来 7 天任务', value: String(upcomingTodoCount.value) },
  { label: '待参加考试', value: String(upcomingExamCount.value) },
]);

const semesterSummary = computed(() => {
  const current = selectedSemester.value;
  if (!current) return EMPTY_GPA;
  return current.gpaByPolicy?.[retakePolicy.value] || current.gpaByPolicy?.first || EMPTY_GPA;
});

const semesterOptions = computed(() =>
  semesters.value.map((item) => ({
    value: item.name,
    label: item.displayName || formatTermDisplayName(item.term, item.name),
  })),
);

const trendRows = computed(() => {
  return [...semesters.value]
    .reverse()
    .map((item) => {
      const summary = item.gpaByPolicy?.[retakePolicy.value] || item.gpaByPolicy?.first || EMPTY_GPA;
      return {
        key: item.name,
        label: item.displayName || formatTermDisplayName(item.term, item.name),
        fivePoint: summary.fivePoint,
        fourPoint: summary.fourPoint,
        fiveWidth: `${Math.max(10, (summary.fivePoint / 5) * 100)}%`,
        fourWidth: `${Math.max(10, (summary.fourPoint / 4.3) * 100)}%`,
      };
    });
});

const gradeRows = computed(() => {
  const current = selectedSemester.value;
  if (!current) return [];
  return (current.grades || []).map((grade: any) => {
    const xkkh = String(grade.xkkh || '');
    const kcdm = String(grade.kcdm || '');
    const isMajor = majorCourseIds.value.has(xkkh) || majorCourseIds.value.has(kcdm);
    return {
      id: xkkh || `${current.name}-${grade.kcmc}`,
      courseName: String(grade.kcmc || '未命名课程'),
      scoreText: String(grade.cj || '待录'),
      credit: numeric(grade.credit ?? grade.xf),
      fivePoint: numeric(grade.fivePoint),
      fourPoint: numeric(grade.fourPoint),
      isMajor,
      raw: grade,
    };
  });
});

const customRows = computed(() =>
  gradeRows.value.filter((row) => Number.isFinite(numeric(row.raw.hundredPoint, NaN))),
);

async function exportToCsv() {
  const headers = ['学期', '课程名称', '课程代码', '学分', '成绩', '五分', '4.3', '主修'];
  let csv = `data:text/csv;charset=utf-8,\uFEFF${headers.join(',')}\n`;
  for (const semester of semesters.value) {
    const label = semester.displayName || formatTermDisplayName(semester.term, semester.name);
    for (const grade of semester.grades || []) {
      const isMajor = majorCourseIds.value.has(String(grade.xkkh || '')) || majorCourseIds.value.has(String(grade.kcdm || ''));
      csv += [
        label,
        `"${String(grade.kcmc || '')}"`,
        String(grade.kcdm || ''),
        numeric(grade.credit ?? grade.xf).toFixed(1),
        `"${String(grade.cj || '')}"`,
        numeric(grade.fivePoint).toFixed(2),
        numeric(grade.fourPoint).toFixed(2),
        isMajor ? '是' : '否',
      ].join(',');
      csv += '\n';
    }
  }
  const link = document.createElement('a');
  link.href = encodeURI(csv);
  link.download = `celechron-scholar-${Date.now()}.csv`;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
}

async function analyzeScholar() {
  if (!zeroClawEndpoint.value) {
    navigateToSettings();
    return;
  }

  aiLoading.value = true;
  aiError.value = '';
  try {
    const env = await runAiAnalysis({
      baseUrl: zeroClawEndpoint.value,
      apiKey: zeroClawApiKey.value || undefined,
      prompt: '请基于当前成绩、学期趋势、近期考试与待办输出中文学业综合分析。包含风险提示、重点课程、近期行动建议。',
      context: {
        policy: retakePolicy.value,
        summary: displayGpa.value,
        semester: selectedSemester.value,
        exams: scholar.value?.exams || [],
        upcomingTodoCount: upcomingTodoCount.value,
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

async function calculateCustomPreview() {
  if (!selectedSemester.value) return;
  customLoading.value = true;
  try {
    const simulatedScores = Object.fromEntries(
      Object.entries(customScores.value)
        .map(([key, value]) => [key, Number(value)])
        .filter(([, value]) => Number.isFinite(value)),
    );
    customPreview.value = await calculateGpaPreview({
      grades: selectedSemester.value.grades,
      simulatedScores,
      retakePolicy: retakePolicy.value,
      majorCourseIds: scholar.value?.majorCourseIds || [],
      courseIdMappings: courseIdMappings.value,
    });
  } catch (error: any) {
    errorMsg.value = error?.message || String(error);
  } finally {
    customLoading.value = false;
  }
}

async function loadScholar() {
  isLoading.value = true;
  errorMsg.value = '';
  aiMarkdown.value = '';
  aiError.value = '';

  try {
    const [scholarEnv, todoEnv] = await Promise.all([fetchScholarData(), fetchTodos()]);
    scholar.value = scholarEnv.data;
    isOffline.value = scholarEnv._meta?.source === 'cache';
    offlineTime.value = scholarEnv._meta?.timestamp
      ? new Date(scholarEnv._meta.timestamp * 1000).toLocaleString('zh-CN', { hour12: false })
      : '';

    if (!selectedSemesterName.value || !scholarEnv.data.semesters.some((item) => item.name === selectedSemesterName.value)) {
      selectedSemesterName.value = scholarEnv.data.semesters[0]?.name || '';
    }

    const now = Date.now();
    upcomingTodoCount.value = todoEnv.data.todo_list.filter((item) => {
      const raw = item.endTime || item.end_time;
      if (!raw) return false;
      const parsed = new Date(raw).getTime();
      return Number.isFinite(parsed) && parsed >= now && parsed <= now + 7 * 86400000;
    }).length;

    upcomingExamCount.value = (scholarEnv.data.exams || []).filter((exam: any) => {
      const raw = exam.ksrq || exam.time || exam.date;
      if (!raw) return false;
      const parsed = new Date(raw).getTime();
      return Number.isFinite(parsed) && parsed >= now;
    }).length;
  } catch (error: any) {
    errorMsg.value = error?.message || String(error);
  } finally {
    isLoading.value = false;
  }
}

watch(selectedSemesterName, () => {
  customScores.value = {};
  customPreview.value = null;
});

onMounted(loadScholar);
watch(accountScope, loadScholar);
</script>

<template>
  <div class="page-shell scholar-view">
    <header class="page-header">
      <div>
        <h1>学业</h1>
        <p class="page-subtitle">后端是 GPA 唯一真值，首屏只保留紧凑摘要与趋势。</p>
      </div>
      <div class="scholar-header-actions">
        <SegmentedFilter
          :model-value="retakePolicy"
          :options="[
            { value: 'first', label: '首次成绩' },
            { value: 'highest', label: '最高成绩' },
          ]"
          @update:model-value="setRetakePolicy($event as 'first' | 'highest')"
        />
        <span class="badge" :class="isOffline ? 'warning' : 'accent'">{{ isOffline ? '缓存模式' : '实时数据' }}</span>
      </div>
    </header>

    <StatusBanner v-if="errorMsg" tone="danger" title="学业失败">
      {{ errorMsg }}
    </StatusBanner>
    <StatusBanner v-else-if="isOffline && offlineTime" tone="warning" title="缓存回退">
      当前展示的是本地缓存，更新时间 {{ offlineTime }}。
    </StatusBanner>

    <SectionCard v-if="isLoading" title="加载中" subtitle="正在同步成绩、考试和规则口径。">
      <div class="state-card">请稍候，正在构建统一的 GPA 与学期摘要。</div>
    </SectionCard>

    <template v-else-if="scholar">
      <div class="scholar-summary-grid">
        <InlineStat
          v-for="item in summaryMetrics"
          :key="item.label"
          :label="item.label"
          :value="item.value"
          :hint="item.hint"
          emphasis
        />
      </div>

      <SectionCard title="次级摘要" subtitle="不抢首屏，但保留完整上下文。" dense>
        <div class="scholar-summary-grid secondary">
          <InlineStat v-for="item in extraMetrics" :key="item.label" :label="item.label" :value="item.value" />
        </div>
      </SectionCard>

      <div class="scholar-two-col">
        <SectionCard title="学期趋势" subtitle="切换重修策略后立即同步。">
          <div class="trend-list">
            <article v-for="row in trendRows" :key="row.key" class="trend-row">
              <div class="trend-row__meta">
                <strong>{{ row.label }}</strong>
                <span>{{ hideGpa ? '****' : row.fivePoint.toFixed(2) }} / {{ hideGpa ? '****' : row.fourPoint.toFixed(2) }}</span>
              </div>
              <div class="trend-row__bars">
                <div class="trend-bar">
                  <span class="trend-bar__fill five" :style="{ width: row.fiveWidth }"></span>
                </div>
                <div class="trend-bar">
                  <span class="trend-bar__fill four" :style="{ width: row.fourWidth }"></span>
                </div>
              </div>
            </article>
          </div>
        </SectionCard>

        <SectionCard title="综合分析" subtitle="AI 配置缺失时直达设置页。">
          <div class="scholar-side-actions">
            <ActionPill v-if="zeroClawEndpoint" tone="accent" :disabled="aiLoading" @click="analyzeScholar">
              {{ aiLoading ? '分析中…' : '生成综合分析' }}
            </ActionPill>
            <ActionPill v-else tone="warning" @click="navigateToSettings">去设置</ActionPill>
            <ActionPill @click="exportToCsv">导出 CSV</ActionPill>
            <ActionPill @click="customMode = !customMode">{{ customMode ? '收起 DIY 模拟' : 'DIY 均绩模拟' }}</ActionPill>
          </div>
          <StatusBanner v-if="aiError" tone="warning" title="AI 提示">
            {{ aiError }}
          </StatusBanner>
          <article v-if="aiMarkdown" class="ai-markdown">{{ aiMarkdown }}</article>
          <StatusBanner v-else tone="warning" title="AI 未就绪">
            {{ zeroClawEndpoint ? '点击后会输出风险课程、趋势判断和行动建议。' : '尚未配置 ZeroClaw Endpoint，点击“去设置”完成接入。' }}
          </StatusBanner>
        </SectionCard>
      </div>

      <SectionCard title="学期明细" subtitle="紧凑表格式查看课程明细和学期均绩。">
        <div class="semester-toolbar">
          <select v-model="selectedSemesterName" class="select-field semester-select">
            <option v-for="item in semesterOptions" :key="item.value" :value="item.value">{{ item.label }}</option>
          </select>
          <div class="semester-stats">
            <InlineStat label="学期五分" :value="mask(semesterSummary.fivePoint)" emphasis />
            <InlineStat label="学期 4.3" :value="mask(semesterSummary.fourPoint)" />
            <InlineStat label="获得学分" :value="semesterSummary.totalCredits.toFixed(1)" />
            <InlineStat label="平均分" :value="mask(semesterSummary.hundredPoint)" />
          </div>
        </div>

        <div class="table-shell">
          <table class="data-table scholar-table">
            <thead>
              <tr>
                <th>课程名</th>
                <th>成绩</th>
                <th>学分</th>
                <th>五分</th>
                <th>4.3</th>
                <th>主修</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="row in gradeRows" :key="row.id">
                <td>{{ row.courseName }}</td>
                <td>{{ hideGpa ? '****' : row.scoreText }}</td>
                <td>{{ row.credit.toFixed(1) }}</td>
                <td>{{ hideGpa ? '****' : row.fivePoint.toFixed(2) }}</td>
                <td>{{ hideGpa ? '****' : row.fourPoint.toFixed(2) }}</td>
                <td>
                  <span class="badge" :class="row.isMajor ? 'accent' : ''">{{ row.isMajor ? '主修' : '通识/其他' }}</span>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      </SectionCard>

      <SectionCard v-if="customMode" title="DIY 均绩模拟" subtitle="基于当前学期课程，调用后端同一算法。">
        <div class="custom-grid">
          <div class="custom-list">
            <label v-for="row in customRows" :key="row.id" class="custom-row">
              <span>{{ row.courseName }}</span>
              <input
                v-model="customScores[row.id]"
                class="input-field"
                type="number"
                min="0"
                max="100"
                step="0.1"
                :placeholder="String(row.raw.hundredPoint || row.scoreText || '')"
              />
            </label>
          </div>
          <div class="custom-side">
            <ActionPill tone="accent" :disabled="customLoading" @click="calculateCustomPreview">
              {{ customLoading ? '计算中…' : '计算当前学期模拟' }}
            </ActionPill>
            <div v-if="customPreview" class="custom-preview-grid">
              <InlineStat label="DIY 五分" :value="customPreview.fivePoint.toFixed(2)" emphasis />
              <InlineStat label="DIY 4.3" :value="customPreview.fourPoint.toFixed(2)" />
              <InlineStat label="DIY 平均分" :value="customPreview.hundredPoint.toFixed(2)" />
              <InlineStat label="DIY 学分" :value="customPreview.totalCredits.toFixed(1)" />
            </div>
            <div v-else class="state-card">输入分数后点击计算，结果会严格走后端 GPA 引擎。</div>
          </div>
        </div>
      </SectionCard>

      <StatusBanner title="GPA 口径">标准 GPA 与 DIY 模拟都复用后端引擎，重修策略切换会同步刷新摘要、趋势和课程表格。</StatusBanner>
    </template>
  </div>
</template>

<style scoped>
.scholar-view {
  gap: 1rem;
}

.scholar-header-actions,
.scholar-side-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.7rem;
  align-items: center;
}

.scholar-summary-grid {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  gap: 0.75rem;
}

.scholar-summary-grid.secondary {
  grid-template-columns: repeat(4, minmax(0, 1fr));
}

.scholar-two-col,
.custom-grid {
  display: grid;
  grid-template-columns: minmax(0, 1.15fr) minmax(320px, 0.85fr);
  gap: 1rem;
}

.trend-list,
.custom-list,
.custom-side {
  display: flex;
  flex-direction: column;
  gap: 0.8rem;
}

.trend-row {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.trend-row__meta {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
}

.trend-row__meta strong {
  color: var(--text-primary);
}

.trend-row__meta span {
  color: var(--text-secondary);
}

.trend-row__bars {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.trend-bar {
  width: 100%;
  height: 0.65rem;
  border-radius: var(--radius-pill);
  background: var(--surface-2);
  overflow: hidden;
}

.trend-bar__fill {
  display: block;
  height: 100%;
  border-radius: inherit;
}

.trend-bar__fill.five {
  background: var(--accent-text);
}

.trend-bar__fill.four {
  background: var(--success-text);
}

.ai-markdown {
  white-space: pre-wrap;
  word-break: break-word;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: var(--surface-2);
  padding: 1rem;
  color: var(--text-primary);
  max-height: 340px;
  overflow: auto;
}

.semester-toolbar,
.semester-stats {
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem;
  align-items: center;
}

.semester-select {
  width: min(100%, 280px);
}

.semester-stats {
  flex: 1;
}

.table-shell {
  overflow-x: auto;
}

.scholar-table th,
.scholar-table td {
  color: var(--text-primary);
}

.scholar-table tbody tr:hover {
  background: var(--surface-2);
}

.scholar-table td:first-child {
  min-width: 14rem;
}

.custom-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 140px;
  gap: 0.7rem;
  align-items: center;
}

.custom-row span {
  color: var(--text-secondary);
}

.custom-preview-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.75rem;
}

@media (max-width: 980px) {
  .scholar-summary-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .scholar-summary-grid.secondary,
  .scholar-two-col,
  .custom-grid {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 720px) {
  .scholar-summary-grid,
  .scholar-summary-grid.secondary,
  .custom-preview-grid {
    grid-template-columns: 1fr;
  }

  .custom-row {
    grid-template-columns: 1fr;
  }
}
</style>
