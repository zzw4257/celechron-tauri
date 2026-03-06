# 开发规范（长期）

## 1. 提交信息

强制使用 Conventional Commits：

```text
<type>(<scope>): <subject>
```

允许 `type`：

- `feat`
- `fix`
- `docs`
- `style`
- `refactor`
- `perf`
- `test`
- `build`
- `ci`
- `chore`
- `revert`

允许 `scope`：

- `ui`
- `theme`
- `gpa`
- `tauri`
- `android`
- `ios`
- `mac`
- `release`
- `ci`
- `docs`
- `security`

禁止：

- emoji 前缀（如 `fix(📅): ...`）
- phase 风格（如 `feat(P3): ...`）

## 2. GPA 单一真值

- GPA 标准口径只由 Rust 后端提供。
- 前端标准展示必须读取 `gpaByPolicy`。
- DIY 模拟通过 `calculate_gpa_preview` 调用后端统一算法。

## 3. 主题与样式

- 新增样式优先使用 `src/styles/tokens.css` 中的语义 token。
- 不允许增加硬编码色值（由 `scripts/check-colors.mjs` + baseline 门禁控制）。

## 4. 凭据与安全

- 测试账号仅用于本地开发环境，禁止写入源码、脚本、配置。
- 不允许提交任何账号密码、证书、私钥到 Git。


## 5. 本地 Smoke

- `npm run smoke:local` 仅用于本地开发核验。
- 必须通过环境变量注入 `ZJU_USERNAME` / `ZJU_PASSWORD`。
- Smoke 输出摘要可用于人工比对，但不得提交到仓库。
