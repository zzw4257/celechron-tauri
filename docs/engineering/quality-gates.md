# 质量门禁

## 本地门禁

- `commit-msg`：`commitlint`
- `pre-commit`：`npm run typecheck` + `npm run check:colors`
- `pre-push`：`npm run check`

## CI 门禁

`quality.yml` 执行：

```bash
npm ci
npm run release:sync-version
npm run check
```

`npm run check` 包含：

- TypeScript 类型检查
- 前端构建
- 颜色硬编码基线检查
- Rust `cargo check --lib` + `cargo check --bin celechron`

## 颜色检查说明

`check-colors.mjs` 采用“基线不增长”策略：

- 现有历史硬编码允许存在（短期兼容）
- 新增硬编码会阻断
- 需要升级基线时，显式执行：

```bash
node scripts/check-colors.mjs --update-baseline
```
