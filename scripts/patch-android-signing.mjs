#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";

const ROOT = process.cwd();
const target = path.join(ROOT, "src-tauri", "gen", "android", "app", "build.gradle.kts");

if (!fs.existsSync(target)) {
  console.log("android signing patch skipped: src-tauri/gen/android/app/build.gradle.kts not found");
  process.exit(0);
}

const original = fs.readFileSync(target, "utf8");
let content = original;

const requiredSigningEnvBlock = `val requiredSigningEnv = listOf(
    "ANDROID_KEYSTORE_PATH",
    "ANDROID_KEYSTORE_PASSWORD",
    "ANDROID_KEY_ALIAS",
    "ANDROID_KEY_PASSWORD",
)`;

const releaseCheckBlock = `gradle.taskGraph.whenReady {
    val needsReleaseSigning = allTasks.any { task ->
        val name = task.name
        val isPackagingTask = name.startsWith("assemble", ignoreCase = true)
            || name.startsWith("bundle", ignoreCase = true)
            || name.startsWith("package", ignoreCase = true)
        isPackagingTask && name.contains("Release", ignoreCase = true)
    }

    if (needsReleaseSigning) {
        val missing = requiredSigningEnv.filter { System.getenv(it).isNullOrBlank() }
        if (missing.isNotEmpty()) {
            throw GradleException(
                "Missing Android release signing env vars: \${missing.joinToString(\", \")}"
            )
        }
    }
}
`;

if (!content.includes('import org.gradle.api.GradleException')) {
  content = content.replace(
    'import java.util.Properties',
    'import java.util.Properties\nimport org.gradle.api.GradleException',
  );
}

if (!content.includes("val requiredSigningEnv = listOf(")) {
  const marker = /(val tauriProperties = Properties\(\)\.apply \{[\s\S]*?\n\})\n\nandroid \{/m;
  if (marker.test(content)) {
    content = content.replace(marker, `$1\n\n${requiredSigningEnvBlock}\n\nandroid {`);
  } else {
    console.error("android signing patch failed: cannot locate tauriProperties block");
    process.exit(1);
  }
}

const eagerCheckPattern =
  /            val requiredEnv = listOf\([\s\S]*?            }\n            signingConfig = signingConfigs\.getByName\("release"\)\n/m;
if (eagerCheckPattern.test(content)) {
  content = content.replace(
    eagerCheckPattern,
    '            signingConfig = signingConfigs.getByName("release")\n',
  );
}

if (!content.includes("gradle.taskGraph.whenReady")) {
  const rustMarker = /\nrust \{/m;
  if (rustMarker.test(content)) {
    content = content.replace(rustMarker, `\n${releaseCheckBlock}\nrust {`);
  } else {
    console.error("android signing patch failed: cannot locate rust block");
    process.exit(1);
  }
}

if (content !== original) {
  fs.writeFileSync(target, content);
  console.log("android signing patch applied");
} else {
  console.log("android signing patch already up to date");
}
