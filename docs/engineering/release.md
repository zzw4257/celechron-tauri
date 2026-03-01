# 三端发布流程

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
npm run build:android:debug
npm run release:ios
npm run release:all
```

策略：自动出包，人工上传商店。

- `release:android`：上架包（APK + AAB），要求 release 签名环境变量。
- `build:android:debug`：测试包（debug 签名，仅 APK），不要求 release 签名环境变量。

## 2. Android 签名

Release 必须注入环境变量：

- `ANDROID_KEYSTORE_PATH`
- `ANDROID_KEYSTORE_PASSWORD`
- `ANDROID_KEY_ALIAS`
- `ANDROID_KEY_PASSWORD`

缺失任意一项会直接失败，禁止回退到 debug keystore。

## 3. CI 出包

`release-pack.yml` 在 `v*` tag 触发并上传 artifacts：

- mac bundle
- android apk/aab
- ios ipa/xcarchive/app
