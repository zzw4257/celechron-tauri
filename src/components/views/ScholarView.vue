<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";

const isLoading = ref(true);
const errorMsg = ref("");

// State variables
const gpa = ref({
  fivePoint: 0,
  fourPoint: 0,
  fourPointLegacy: 0,
  hundredPoint: 0,
  totalCredits: 0,
  majorGpa: 0,
  majorGpaLegacy: 0,
  majorCredits: 0,
});

const semester = ref({
  name: "获取中...",
  courseCount: 0,
  courseCredits: 0,
  examCount: 0,
  gpaArr: [0, 0, 0, 0],
});

const semestersList = ref<any[]>([]);
const selectedSemesterIndex = ref(0);

const practice = ref({
  pt2: 0,
  pt3: 0,
  pt4: 0,
});

const exams = ref<any[]>([]);
const todos = ref<any[]>([]);
const todoStats = ref({ total: 0, inOneDay: 0, inOneWeek: 0 });
const lastUpdate = ref("正在同步...");
const isOffline = ref(false);
const offlineTime = ref("");

const gradeItems = [
  { label: "五分制", value: () => gpa.value.fivePoint.toFixed(2), color: "#06b6d4" },
  { label: "获得学分", value: () => gpa.value.totalCredits.toFixed(1), color: "#f97316" },
  { label: "四分制(4.3)", value: () => gpa.value.fourPoint.toFixed(2), color: "#22c55e" },
  { label: "主修四分制(4.3)", value: () => gpa.value.majorGpa.toFixed(2), color: "#ec4899" },
  { label: "主修学分", value: () => gpa.value.majorCredits.toFixed(1), color: "#eab308" },
  { label: "百分制", value: () => gpa.value.hundredPoint.toFixed(2), color: "#a855f7" },
  { label: "四分制(4.0)", value: () => gpa.value.fourPointLegacy.toFixed(2), color: "#10b981" },
  { label: "主修四分制(4.0)", value: () => gpa.value.majorGpaLegacy.toFixed(2), color: "#f43f5e" },
];

const customGpaMode = ref(false);
const customGpaSelected = ref(new Set<string>());
const majorCourseIds = ref(new Set<string>());

const simulatedScores = ref<Record<string, number>>({});

function toFivePoint(score: number): number {
  if (score >= 95) return 5.0;
  if (score >= 90) return 4.5;
  if (score >= 85) return 4.0;
  if (score >= 80) return 3.5;
  if (score >= 75) return 3.0;
  if (score >= 70) return 2.5;
  if (score >= 65) return 2.0;
  if (score >= 60) return 1.5;
  return 0.0;
}
function toFourPoint43(fiveP: number): number {
  if (fiveP >= 5.0) return 4.3;
  if (fiveP >= 4.5) return 4.0;
  if (fiveP >= 4.0) return 3.7;
  if (fiveP >= 3.5) return 3.3;
  if (fiveP >= 3.0) return 3.0;
  if (fiveP >= 2.5) return 2.7;
  if (fiveP >= 2.0) return 2.3;
  if (fiveP >= 1.5) return 2.0;
  return 0.0;
}
function toFourPointLegacy(fiveP: number): number {
  if (fiveP >= 4.0) return 4.0;
  if (fiveP >= 3.0) return 3.0;
  if (fiveP >= 2.0) return 2.0;
  if (fiveP >= 1.5) return 1.5;
  return 0.0;
}

const customGpa = computed(() => {
  let totalCredits = 0;
  let weightedFive = 0;
  let weightedFour = 0;
  let weightedLegacy = 0;
  let weightedHundred = 0;
  
  let majorCredits = 0;
  let majorWeightedFour = 0;
  let majorWeightedLegacy = 0;
  
  if (!semestersList.value) return { fivePoint: 0, fourPoint: 0, fourPointLegacy: 0, hundredPoint: 0, totalCredits: 0, majorGpa: 0, majorGpaLegacy: 0, majorCredits: 0 };
  
  semestersList.value.forEach(sem => {
    sem.grades.forEach((g: any) => {
      // Only include if selected and it's a valid GPA course
      if (customGpaSelected.value.has(g.xkkh)) {
        let credit = g.credit || 0;
        let fiveP = g.fivePoint || 0;
        let fourP = g.fourPoint || 0;
        let legacyP = g.fourPointLegacy || 0;
        let hundredP = g.hundredPoint || 0;
        
        let cj = g.cj || "";
        
        if (["待录", "缓考", "无效"].includes(cj)) {
           if (simulatedScores.value[g.xkkh] !== undefined) {
               hundredP = simulatedScores.value[g.xkkh] || 0;
               fiveP = toFivePoint(hundredP);
               fourP = toFourPoint43(fiveP);
               legacyP = toFourPointLegacy(fiveP);
           } else {
               return; // Skip if no simulated prediction
           }
        } else if (["弃修", "合格", "不合格"].includes(cj)) {
            return;
        }

        if (credit > 0) {
            totalCredits += credit;
            weightedFive += credit * fiveP;
            weightedFour += credit * fourP;
            weightedLegacy += credit * legacyP;
            weightedHundred += credit * hundredP;
          if (majorCourseIds.value.has(g.xkkh)) {
             majorCredits += credit;
             majorWeightedFour += credit * fourP;
             majorWeightedLegacy += credit * legacyP;
          }
        }
      }
    });
  });
  
  if (totalCredits === 0) return { fivePoint: 0, fourPoint: 0, fourPointLegacy: 0, hundredPoint: 0, totalCredits: 0, majorGpa: 0, majorGpaLegacy: 0, majorCredits: 0 };
  
  return {
    fivePoint: weightedFive / totalCredits,
    fourPoint: weightedFour / totalCredits,
    fourPointLegacy: weightedLegacy / totalCredits,
    hundredPoint: weightedHundred / totalCredits,
    totalCredits: totalCredits,
    majorGpa: majorCredits > 0 ? majorWeightedFour / majorCredits : 0,
    majorGpaLegacy: majorCredits > 0 ? majorWeightedLegacy / majorCredits : 0,
    majorCredits: majorCredits
  };
});

const customGradeItems = [
  { label: "DIY 五分制", value: () => customGpa.value.fivePoint.toFixed(2), color: "#06b6d4" },
  { label: "DIY 总学分", value: () => customGpa.value.totalCredits.toFixed(1), color: "#f97316" },
  { label: "DIY 四分制(4.3)", value: () => customGpa.value.fourPoint.toFixed(2), color: "#22c55e" },
  { label: "DIY 主修(4.3)", value: () => customGpa.value.majorGpa.toFixed(2), color: "#ec4899" },
  { label: "DIY 主修学分", value: () => customGpa.value.majorCredits.toFixed(1), color: "#eab308" },
  { label: "DIY 百分制", value: () => customGpa.value.hundredPoint.toFixed(2), color: "#a855f7" },
  { label: "DIY 四分制(4.0)", value: () => customGpa.value.fourPointLegacy.toFixed(2), color: "#10b981" },
  { label: "DIY 主修(4.0)", value: () => customGpa.value.majorGpaLegacy.toFixed(2), color: "#f43f5e" },
];

function toggleCustomMode() {
  customGpaMode.value = !customGpaMode.value;
}

function toggleCourseSelection(xkkh: string) {
  if (!customGpaMode.value) return;
  const newSet = new Set(customGpaSelected.value);
  if (newSet.has(xkkh)) {
    newSet.delete(xkkh);
  } else {
    newSet.add(xkkh);
  }
  customGpaSelected.value = newSet;
}

function selectSemester(index: number) {
  selectedSemesterIndex.value = index;
  const items = semestersList.value[index];
  semester.value.name = items.name;
  semester.value.courseCount = items.grades.length;
  semester.value.courseCredits = items.credits;
  semester.value.gpaArr = items.gpa;
}

function exportToCSV() {
  const headers = ["学期", "课程名称", "课程代码", "学分", "成绩", "五分制绩点", "四分制(4.3)", "四分制(4.0)", "课程性质", "是否主修", "获得主修学分"];
  let csvContent = "data:text/csv;charset=utf-8,\uFEFF" + headers.join(",") + "\n";
  
  semestersList.value.forEach(sem => {
    sem.grades.forEach((g: any) => {
       const isMajor = majorCourseIds.value.has(g.xkkh);
       const row = [
         sem.name,
         `"${g.kcmc || ''}"`,
         g.kcdm || '',
         g.credit || g.xf || 0,
         g.cj || '',
         g.fivePoint || '',
         g.fourPoint || '',
         g.fourPointLegacy || '',
         g.kcxzmc || '',
         isMajor ? "是" : "否",
         isMajor ? (g.credit || g.xf || 0) : 0
       ];
       csvContent += row.join(",") + "\n";
    });
  });
  
  const encodedUri = encodeURI(csvContent);
  const link = document.createElement("a");
  link.setAttribute("href", encodedUri);
  link.setAttribute("download", `成绩单导出_${new Date().toISOString().split('T')[0]}.csv`);
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
}

function urgencyClass(urgent: boolean) {
  return urgent ? "urgent" : "";
}

function daysLeft(deadline: string) {
  const diff = Math.ceil((new Date(deadline).getTime() - Date.now()) / 86400000);
  if (diff <= 0) return "已截止";
  if (diff === 1) return "明天截止";
  return `${diff} 天后`;
}

async function fetchData() {
  try {
    isLoading.value = true;
    errorMsg.value = "";
    lastUpdate.value = "正在同步...";

    // Fetch scholar data (grades, practice, etc.)
    const data: any = await invoke("fetch_scholar_data");
    
    if (data._meta && data._meta.source === "cache") {
      isOffline.value = true;
      offlineTime.value = new Date(data._meta.timestamp * 1000).toLocaleString('zh-CN', { hour12: false });
    } else {
      isOffline.value = false;
    }
    
    gpa.value = {
      fivePoint: data.gpa.fivePoint || 0,
      fourPoint: data.gpa.fourPoint || 0,
      fourPointLegacy: data.gpa.fourPointLegacy || 0,
      hundredPoint: data.gpa.hundredPoint || 0,
      totalCredits: data.gpa.totalCredits || 0,
      majorGpa: data.gpa.majorGpa || 0,
      majorGpaLegacy: data.gpa.majorGpaLegacy || 0,
      majorCredits: data.gpa.majorCredits || 0,
    };

    practice.value = {
      pt2: data.practice.pt2 || 0,
      pt3: data.practice.pt3 || 0,
      pt4: data.practice.pt4 || 0,
    };

    // Calculate Semester Info 
    // Usually we pick the latest semester from the map
    semestersList.value = (data.semesters || []).reverse(); 

    // Retrieve Major Course Ids
    majorCourseIds.value = new Set(data.majorCourseIds || []);

    // Fetch Exams
    const nowTime = Date.now();
    exams.value = (data.exams || []).filter((e: any) => {
      const timeStr = e.kssj || e.qzkssj || (e.time ? e.time[0] : '');
      if (!timeStr || timeStr.includes("未知") || timeStr.trim() === "") return false;
      
      const match = timeStr.match(/(\d{4})年(\d{2})月(\d{2})日/);
      if (match) {
        const examDate = new Date(`${match[1]}-${match[2]}-${match[3]}T23:59:59`).getTime();
        if (examDate < nowTime - 86400000) return false;
      }
      return true;
    });

    // Pre-select all courses for custom GPA mode
    const allCourseIds = new Set<string>();
    semestersList.value.forEach((sem: any) => {
      sem.grades.forEach((g: any) => {
        allCourseIds.add(g.xkkh);
      });
    });
    customGpaSelected.value = allCourseIds;

    if (semestersList.value.length > 0) {
      semester.value.examCount = data.exams?.length || 0;
      selectSemester(0);
    } else {
      semester.value.name = "无数据";
    }

    // Fetch Todos
    try {
      const td: any = await invoke("fetch_todos");
      const list = td.todo_list || [];
      const now = Date.now();
      
      let inOneDay = 0;
      let inOneWeek = 0;

      const computedTodos = list.map((t: any) => {
        const time = new Date(t.end_time).getTime();
        const days = (time - now) / 86400000;
        if (days > 0 && days <= 1) inOneDay++;
        if (days > 0 && days <= 7) inOneWeek++;
        return {
          name: t.title,
          course: t.course_name,
          deadline: t.end_time,
          urgent: days <= 1 && days >= 0
        };
      }).sort((a: any, b: any) => new Date(a.deadline).getTime() - new Date(b.deadline).getTime());
      
      todos.value = computedTodos;

      todoStats.value = {
        total: list.length,
        inOneDay,
        inOneWeek,
      };
    } catch (e) {
      console.error("Failed to fetch todos:", e);
    }

    lastUpdate.value = "刚刚更新";
  } catch (err: any) {
    errorMsg.value = err.toString();
    lastUpdate.value = "更新失败";
  } finally {
    isLoading.value = false;
  }
}

onMounted(() => {
  fetchData();
});
</script>

<template>
  <div class="scholar-view">
    <!-- Header -->
    <header class="scholar-header">
      <h1>学业</h1>
      <span class="update-badge">
        <span class="dot" :class="isOffline ? 'orange' : 'green'"></span>
        {{ isOffline ? '离线模式' : ('更新于 ' + lastUpdate) }}
      </span>
    </header>

    <!-- Offline Warning Banner -->
    <div v-if="isOffline" class="offline-banner">
      <span class="offline-icon">⚠️</span>
      <div class="offline-text">
        <strong>网络连接异常，暂未同步最新数据。</strong>
        当前显示的是缓存在本地的数据 (更新于: {{ offlineTime }})
      </div>
    </div>

    <!-- Grade Brief Section -->
    <section class="section-card">
      <div class="section-header">
        <span class="section-icon">
          <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 10v6M2 10l10-5 10 5-10 5z"/><path d="M6 12v5c3 3 9 3 12 0v-5"/></svg>
        </span>
        <span class="section-title">成绩</span>
        <div style="flex-grow: 1;"></div>
        <button class="custom-gpa-toggle" @click="exportToCSV" style="margin-right: 8px;">
          导出 CSV
        </button>
        <button class="custom-gpa-toggle" :class="{ active: customGpaMode }" @click="toggleCustomMode">
          DIY 均绩模拟区
        </button>
      </div>
      <div class="grade-grid">
        <div
          v-for="(item, i) in (customGpaMode ? customGradeItems : gradeItems)"
          :key="i"
          class="grade-card"
          :style="{ '--accent': item.color }"
        >
          <span class="grade-label">{{ item.label }}</span>
          <span class="grade-value">{{ item.value() }}</span>
        </div>
      </div>
    </section>

    <!-- Semester Section -->
    <section class="section-card">
      <div class="section-header">
        <span class="section-icon">
          <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="18" rx="2" ry="2"></rect><line x1="16" y1="2" x2="16" y2="6"></line><line x1="8" y1="2" x2="8" y2="6"></line><line x1="3" y1="10" x2="21" y2="10"></line></svg>
        </span>
        <span class="section-title">课程明细</span>
        <span class="semester-badge">{{ semester.name }}</span>
      </div>
      
      <!-- Semester Navigator -->
      <div class="semester-scroll-nav">
        <button
          v-for="(sem, index) in semestersList"
          :key="sem.name"
          class="sem-nav-btn"
          :class="{ active: selectedSemesterIndex === index }"
          @click="selectSemester(index)"
        >
          {{ sem.name }}
        </button>
      </div>

      <div class="stats-row">
        <div class="stat-item">
          <span class="stat-value">{{ semester.courseCount }}</span>
          <span class="stat-label">课程数</span>
        </div>
        <div class="stat-item">
          <span class="stat-value">{{ semester.courseCredits.toFixed(1) }}</span>
          <span class="stat-label">学期学分</span>
        </div>
        <div class="stat-item">
          <span class="stat-value">{{ semester.gpaArr[0].toFixed(2) }}</span>
          <span class="stat-label">学期五分制</span>
        </div>
        <div class="stat-item">
          <span class="stat-value">{{ semester.gpaArr[1].toFixed(2) }}</span>
          <span class="stat-label">学期(4.3)</span>
        </div>
        <div class="stat-item" v-if="selectedSemesterIndex === 0">
          <span class="stat-value highlight-red">{{ semester.examCount }}</span>
          <span class="stat-label">考试</span>
        </div>
      </div>

      <!-- Course List -->
      <div class="course-list-wrap" v-if="semestersList.length > 0">
        <div 
          v-for="course in semestersList[selectedSemesterIndex]?.grades" 
          :key="course.xkkh"
          class="course-item-card"
          :class="{ 'dimmed': customGpaMode && !customGpaSelected.has(course.xkkh) }"
          @click="toggleCourseSelection(course.xkkh)"
          :style="{ cursor: customGpaMode ? 'pointer' : 'default' }"
        >
          <div class="course-item-header">
            <div style="display: flex; align-items: center; gap: 8px;">
              <span v-if="customGpaMode" class="custom-checkbox" :class="{ 'checked': customGpaSelected.has(course.xkkh) }"></span>
              <span class="course-item-name">{{ course.kcmc }}</span>
            </div>
            
            <input 
              v-if="customGpaMode && ['待录', '缓考', '无效'].includes(course.cj)" 
              type="number" 
              class="simulated-input" 
              v-model.number="simulatedScores[course.xkkh]" 
              placeholder="预估百制" 
              @click.stop 
            />
            <span v-else class="course-item-score" :class="{ 'failed': parseFloat(course.jd) === 0 }">{{ course.cj }}</span>
          </div>
          <div class="course-item-details" :style="{ paddingLeft: customGpaMode ? '24px' : '0' }">
            <span>学分: {{ course.credit }}</span>
            <span>五分: {{ course.fivePoint?.toFixed(2) }}</span>
            <span>四分(4.3): {{ course.fourPoint?.toFixed(2) }}</span>
            <span>类别: {{ course.kcxzmc }}</span>
          </div>
        </div>
      </div>
    </section>

    <!-- Exams Section -->
    <section class="section-card" v-if="exams.length > 0">
      <div class="section-header">
        <span class="section-icon">
          <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path></svg>
        </span>
        <span class="section-title">考试安排</span>
      </div>
      <div class="todo-scroll">
        <div
          v-for="(exam, i) in exams"
          :key="i"
          class="todo-card exam-card"
        >
          <div class="todo-name">{{ exam.kcmc || exam.name }} <span v-if="exam.qzkssj">(期中)</span></div>
          <div class="todo-course">时间: {{ exam.kssj || exam.qzkssj || (exam.time ? exam.time[0] : '未知') }}</div>
          <div class="todo-course">地点: {{ exam.jsmc || exam.qzjsmc || exam.location || '未知' }}</div>
          <div class="todo-course">座位: {{ exam.zwxh || exam.qzzwxh || exam.seat || '未知' }}</div>
        </div>
      </div>
    </section>

    <!-- Todos Section -->
    <section class="section-card">
      <div class="section-header">
        <span class="section-icon">
          <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path><polyline points="22 4 12 14.01 9 11.01"></polyline></svg>
        </span>
        <span class="section-title">作业</span>
      </div>
      <div class="stats-row">
        <div class="stat-item">
          <span class="stat-value">{{ todoStats.total }}</span>
          <span class="stat-label">总计</span>
        </div>
        <div class="stat-item">
          <span class="stat-value highlight-red">{{ todoStats.inOneDay }}</span>
          <span class="stat-label">一天内</span>
        </div>
        <div class="stat-item">
          <span class="stat-value highlight-yellow">{{ todoStats.inOneWeek }}</span>
          <span class="stat-label">本周截止</span>
        </div>
      </div>
      <div class="todo-scroll">
        <div
          v-for="(todo, i) in todos"
          :key="i"
          class="todo-card"
          :class="urgencyClass(todo.urgent)"
        >
          <div class="todo-name">{{ todo.name }}</div>
          <div class="todo-course">{{ todo.course }}</div>
          <div class="todo-deadline">{{ daysLeft(todo.deadline) }}</div>
        </div>
      </div>
    </section>

    <!-- Practice Section -->
    <section class="section-card">
      <div class="section-header">
        <span class="section-icon">⭐</span>
        <span class="section-title">实践</span>
      </div>
      <div class="stats-row">
        <div class="stat-item">
          <span class="stat-value">{{ practice.pt2.toFixed(2) }}</span>
          <span class="stat-label">二课分</span>
        </div>
        <div class="stat-item">
          <span class="stat-value">{{ practice.pt3.toFixed(2) }}</span>
          <span class="stat-label">三课分</span>
        </div>
        <div class="stat-item">
          <span class="stat-value">{{ practice.pt4.toFixed(2) }}</span>
          <span class="stat-label">四课分</span>
        </div>
      </div>
    </section>
    <!-- GPA Rule Explanation -->
    <section class="section-card">
      <div class="section-header">
        <span class="section-icon">
          <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><path d="M12 16v-4"></path><path d="M12 8h.01"></path></svg>
        </span>
        <span class="section-title">浙大均绩计算规则</span>
      </div>
      <div class="rule-content">
        <p><strong>五分制：</strong> 满分5.0。95-100对应5.0，90-94对应4.5，85-89对应4.0，以此类推直至60分对应1.5，不及格为0。</p>
        <p><strong>四分制 (4.3)：</strong> 近年来浙大推行的主要标准。五分制 5.0 → 4.3，4.5 → 4.0，4.0 → 3.7，3.5 → 3.3，3.0 → 3.0，2.5 → 2.7...</p>
        <p><strong>四分制 (4.0)：</strong> 经典算法。五分制 ≥4.0 → 4.0，≥3.0 → 3.0，≥2.0 → 2.0，1.5 → 1.5。</p>
        <p class="formula"><strong>公式：</strong> Σ(课程绩点 × 学分) / Σ学分 （注：百分制均分为百分制成绩的学分加权）。</p>
      </div>
    </section>
  </div>
</template>

<style scoped>
.scholar-view {
  padding: 2rem 2.5rem 6rem;
  max-width: 900px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

/* Header */
.scholar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.scholar-header h1 {
  font-size: 1.8rem;
  font-weight: 700;
  margin: 0;
  background: linear-gradient(135deg, #e2e8f0, #f8fafc);
  background-clip: text;
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
}
.update-badge {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 0.8rem;
  color: #94a3b8;
}
.dot {
  width: 8px; height: 8px;
  border-radius: 50%;
  display: inline-block;
}
.dot.green { background: #22c55e; box-shadow: 0 0 6px #22c55e88; }
.dot.orange { background: #f59e0b; box-shadow: 0 0 6px #f59e0b88; }

/* Offline Banner */
.offline-banner {
  background: rgba(245, 158, 11, 0.15);
  border: 1px solid rgba(245, 158, 11, 0.3);
  color: #fcd34d;
  padding: 12px 16px;
  border-radius: 16px;
  margin-bottom: 20px;
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 0.85rem;
  line-height: 1.4;
  backdrop-filter: blur(12px);
  animation: fade-in 0.4s ease-out;
}
.offline-icon {
  font-size: 1.3rem;
  animation: pulse-warn 2s infinite;
}
.offline-text strong {
  display: block;
  color: #fbbf24;
  margin-bottom: 2px;
}

@keyframes pulse-warn {
  0%, 100% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.7; transform: scale(1.1); }
}


/* Section Card */
.section-card {
  background: rgba(255,255,255,0.04);
  border: 1px solid rgba(255,255,255,0.08);
  border-radius: 20px;
  padding: 1.5rem;
  backdrop-filter: blur(12px);
  transition: transform 0.2s, box-shadow 0.2s;
}
.section-card:hover {
  transform: translateY(-2px);
  box-shadow: 0 8px 24px rgba(0,0,0,0.25);
}

.section-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 1.2rem;
}
.section-icon { font-size: 1.2rem; }
.section-title {
  font-size: 1.15rem;
  font-weight: 700;
  color: var(--text-main);
}
.semester-badge {
  margin-left: auto;
  font-size: 0.75rem;
  background: rgba(56,189,248,0.15);
  color: #38bdf8;
  padding: 3px 10px;
  border-radius: 20px;
  font-weight: 600;
}

/* Grade Grid */
.grade-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 10px;
}
.grade-card {
  background: color-mix(in srgb, var(--accent) 10%, transparent);
  border: 1px solid color-mix(in srgb, var(--accent) 20%, transparent);
  border-radius: 14px;
  padding: 1rem;
  display: flex;
  flex-direction: column;
  gap: 6px;
  transition: transform 0.2s, background 0.2s;
  cursor: default;
}
.grade-card:hover {
  transform: scale(1.03);
  background: color-mix(in srgb, var(--accent) 18%, transparent);
}
.grade-label {
  font-size: 0.78rem;
  color: #94a3b8;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}
.grade-value {
  font-size: 1.5rem;
  font-weight: 800;
  color: var(--accent);
  font-variant-numeric: tabular-nums;
}

/* Stats Row */
.stats-row {
  display: flex;
  justify-content: space-around;
  margin-bottom: 1rem;
}
.stat-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
}
.stat-value {
  font-size: 1.5rem;
  font-weight: 800;
  color: var(--text-main);
  font-variant-numeric: tabular-nums;
}
.stat-value.highlight-red { color: #ef4444; }
.stat-value.highlight-yellow { color: #eab308; }
.stat-label {
  font-size: 0.78rem;
  color: var(--text-muted);
  font-weight: 600;
}

/* Semester Cards */
.semester-cards {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}
.semester-half {
  border-radius: 14px;
  padding: 1rem;
  display: flex;
  flex-direction: column;
  gap: 4px;
  cursor: pointer;
  transition: transform 0.2s;
}
.semester-half:hover { transform: scale(1.03); }
.semester-half.spring {
  background: linear-gradient(135deg, rgba(34,197,94,.15), rgba(34,197,94,.05));
  border: 1px solid rgba(34,197,94,.2);
}
.semester-half.summer {
  background: linear-gradient(135deg, rgba(234,179,8,.15), rgba(234,179,8,.05));
  border: 1px solid rgba(234,179,8,.2);
}
.half-label { font-size: 0.82rem; color: var(--text-muted); font-weight: 600; }
.half-value { font-size: 1.15rem; color: var(--text-main); font-weight: 700; }

/* Todo Scroll */
.todo-scroll {
  display: flex;
  gap: 10px;
  overflow-x: auto;
  padding-bottom: 6px;
  scrollbar-width: thin;
  scrollbar-color: rgba(255,255,255,.1) transparent;
}
.todo-card {
  min-width: 180px;
  background: rgba(255,255,255,0.04);
  border: 1px solid rgba(255,255,255,0.08);
  border-radius: 14px;
  padding: 1rem;
  display: flex;
  flex-direction: column;
  gap: 6px;
  flex-shrink: 0;
  transition: transform 0.2s;
}
.todo-card.exam-card {
  border: 1px solid rgba(245, 158, 11, 0.2);
  background: linear-gradient(135deg, rgba(245, 158, 11, 0.1), rgba(245, 158, 11, 0.02));
  box-shadow: 0 4px 12px rgba(0,0,0,0.1);
}
.todo-card.exam-card:hover {
  transform: translateY(-2px);
  box-shadow: 0 6px 16px rgba(245, 158, 11, 0.15);
}
.todo-card:hover { transform: translateY(-2px); }
.todo-card.urgent {
  border-color: rgba(239,68,68,0.3);
  background: rgba(239,68,68,0.06);
}
.todo-name { font-weight: 700; font-size: 0.9rem; color: var(--text-main); }
.todo-course { font-size: 0.78rem; color: var(--text-muted); }
.todo-deadline { font-size: 0.78rem; color: var(--text-muted); font-weight: 600; }
.todo-card.urgent .todo-deadline { color: #ef4444; }

@media (max-width: 600px) {
  .scholar-view { padding: 1rem 1rem 6rem; }
  .grade-grid { grid-template-columns: repeat(2, 1fr); }
  .stats-row { flex-wrap: wrap; gap: 10px; }
}

.semester-scroll-nav {
  display: flex;
  gap: 8px;
  overflow-x: auto;
  padding-bottom: 12px;
  margin-bottom: 16px;
  scrollbar-width: none; /* Firefox */
}
.semester-scroll-nav::-webkit-scrollbar {
  display: none; /* Chrome, Safari, Opera */
}
.sem-nav-btn {
  background: rgba(255,255,255,0.05);
  border: 1px solid rgba(255,255,255,0.1);
  color: #94a3b8;
  padding: 6px 14px;
  border-radius: 20px;
  font-size: 0.85rem;
  font-weight: 600;
  cursor: pointer;
  white-space: nowrap;
  transition: all 0.2s;
}
.sem-nav-btn:hover {
  background: rgba(255,255,255,0.1);
}
.sem-nav-btn.active {
  background: rgba(56,189,248,0.15);
  border-color: rgba(56,189,248,0.3);
  color: #38bdf8;
}

.custom-gpa-toggle {
  background: rgba(255,255,255,0.05);
  border: 1px solid rgba(255,255,255,0.1);
  color: #94a3b8;
  padding: 4px 12px;
  border-radius: 6px;
  font-size: 0.8rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
}
.custom-gpa-toggle:hover {
  background: rgba(255,255,255,0.1);
}
.custom-gpa-toggle.active {
  background: rgba(168,85,247,0.15);
  border-color: rgba(168,85,247,0.3);
  color: #a855f7;
}

.custom-checkbox {
  width: 16px;
  height: 16px;
  border: 2px solid #64758b;
  border-radius: 4px;
  display: inline-block;
  position: relative;
  transition: all 0.2s;
}
.custom-checkbox.checked {
  background: #a855f7;
  border-color: #a855f7;
}
.custom-checkbox.checked::after {
  content: '';
  position: absolute;
  left: 4px;
  top: 1px;
  width: 4px;
  height: 8px;
  border: solid white;
  border-width: 0 2px 2px 0;
  transform: rotate(45deg);
}

.course-list-wrap {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-top: 20px;
  border-top: 1px solid rgba(255,255,255,0.05);
  padding-top: 20px;
}
.course-item-card {
  background: rgba(0,0,0,0.2);
  border-radius: 12px;
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  transition: opacity 0.2s, background 0.2s;
}
.course-item-card.dimmed {
  opacity: 0.4;
}
.course-item-card:hover {
  background: rgba(255,255,255,0.02);
}
.course-item-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.course-item-name {
  font-weight: 700;
  color: #f1f5f9;
  font-size: 1rem;
}
.course-item-score {
  font-weight: 800;
  color: #22c55e;
  font-size: 1.2rem;
}
.course-item-score.failed {
  color: #ef4444;
}
.simulated-input {
  background: rgba(255, 255, 255, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.2);
  color: #38bdf8;
  border-radius: 6px;
  padding: 4px 8px;
  width: 80px;
  font-weight: 700;
  text-align: right;
  outline: none;
}
.simulated-input:focus {
  border-color: #38bdf8;
}
.course-item-details {
  display: flex;
  gap: 12px;
  font-size: 0.8rem;
  color: #64758b;
  flex-wrap: wrap;
}

.rule-content {
  font-size: 0.85rem;
  color: #cbd5e1;
  line-height: 1.6;
  background: rgba(0,0,0,0.15);
  padding: 1rem;
  border-radius: 12px;
}
.rule-content p {
  margin: 0 0 8px 0;
}
.rule-content p:last-child {
  margin-bottom: 0;
}
.rule-content strong {
  color: #f1f5f9;
}
.formula {
  margin-top: 12px !important;
  color: #94a3b8 !important;
  border-top: 1px dotted rgba(255,255,255,0.1);
  padding-top: 8px;
}.rule-content {
  font-size: 0.85rem;
  color: #cbd5e1;
  line-height: 1.6;
  background: rgba(0,0,0,0.15);
  padding: 1rem;
  border-radius: 12px;
}
.rule-content p {
  margin: 0 0 8px 0;
}
.rule-content p:last-child {
  margin-bottom: 0;
}
.rule-content strong {
  color: #f1f5f9;
}
.formula {
  margin-top: 12px !important;
  color: #94a3b8 !important;
  border-top: 1px dotted rgba(255,255,255,0.1);
  padding-top: 8px;
}

/* ── ScholarView Light Mode Overrides ── */
:root.light-theme .scholar-header h1 {
  color: #1e293b;
}
:root.light-theme .offline-banner {
  background: rgba(245, 158, 11, 0.1);
  color: #d97706;
  border-color: rgba(245, 158, 11, 0.3);
}
:root.light-theme .offline-text strong {
  color: #b45309;
}
:root.light-theme .loading-state {
  color: #64748b;
}
:root.light-theme .section-card {
  background: rgba(255, 255, 255, 0.7);
  border-color: rgba(0, 0, 0, 0.08);
  box-shadow: 0 4px 12px rgba(0,0,0,0.03);
}
:root.light-theme .section-title {
  color: #1e293b;
}
:root.light-theme .action-btn {
  color: #64748b;
}
:root.light-theme .action-btn:hover {
  background: rgba(0,0,0,0.05);
  color: #1e293b;
}
:root.light-theme .grade-card {
  background: color-mix(in srgb, var(--accent) 15%, #fff);
  border-color: color-mix(in srgb, var(--accent) 25%, #fff);
}
:root.light-theme .grade-label {
  color: #64748b;
}
:root.light-theme .sem-nav {
  border-bottom-color: rgba(0,0,0,0.05);
}
:root.light-theme .sem-nav-btn {
  color: #64748b;
}
:root.light-theme .sem-nav-btn:hover {
  background: rgba(0,0,0,0.05);
  color: #1e293b;
}
:root.light-theme .sem-nav-btn.active {
  background: #e0f2fe;
  color: #0284c7;
}
:root.light-theme .stat-item {
  background: transparent;
}
:root.light-theme .stat-value {
  color: #1e293b;
}
:root.light-theme .highlight-red { color: #dc2626 !important; }
:root.light-theme .stat-label {
  color: #64748b;
}
:root.light-theme .course-card {
  background: rgba(0,0,0,0.03);
  border-color: rgba(0,0,0,0.05);
}
:root.light-theme .course-top {
  border-bottom-color: rgba(0,0,0,0.05);
}
:root.light-theme .course-name {
  color: #1e293b;
}
:root.light-theme .course-score {
  color: #16a34a;
}
:root.light-theme .course-score.low-score {
  color: #dc2626;
}
:root.light-theme .course-info span {
  color: #64748b;
}
:root.light-theme .exam-card {
  background: rgba(255,255,255,0.8);
  border-color: rgba(0,0,0,0.08);
}
:root.light-theme .exam-title {
  color: #1e293b;
}
:root.light-theme .exam-title .exam-type {
  background: rgba(234, 88, 12, 0.1);
  color: #ea580c;
}
:root.light-theme .exam-time {
  color: #1e293b;
}
:root.light-theme .exam-loc,
:root.light-theme .exam-seat {
  color: #64748b;
}
:root.light-theme .exam-days {
  color: #64748b;
}
:root.light-theme .exam-days.urgent {
  color: #dc2626;
  background: rgba(220, 38, 38, 0.1);
}
:root.light-theme .exam-days.soon {
  color: #ea580c;
  background: rgba(234, 88, 12, 0.1);
}
:root.light-theme .exam-days.future {
  color: #0284c7;
  background: rgba(2, 132, 199, 0.1);
}
</style>
