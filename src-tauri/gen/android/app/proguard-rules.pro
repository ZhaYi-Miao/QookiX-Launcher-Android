# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.
#
# For more details, see
#   http://developer.android.com/guide/developing/tools/proguard.html

# If your project uses WebView with JS, uncomment the following
# and specify the fully qualified class name to the JavaScript interface
# class:
#-keepclassmembers class fqcn.of.javascript.interface.for.webview {
#   public *;
#}

# Uncomment this to preserve the line number information for
# debugging stack traces.
#-keepattributes SourceFile,LineNumberTable

# If you keep the line number information, uncomment this to
# hide the original source file name.
#-renamesourcefileattribute SourceFile
# ── JNI 反查的类必须保留原名 ──────────────────────────────────────────
# 这些类是被 C 层用 FindClass("...") 反查的。release 构建开了 R8（isMinifyEnabled=true），
# 一旦被改名，FindClass 返回 NULL，紧接着的 GetMethodID(NULL, ...) 会直接原生崩溃 ——
# 这正是「debug 能跑、release 崩」的原因（例如 net/kdt/pojavlaunch/Logger$eventLogListener，
# 它自己没有 native 方法，所以不在 AGP 默认的 -keepclasseswithmembernames 规则里）。
-keep class net.kdt.pojavlaunch.** { *; }
-keep class com.kdt.** { *; }
-keep class org.lwjgl.glfw.** { *; }
-keep class com.zhayi.qookix.** { *; }
# Java 层被 JNI 调用的方法名同样要保留（GetMethodID 按名字查找）
-keepclasseswithmembernames class * { native <methods>; }
