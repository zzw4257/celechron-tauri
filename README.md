<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="96" alt="Celechron" />
</p>

<h1 align="center">Celechron</h1>

<p align="center">
  <strong>浙大本科生时间管理 & 学业仪表盘</strong><br/>
  <sub>A Tauri app for macOS / Android / iOS built for ZJU undergrads.</sub>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Tauri-v2-24C8D8?style=flat-square&logo=tauri&logoColor=white" />
  <img src="https://img.shields.io/badge/Vue-3.5-4FC08D?style=flat-square&logo=vue.js&logoColor=white" />
  <img src="https://img.shields.io/badge/Rust-2024-DEA584?style=flat-square&logo=rust&logoColor=white" />
  <img src="https://img.shields.io/badge/TypeScript-5.6-3178C6?style=flat-square&logo=typescript&logoColor=white" />
  <img src="https://img.shields.io/badge/ECharts-5-AA344D?style=flat-square&logo=apache-echarts&logoColor=white" />
  <img src="https://img.shields.io/badge/Platform-macOS%20%7C%20Android%20%7C%20iOS-111?style=flat-square" />
</p>

---

## ✨ Features

| Module | Description |
|--------|-------------|
| **📊 学业仪表盘** | 全维度 GPA 总览（五分 / 4.3 / 4.0 / 百分制），精准区分主修与辅修学分 |
| **📈 均绩趋势图** | 基于 ECharts 的交互式折线图，展示各学期 GPA 走势 |
| **🎛️ DIY 均绩模拟** | 勾选 / 排除任意课程，即时预估 GPA 变动；支持给"待录"科目模拟分数 |
| **📅 智能课表** | 周视图 + 月视图，自动解析单双周、课程冲突叠层展示 |
| **✅ 作业追踪** | 接入学在浙大获取待办列表，按截止日倒计时高亮 |
| **📝 考试安排** | 自动拉取并过滤已结束考试，展示时间 / 地点 / 座位号 |
| **🌗 深浅双主题** | 全局 Light / Dark 模式，毛玻璃质感 + 微动画 |
| **📤 CSV 导出** | 一键导出带时间戳的完整成绩单 |

## 🏗️ Architecture

```
celechron-tauri/
├── src/                    # Vue 3 前端
│   ├── components/
│   │   ├── Login.vue       # ZJU 统一认证登录
│   │   ├── MainLayout.vue  # 导航框架
│   │   └── views/
│   │       ├── ScholarView  # 学业 · GPA · 成绩
│   │       ├── CalendarView # 课表 · 日程
│   │       ├── TaskView     # 待办管理
│   │       ├── FlowView     # 时间流
│   │       └── OptionView   # 设置 · 主题
│   └── composables/        # 可复用逻辑 (useTheme 等)
└── src-tauri/              # Rust 后端
    └── src/
        ├── zjuam.rs        # ZJU 统一认证 (RSA 登录)
        ├── zdbk.rs         # 教务系统 API 对接
        └── main.rs         # Tauri 命令桥接
```

## 🚀 Quick Start

### Prerequisites

- [Node.js](https://nodejs.org/) ≥ 18
- [Rust](https://rustup.rs/) (stable)
- [Tauri CLI v2](https://v2.tauri.app/)

### Development

```bash
# 安装前端依赖
npm install

# 启动开发服务器 (Vite + Tauri)
npm run tauri dev
```

### Build

```bash
# 构建 macOS 包
npm run release:mac

# 构建 Android 包 (apk + aab)
npm run release:android

# 构建 Android 测试包 (debug 签名，仅 APK)
npm run build:android:debug

# 构建 iOS 包
npm run release:ios
```

统一三端构建：`npm run release:all`

版本同步：`npm run release:sync-version`

Android 本地 release 打包需要以下环境变量（缺失会被阻断）：

- `ANDROID_KEYSTORE_PATH`
- `ANDROID_KEYSTORE_PASSWORD`
- `ANDROID_KEY_ALIAS`
- `ANDROID_KEY_PASSWORD`

## ✅ Quality Gates

```bash
# 全量质量检查（本地/CI一致）
npm run check
```

门禁策略与提交规范文档见：

- `docs/engineering/conventions.md`
- `docs/engineering/quality-gates.md`
- `docs/engineering/release.md`

## ⚙️ Configuration

登录凭据**不会**硬编码在源码中。运行集成测试时通过环境变量传入：

```bash
ZJU_USERNAME=你的学号 ZJU_PASSWORD=你的密码 cargo test
```

## 🙏 Credits

- [Celechron](https://github.com/Celechron/Celechron) — 原始 Flutter 版本的灵感来源与 API 参考
- [Tauri](https://tauri.app/) — 轻量级跨平台桌面框架
- [Vue.js](https://vuejs.org/) — 渐进式前端框架
- [ECharts](https://echarts.apache.org/) — 数据可视化引擎

## 📜 License

MIT © [zzw4257](https://github.com/zzw4257)
