# macOS + Android 发布流程

## 0. 版本源

唯一版本源：`src-tauri/tauri.conf.json` 的 `version`。

执行同步：

```bash
npm run release:sync-version
```

会同步到：

- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/gen/android/app/tauri.properties`
- `src-tauri/gen/apple/project.yml`

## 1. 本地出包

```bash
npm run release:mac
npm run release:android
npm run release:android:test
npm run build:android:debug
npm run release:ios
npm run release:all
```

策略：macOS + Android 自动出包，人工分发或上传；iOS 保留显式单独命令，但不进入本轮 release gate。

- `release:android`：上架包（APK + AAB），要求 release 签名环境变量。
- `release:android:test`：真机测试优先方案，生成 release 签名的 arm64 APK，体积和安装行为更接近正式版。
- `build:android:debug`：仅在需要调试器、临时免 release 签名或排查构建问题时使用，不作为默认测试发包。

## 2. Android 签名

Release 必须注入环境变量：

- `ANDROID_KEYSTORE_PATH`
- `ANDROID_KEYSTORE_PASSWORD`
- `ANDROID_KEY_ALIAS`
- `ANDROID_KEY_PASSWORD`

缺失任意一项会直接失败，禁止回退到 debug keystore。

## 3. CI 出包

`release-pack.yml` 在 `v*` tag 触发，GitHub Release 标题严格等于 tag，并上传 artifacts：

- `Celechron-${tag}-mac`
- `Celechron-${tag}-android`

iOS 保留 `release:ios` 本地显式命令，但不作为本轮 CI 阻断项。

## 4. Release Manifest

每次 `npm run build` / `npm run release:*` 会生成 `dist/release-manifest.json`，字段至少包含：

- `version`
- `tag`
- `title`
- `channel`
- `commit`
- `artifactPrefix`

其中：

- stable tag 形如 `vX.Y.Z`
- dev tag 形如 `vX.Y.Z-dev-YYYYMMDD-HHMM`
- GitHub Release 标题必须与 `tag` 完全一致
