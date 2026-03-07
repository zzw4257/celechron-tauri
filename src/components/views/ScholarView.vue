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

const {
  accountScope,
  courseIdMappings,
  hideGpa,
  retakePolicy,
  setRetakePolicy,
  zeroClawApiKey,
  zeroClawEndpoint,
} = usePreferences();

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

function formatDelta(value: number, digits = 2) {
  if (!Number.isFinite(value) || Math.abs(value) < 0.0001) return '±0.00';
  const sign = value > 0 ? '+' : '';
  return `${sign}${value.toFixed(digits)}`;
}

function numeric(value: unknown, fallback = 0) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function normalizeScoreText(value: unknown) {
  const text = String(value || '').trim();
  return text || '待录';
}

function isPendingScoreText(value: string) {
  const normalized = value.replace(/\s+/g, '');
  return !normalized
    || normalized === '--'
    || normalized.includes('待录')
    || normalized.includes('缓考')
    || normalized.includes('未录')
    || normalized.includes('未出')
    || normalized.includes('未评')
    || normalized.includes('修读中')
    || normalized.includes('进行中');
}

function isDeferredScoreText(value: string) {
  const normalized = value.replace(/\s+/g, '');
  return normalized.includes('缓考') || normalized.includes('待录') || normalized.includes('未录');
}

const majorCourseIds = computed(() => new Set(scholar.value?.majorCourseIds || []));
const semesters = computed(() => scholar.value?.semesters || []);
const selectedSemester = computed<ScholarSemester | null>(() => semesters.value.find((item) => item.name === selectedSemesterName.value) || semesters.value[0] || null);
const displayGpa = computed(() => scholar.value?.gpaByPolicy?.[retakePolicy.value] || scholar.value?.gpaByPolicy?.first || EMPTY_GPA);
const semesterSummary = computed(() => selectedSemester.value?.gpaByPolicy?.[retakePolicy.value] || selectedSemester.value?.gpaByPolicy?.first || EMPTY_GPA);

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

const semesterOptions = computed(() => semesters.value.map((item) => ({
  value: item.name,
  label: item.displayName || formatTermDisplayName(item.term, item.name),
})));

const trendRows = computed(() => {
  return [...semesters.value].reverse().map((item) => {
    const summary = item.gpaByPolicy?.[retakePolicy.value] || item.gpaByPolicy?.first || EMPTY_GPA;
    return {
      key: item.name,
      label: item.displayName || formatTermDisplayName(item.term, item.name),
      fivePoint: summary.fivePoint,
      fourPoint: summary.fourPoint,
      hundredPoint: summary.hundredPoint,
    };
  });
});

const trendChart = computed(() => {
  const rows = trendRows.value;
  if (!rows.length) return { width: 520, height: 180, line: '', area: '', points: [] as Array<{ x: number; y: number; label: string; value: number }> };
  const width = 520;
  const height = 180;
  const padX = 24;
  const padY = 24;
  const drawableW = Math.max(width - padX * 2, 1);
  const drawableH = Math.max(height - padY * 2, 1);
  return {
    ...buildSparkline(rows.map((item) => item.fivePoint), rows.map((item) => item.label), width, height, padX, padY, drawableW, drawableH),
    width,
    height,
  };
});

function buildSparkline(values: number[], labels: string[], width: number, height: number, padX: number, padY: number, drawableW: number, drawableH: number) {
  if (!values.length) {
    return { line: '', area: '', points: [] as Array<{ x: number; y: number; label: string; value: number }> };
  }
  const max = Math.max(...values, 5);
  const min = Math.min(...values, 0);
  const range = Math.max(max - min, 0.1);
  const points = values.map((value, index) => {
    const x = values.length === 1 ? width / 2 : padX + (drawableW * index) / (values.length - 1);
    const y = padY + drawableH - ((value - min) / range) * drawableH;
    return { x, y, label: labels[index], value };
  });
  const line = points.map((point) => `${point.x},${point.y}`).join(' ');
  const area = `${padX},${height - padY} ${line} ${padX + drawableW},${height - padY}`;
  return { line, area, points };
}

const gradeRows = computed(() => {
  const current = selectedSemester.value;
  if (!current) return [];
  return (current.grades || []).map((grade: any) => {
    const fallbackId = String(grade.xkkh || `${current.name}-${grade.kcmc || grade.kcdm || 'course'}`);
    const kcdm = String(grade.kcdm || '');
    const scoreText = normalizeScoreText(grade.cj);
    const hundredPoint = numeric(grade.hundredPoint, NaN);
    const hasNumericScore = Number.isFinite(hundredPoint);
    const isMajor = majorCourseIds.value.has(fallbackId) || majorCourseIds.value.has(kcdm);
    const isDeferred = isDeferredScoreText(scoreText);
    const isPending = isPendingScoreText(scoreText) || !hasNumericScore;
    return {
      id: fallbackId,
      courseName: String(grade.kcmc || '未命名课程'),
      scoreText,
      credit: numeric(grade.credit ?? grade.xf),
      fivePoint: numeric(grade.fivePoint),
      fourPoint: numeric(grade.fourPoint),
      hundredPoint,
      hasNumericScore,
      isPending,
      isDeferred,
      priorityLabel: isDeferred ? '缓考 / 待补录' : (isPending ? '未出分' : '已出分'),
      priorityTone: isDeferred ? 'warning' : (isPending ? 'accent' : ''),
      scorePlaceholder: hasNumericScore ? hundredPoint.toFixed(1) : '输入预计分',
      isMajor,
      raw: { ...grade, xkkh: fallbackId },
    };
  });
});

const customRows = computed(() => {
  return [...gradeRows.value]
    .filter((row) => row.credit > 0)
    .sort((left, right) => Number(right.isPending) - Number(left.isPending)
      || Number(left.hasNumericScore) - Number(right.hasNumericScore)
      || right.credit - left.credit
      || left.courseName.localeCompare(right.courseName));
});

const customPriorityRows = computed(() => customRows.value.filter((row) => row.isPending));
const customSupplementRows = computed(() => customRows.value.filter((row) => !row.isPending));
const customFilledCount = computed(() => Object.values(customScores.value).filter((value) => String(value).trim() !== '' && Number.isFinite(Number(value))).length);
const customGuideText = computed(() => {
  if (customPriorityRows.value.length) {
    return `优先给本学期 ${customPriorityRows.value.length} 门缓考、待录或暂未出分课程填写预估分；已出分课程只作为补充压力测试。`;
  }
  if (customRows.value.length) {
    return '当前学期没有待录/缓考课程，DIY 区会退化成已出分课程的压力测试模式。';
  }
  return '当前学期没有可用于模拟的课程。';
});


const comparisonRows = computed(() => {
  if (!customPreview.value) return [];
  return [
    { label: '五分制', max: 5, original: semesterSummary.value.fivePoint, preview: customPreview.value.fivePoint },
    { label: '4.3 制', max: 4.3, original: semesterSummary.value.fourPoint, preview: customPreview.value.fourPoint },
    { label: '平均分', max: 100, original: semesterSummary.value.hundredPoint, preview: customPreview.value.hundredPoint },
  ].map((item) => ({
    ...item,
    delta: item.preview - item.original,
    originalWidth: `${Math.max((item.original / item.max) * 100, 4)}%`,
    previewWidth: `${Math.max((item.preview / item.max) * 100, 4)}%`,
  }));
});

const customSummaryMetrics = computed(() => {
  if (!customPreview.value) return [];
  return [
    { label: 'DIY 五分', value: customPreview.value.fivePoint.toFixed(2), delta: formatDelta(customPreview.value.fivePoint - semesterSummary.value.fivePoint) },
    { label: 'DIY 4.3', value: customPreview.value.fourPoint.toFixed(2), delta: formatDelta(customPreview.value.fourPoint - semesterSummary.value.fourPoint) },
    { label: 'DIY 平均分', value: customPreview.value.hundredPoint.toFixed(2), delta: formatDelta(customPreview.value.hundredPoint - semesterSummary.value.hundredPoint) },
    { label: 'DIY 学分', value: customPreview.value.totalCredits.toFixed(1), delta: formatDelta(customPreview.value.totalCredits - semesterSummary.value.totalCredits, 1) },
  ];
});

async function exportToCsv() {
  const headers = ['学期', '课程名称', '课程代码', '学分', '成绩', '五分', '4.3', '主修'];
  let csv = `data:text/csv;charset=utf-8,\uFEFF${headers.join(',')}\n`;
  for (const semester of semesters.value) {
    const label = semester.displayName || formatTermDisplayName(semester.term, semester.name);
    for (const grade of semester.grades || []) {
      const gradeId = String(grade.xkkh || `${semester.name}-${grade.kcmc || grade.kcdm || 'course'}`);
      const isMajor = majorCourseIds.value.has(gradeId) || majorCourseIds.value.has(String(grade.kcdm || ''));
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
        availableTabs: ['学业', '日程', '接下来', '任务', '资料', '设置'],
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

async function calculateCustomPreview() {
  if (!selectedSemester.value) return;
  customLoading.value = true;
  errorMsg.value = '';

  try {
    const simulatedScores = Object.fromEntries(
      Object.entries(customScores.value)
        .map(([key, value]) => [key, Number(value)])
        .filter(([, value]) => Number.isFinite(value)),
    );

    if (Object.keys(simulatedScores).length === 0) {
      customPreview.value = null;
      errorMsg.value = customPriorityRows.value.length
        ? '先给缓考、待录或未出分课程输入预计分，再计算本学期模拟。'
        : '先输入至少一门课程的预计分，再计算当前学期模拟。';
      return;
    }

    const previewGrades = (selectedSemester.value.grades || []).map((grade: any) => ({
      ...grade,
      xkkh: String(grade.xkkh || `${selectedSemester.value?.name}-${grade.kcmc || grade.kcdm || 'course'}`),
    }));

    customPreview.value = await calculateGpaPreview({
      grades: previewGrades,
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

watch(retakePolicy, () => {
  if (customFilledCount.value > 0) {
    void calculateCustomPreview();
    return;
  }
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
        <p class="page-subtitle">后端是 GPA 唯一真值；学期切换、趋势和 DIY 模拟都从同一口径出发。</p>
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
        <InlineStat v-for="item in summaryMetrics" :key="item.label" :label="item.label" :value="item.value" :hint="item.hint" emphasis />
      </div>

      <SectionCard title="次级摘要" subtitle="不抢首屏，但保留完整上下文。" dense>
        <div class="scholar-summary-grid secondary">
          <InlineStat v-for="item in extraMetrics" :key="item.label" :label="item.label" :value="item.value" />
        </div>
      </SectionCard>

      <div class="scholar-two-col">
        <SectionCard title="学期趋势" subtitle="曲线回来了，切换重修策略会立即同步。">
          <div class="trend-chart" v-if="trendChart.points.length">
            <svg :viewBox="`0 0 ${trendChart.width} ${trendChart.height}`" preserveAspectRatio="none" class="trend-chart__svg">
              <polygon :points="trendChart.area" class="trend-chart__area" />
              <polyline :points="trendChart.line" class="trend-chart__line" />
              <g v-for="point in trendChart.points" :key="point.label">
                <circle :cx="point.x" :cy="point.y" r="5" class="trend-chart__dot" />
              </g>
            </svg>
            <div class="trend-chart__labels">
              <span v-for="point in trendChart.points" :key="point.label">{{ point.label }}</span>
            </div>
          </div>

          <div class="trend-list">
            <article v-for="row in trendRows" :key="row.key" class="trend-row">
              <div class="trend-row__meta">
                <strong>{{ row.label }}</strong>
                <span>{{ hideGpa ? '****' : row.fivePoint.toFixed(2) }} / {{ hideGpa ? '****' : row.fourPoint.toFixed(2) }}</span>
              </div>
              <div class="trend-row__bars">
                <div class="trend-bar"><span class="trend-bar__fill five" :style="{ width: `${Math.max((row.fivePoint / 5) * 100, 4)}%` }"></span></div>
                <div class="trend-bar secondary"><span class="trend-bar__fill four" :style="{ width: `${Math.max((row.fourPoint / 4.3) * 100, 4)}%` }"></span></div>
              </div>
            </article>
          </div>
        </SectionCard>

        <SectionCard title="AI 网关" subtitle="优先做快速 setup；后续可以把学业、任务、资料与页面跳转串联进去。">
          <div class="scholar-side-actions">
            <ActionPill v-if="zeroClawEndpoint" tone="accent" :disabled="aiLoading" @click="analyzeScholar">
              {{ aiLoading ? '分析中…' : '生成综合分析' }}
            </ActionPill>
            <ActionPill v-else tone="warning" @click="navigateToSettings">快速配置 AI</ActionPill>
            <ActionPill @click="exportToCsv">导出 CSV</ActionPill>
            <ActionPill @click="customMode = !customMode">{{ customMode ? '收起 DIY 模拟' : 'DIY 均绩模拟' }}</ActionPill>
          </div>
          <StatusBanner v-if="aiError" tone="warning" title="AI 提示">{{ aiError }}</StatusBanner>
          <article v-if="aiMarkdown" class="ai-markdown">{{ aiMarkdown }}</article>
          <div v-else class="ai-guide">
            <p>当前 AI 能吃到成绩、学期趋势、考试和近期任务。下一步会继续接上资料与页面操作能力。</p>
            <ActionPill :tone="zeroClawEndpoint ? 'accent' : 'warning'" @click="navigateToSettings">
              {{ zeroClawEndpoint ? '调整 AI 配置' : '去设置完成快速接入' }}
            </ActionPill>
          </div>
        </SectionCard>
      </div>

      <SectionCard title="学期明细" subtitle="用学期胶囊切换，不再使用原生下拉。">
        <div class="semester-toolbar">
          <div class="semester-switcher" role="tablist" aria-label="学期切换">
            <button
              v-for="item in semesterOptions"
              :key="item.value"
              type="button"
              class="semester-chip"
              :class="{ active: selectedSemesterName === item.value }"
              @click="selectedSemesterName = item.value"
            >
              {{ item.label }}
            </button>
          </div>
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
                <td><span class="badge" :class="row.isMajor ? 'accent' : ''">{{ row.isMajor ? '主修' : '通识/其他' }}</span></td>
              </tr>
            </tbody>
          </table>
        </div>
      </SectionCard>

      <SectionCard v-if="customMode" title="DIY 均绩模拟" subtitle="优先给本学期缓考、待录与未出分课程做预估，右侧给出可视化对比。">
        <div class="custom-grid">
          <div class="custom-list">
            <StatusBanner title="模拟重点" tone="info">{{ customGuideText }}</StatusBanner>

            <div v-if="customRows.length === 0" class="state-card">当前学期暂无可模拟课程。</div>

            <template v-else>
              <section v-if="customPriorityRows.length" class="custom-group">
                <header class="custom-group__head">
                  <strong>优先预估</strong>
                  <span>{{ customPriorityRows.length }} 门</span>
                </header>
                <label v-for="row in customPriorityRows" :key="row.id" class="custom-row">
                  <div class="custom-row__meta">
                    <strong>{{ row.courseName }}</strong>
                    <p>{{ row.scoreText }} · {{ row.credit.toFixed(1) }} 学分</p>
                  </div>
                  <div class="custom-row__input">
                    <span class="badge" :class="row.priorityTone">{{ row.priorityLabel }}</span>
                    <input
                      v-model="customScores[row.id]"
                      class="input-field"
                      type="number"
                      min="0"
                      max="100"
                      step="0.1"
                      inputmode="decimal"
                      :placeholder="row.scorePlaceholder"
                    />
                  </div>
                </label>
              </section>

              <section v-if="customSupplementRows.length" class="custom-group secondary">
                <header class="custom-group__head">
                  <strong>补充压力测试</strong>
                  <span>{{ customSupplementRows.length }} 门已出分课程</span>
                </header>
                <label v-for="row in customSupplementRows" :key="row.id" class="custom-row">
                  <div class="custom-row__meta">
                    <strong>{{ row.courseName }}</strong>
                    <p>{{ row.scoreText }} · {{ row.credit.toFixed(1) }} 学分</p>
                  </div>
                  <div class="custom-row__input">
                    <span class="badge">已出分</span>
                    <input
                      v-model="customScores[row.id]"
                      class="input-field"
                      type="number"
                      min="0"
                      max="100"
                      step="0.1"
                      inputmode="decimal"
                      :placeholder="row.scorePlaceholder"
                    />
                  </div>
                </label>
              </section>
            </template>
          </div>
          <div class="custom-side">
            <div class="custom-side__actions">
              <ActionPill tone="accent" :disabled="customLoading || customRows.length === 0" @click="calculateCustomPreview">
                {{ customLoading ? '计算中…' : '计算本学期预估均绩' }}
              </ActionPill>
              <span class="badge accent">已填写 {{ customFilledCount }} 门</span>
            </div>

            <div v-if="customPreview" class="custom-preview-grid">
              <div v-for="item in customSummaryMetrics" :key="item.label" class="custom-preview-card">
                <span>{{ item.label }}</span>
                <strong>{{ hideGpa ? '****' : item.value }}</strong>
                <small>{{ hideGpa ? '****' : item.delta }}</small>
              </div>
            </div>
            <div v-else class="state-card">先给本学期待录/未出分课程输入预计分，再点击计算；右侧会直接对比当前值和模拟值。</div>

            <div v-if="customPreview" class="comparison-list">
              <article v-for="item in comparisonRows" :key="item.label" class="comparison-row">
                <div class="comparison-row__head">
                  <strong>{{ item.label }}</strong>
                  <span>{{ hideGpa ? '****' : formatDelta(item.delta) }}</span>
                </div>
                <div class="comparison-row__bars">
                  <div class="comparison-bar original"><span :style="{ width: item.originalWidth }"></span></div>
                  <div class="comparison-bar preview"><span :style="{ width: item.previewWidth }"></span></div>
                </div>
                <div class="comparison-row__labels">
                  <small>当前 {{ hideGpa ? '****' : item.original.toFixed(2) }}</small>
                  <small>模拟 {{ hideGpa ? '****' : item.preview.toFixed(2) }}</small>
                </div>
              </article>
            </div>
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
.scholar-side-actions,
.semester-toolbar,
.semester-stats,
.semester-switcher {
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem;
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
  grid-template-columns: minmax(0, 1.2fr) minmax(320px, 0.8fr);
  gap: 1rem;
}

.trend-list,
.custom-list,
.custom-side,
.comparison-list {
  display: flex;
  flex-direction: column;
  gap: 0.8rem;
}

.trend-chart {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: linear-gradient(180deg, color-mix(in srgb, var(--accent-text) 10%, var(--surface-1)) 0%, var(--surface-2) 100%);
  padding: 0.85rem;
}

.trend-chart__svg {
  width: 100%;
  height: 180px;
}

.trend-chart__area {
  fill: color-mix(in srgb, var(--accent-text) 18%, transparent);
}

.trend-chart__line {
  fill: none;
  stroke: var(--accent-text);
  stroke-width: 3;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.trend-chart__dot {
  fill: var(--surface-1);
  stroke: var(--accent-text);
  stroke-width: 3;
}

.trend-chart__labels {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(72px, 1fr));
  gap: 0.45rem;
  margin-top: 0.55rem;
  color: var(--text-secondary);
  font-size: 0.8rem;
}

.trend-row {
  display: flex;
  flex-direction: column;
  gap: 0.45rem;
}

.trend-row__meta,
.comparison-row__head,
.comparison-row__labels {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
}

.trend-row__meta strong,
.comparison-row__head strong {
  color: var(--text-primary);
}

.trend-row__meta span,
.comparison-row__head span,
.comparison-row__labels small,
.ai-guide p {
  color: var(--text-secondary);
}

.trend-row__bars,
.comparison-row__bars {
  display: flex;
  flex-direction: column;
  gap: 0.45rem;
}

.trend-bar,
.comparison-bar {
  width: 100%;
  height: 0.72rem;
  border-radius: var(--radius-pill);
  background: var(--surface-2);
  overflow: hidden;
}

.trend-bar__fill,
.comparison-bar span {
  display: block;
  height: 100%;
  border-radius: inherit;
}

.trend-bar__fill.five,
.comparison-bar.preview span {
  background: var(--accent-text);
}

.trend-bar__fill.four,
.comparison-bar.original span {
  background: color-mix(in srgb, var(--success-text) 90%, white 10%);
}

.ai-markdown,
.ai-guide {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: var(--surface-2);
  padding: 1rem;
  color: var(--text-primary);
}

.ai-markdown {
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 340px;
  overflow: auto;
}

.ai-guide {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.semester-toolbar {
  justify-content: space-between;
}

.semester-switcher {
  flex: 1;
  overflow-x: auto;
  padding-bottom: 0.1rem;
}

.semester-chip {
  border: 1px solid var(--border-subtle);
  background: var(--surface-2);
  color: var(--text-secondary);
  border-radius: var(--radius-pill);
  min-height: 2.35rem;
  padding: 0.55rem 0.95rem;
  white-space: nowrap;
  cursor: pointer;
}

.semester-chip.active {
  background: var(--surface-accent);
  border-color: var(--accent-border);
  color: var(--accent-text);
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

.custom-group {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: var(--surface-2);
  padding: 0.85rem;
}

.custom-group.secondary {
  background: color-mix(in srgb, var(--surface-2) 92%, var(--surface-1));
}

.custom-group__head,
.custom-side__actions,
.custom-row__input {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
}

.custom-group__head {
  padding-bottom: 0.65rem;
  border-bottom: 1px solid var(--border-subtle);
}

.custom-group__head strong,
.custom-row__meta strong {
  color: var(--text-primary);
}

.custom-group__head span,
.custom-row__meta p {
  color: var(--text-secondary);
}

.custom-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(220px, 260px);
  gap: 0.75rem;
  align-items: center;
  padding-top: 0.75rem;
}

.custom-row__meta {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}

.custom-row__meta p {
  margin: 0;
}

.custom-row__input {
  justify-content: flex-end;
}

.custom-row__input .input-field {
  width: min(100%, 150px);
}

.custom-preview-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 0.75rem;
}

.custom-preview-card {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: linear-gradient(160deg, color-mix(in srgb, var(--accent-text) 10%, var(--surface-1)) 0%, var(--surface-2) 100%);
  padding: 0.9rem;
}

.custom-preview-card span,
.custom-preview-card small {
  color: var(--text-secondary);
}

.custom-preview-card strong {
  display: block;
  margin-top: 0.15rem;
  color: var(--text-primary);
  font-size: 1.55rem;
}

.custom-side__actions {
  flex-wrap: wrap;
}

.comparison-row {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-card-sm);
  background: var(--surface-2);
  padding: 0.85rem;
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
  .custom-preview-grid,
  .custom-row {
    grid-template-columns: 1fr;
  }

  .custom-row__input {
    align-items: stretch;
    flex-direction: column;
  }

  .custom-row__input .input-field {
    width: 100%;
  }
}
</style>
