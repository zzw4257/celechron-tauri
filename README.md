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
| **📚 资料中台** | 同步学在浙大当前课程资料索引，支持本地缓存、预览、搜索与离线打开 |
| **🤖 AI 综合分析** | 通过 ZeroClaw 对学业数据和已缓存资料做中文摘要、风险提示与复习建议 |
| **🔔 DingTalk 通知** | 支持 webhook 测试消息、成绩更新提醒、资料同步提醒 |
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

登录后可直接在“资料”页点击“同步远程资料”，拉取学在浙大当前课程的课件索引；已缓存资料支持本地预览和 ZeroClaw 分析。

### Build

```bash
# 构建 macOS 包
npm run release:mac

# 构建 Android 包 (apk + aab)
npm run release:android

# 构建 Android 测试包 (release 签名，arm64，仅 APK，推荐手机真机测试)
npm run release:android:test

# 仅在需要调试器/免签安装时才使用 debug APK
npm run build:android:debug

# 构建 iOS 包（非本轮 release gate，可单独执行）
npm run release:ios
```

统一发布（macOS + Android）：`npm run release:all`

版本同步：`npm run release:sync-version`

每次构建会额外生成 `dist/release-manifest.json`，记录 tag / channel / commit / artifact 前缀。

Android 本地 release 打包需要以下环境变量（包括 `release:android` 和 `release:android:test`，缺失会被阻断）：

- `ANDROID_KEYSTORE_PATH`
- `ANDROID_KEYSTORE_PASSWORD`
- `ANDROID_KEY_ALIAS`
- `ANDROID_KEY_PASSWORD`

本地 smoke 校验（仅开发环境，凭据只走环境变量）：

```bash
ZJU_USERNAME=你的学号 ZJU_PASSWORD=你的密码 npm run smoke:local
```

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

登录凭据**不会**硬编码在源码中。运行集成测试或本地 smoke 时通过环境变量传入：

```bash
ZJU_USERNAME=你的学号 ZJU_PASSWORD=你的密码 cargo test
ZJU_USERNAME=你的学号 ZJU_PASSWORD=你的密码 npm run smoke:local
```

可选的集成配置都在应用内“设置”页完成，不写入仓库：

- `ZeroClaw Endpoint`：外部 ZeroClaw HTTP 服务地址，例如 `https://your-host/api/analyze`
- `ZeroClaw API Key`：如你的 ZeroClaw gateway 需要 Bearer Token，则填这里
- `DingTalk Webhook`：群机器人地址
- `DingTalk Secret`：若机器人开启签名，则一并填写

资料同步不依赖额外凭据，只要求当前账号已成功登录学在浙大。

## 🙏 Credits

- [Celechron](https://github.com/Celechron/Celechron) — 原始 Flutter 版本的灵感来源与 GPA / 学期规则参考
- [zju-learning-assistant](https://github.com/PeiPei233/zju-learning-assistant/) — 资料同步、提醒与课程平台能力 donor
- [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw) — 外部 AI 服务接入目标
- [Tauri](https://tauri.app/) — 轻量级跨平台桌面框架
- [Vue.js](https://vuejs.org/) — 渐进式前端框架
- [ECharts](https://echarts.apache.org/) — 数据可视化引擎

## 📜 License

MIT © [zzw4257](https://github.com/zzw4257)
