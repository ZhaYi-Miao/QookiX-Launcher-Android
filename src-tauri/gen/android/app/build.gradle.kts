import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

android {
    compileSdk = 36
    namespace = "com.zhayi.qookix"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        applicationId = "com.zhayi.qookix"
        minSdk = 24
        targetSdk = 36
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
    }
    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
                jniLibs.keepDebugSymbols.add("*/armeabi-v7a/*.so")
                jniLibs.keepDebugSymbols.add("*/x86/*.so")
                jniLibs.keepDebugSymbols.add("*/x86_64/*.so")
            }
        }
        getByName("release") {
            isMinifyEnabled = true
            proguardFiles(
                *fileTree(".") { include("**/*.pro") }
                    .plus(getDefaultProguardFile("proguard-android-optimize.txt"))
                    .toList().toTypedArray()
            )
            // CI 签名：密钥经 GitHub Secrets 注入（QOOKIX_KEYSTORE 等 4 个环境变量）；
            // 本地/无密钥环境退回 debug 签名，保证产物可直接安装。
            // 密钥文件绝不入库 —— 只存在于本机 ~/.android 与 CI 的临时目录。
            val ksPath = System.getenv("QOOKIX_KEYSTORE") ?: ""
            if (ksPath.isNotEmpty() && file(ksPath).exists()) {
                signingConfig = signingConfigs.create("ci") {
                    storeFile = file(ksPath)
                    storePassword = System.getenv("QOOKIX_KEYSTORE_PASSWORD")
                    keyAlias = System.getenv("QOOKIX_KEY_ALIAS")
                    keyPassword = System.getenv("QOOKIX_KEY_PASSWORD")
                }
            } else {
                signingConfig = signingConfigs.getByName("debug")
            }
        }
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
    buildFeatures {
        buildConfig = true
    }
    // Pojav 版 JNI 核心（libpojavexec.so）：Surface→EGL 桥 + GL 函数转发 + 输入注册。
    // 源码来自 PojavLauncher，模块名必须保持 pojavexec。
    externalNativeBuild {
        ndkBuild {
            path = file("src/main/jni/Android.mk")
        }
    }
}

rust {
    rootDirRel = "../../../"
}

dependencies {
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.10.0")
    implementation("androidx.core:core-ktx:1.13.1")

    // ── Pojav 控制层（net.kdt.pojavlaunch.customcontrols.*）所需 ──────────────
    // 游戏内菜单抽屉：activity_basemain.xml 的根布局是 DrawerLayout
    implementation("androidx.drawerlayout:drawerlayout:1.1.1")
    // 控制编辑器/日志浮层：activity_custom_controls.xml、view_logger.xml 用 ConstraintLayout
    implementation("androidx.constraintlayout:constraintlayout:2.1.4")
    // 控制布局 JSON（ControlData/CustomControls 的序列化格式，字段名即磁盘格式）
    implementation("com.google.code.gson:gson:2.10.1")
    // Pojav 的 res/values/styles.xml 里 PreferenceThemeOverlay.v14.Material / preferenceTheme
    // 来自 androidx.preference（上游同样声明了它）
    implementation("androidx.preference:preference:1.2.0")
    // exp4j：控制按钮动态位置表达式求值（${margin} * 2 + ${width} 这类）
    // 该 jar 来自 PojavLauncher/app_pojavlauncher/libs（PojavLauncherTeam 的 exp4j fork）；
    // 本机无法访问 Maven 仓库，故以本地 jar 形式内置。
    implementation(files("libs/exp4j-0.4.9-SNAPSHOT.jar"))

    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = "tauri.build.gradle.kts")