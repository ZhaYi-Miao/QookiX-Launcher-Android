# QookiX-Launcher-Android 需求规格文档

> 版本：v1.0-draft  
> 日期：2026-09-05  
> 状态：待评审

---

## 1. 项目概述

### 1.1 项目定位

QookiX-Launcher-Android 是 QookiX-Launcher 电脑端 Minecraft 启动器的安卓移植版本，目标是在 Android 设备上提供与电脑端一致的游戏管理体验，同时充分利用安卓平台的触摸交互特性。

### 1.2 目标用户

- 需要在安卓设备上游玩 Minecraft 的玩家
- 已使用 QookiX-Launcher 电脑端的老用户（跨平台同步需求）
- 偏好 Mod/整合包管理的进阶玩家

### 1.3 核心价值

| 价值点 | 说明 |
|--------|------|
| 跨平台体验 | 与电脑端 QookiX 保持一致的操作逻辑和数据结构 |
| 完整生态 | 支持 Modrinth/CurseForge 内容浏览与安装 |
| 多 Java 运行时 | 支持 JRE 8/17/21/25，自动匹配游戏版本 |
| 触摸优化 | 专为触摸屏设计的自定义控制方案 |

---

## 2. 技术栈选型

### 2.1 前端（安卓原生 UI）

| 层级 | 技术 | 选型理由 |
|------|------|----------|
| 语言 | **Kotlin 1.9+** | 安卓官方首选语言，与 Java 互操作 |
| UI 框架 | **Jetpack Compose** | 现代声明式 UI，与 QookiX 的 Vue 响应式理念一致 |
| 导航 | **Navigation Compose** | 类型安全的路由，对应 Vue Router |
| 状态管理 | **MVI / ViewModel + Coroutines Flow** | 单向数据流，对应 Pinia |
| 依赖注入 | **Hilt (Dagger)** | 官方推荐 DI 框架 |
| 网络 | **OkHttp + Retrofit** | 成熟稳定的 HTTP 客户端 |
| JSON | **Kotlinx Serialization** | 多平台 Kotlin 序列化库 |
| 图片加载 | **Coil** | 原生 Compose 图片加载 |

### 2.2 后端（游戏引擎层）

| 层级 | 技术 | 选型理由 |
|------|------|----------|
| Java 运行时 | **OpenJDK 8/17/21/25 (MultiRT)** | 参考 PojavLauncher/Amethyst 方案 |
| 图形库 | **LWJGL 3 + OpenGL ES** | 安卓上运行 Minecraft 的标准方案 |
| 窗口系统 | **GLFW (Android 移植)** | 参考 Pojav 的 CallbackBridge |
| AWT 模拟 | **Caciocavallo** | 在 Android 上实现 AWT/Swing |
| 原生层 | **C/C++ (NDK)** | JNI 桥接、OpenGL 上下文管理 |
| 构建系统 | **Gradle 9 + AGP 9** | 与 Amethyst 一致 |

### 2.3 与 QookiX-Launcher 电脑端的复用

| 模块 | 复用方式 |
|------|----------|
| 数据模型 | 共享 TypeScript 类型定义 → Kotlin data class |
| API 层 | 复用 Modrinth/CurseForge API 接口定义 |
| Rust 后端逻辑 | 通过 Tauri IPC 或独立 Rust 库复用 |
| 配置结构 | 保持 `launcher_profiles.json` 格式兼容 |
| 账号数据 | 共享 `accounts.json` 格式 |

---

## 3. 功能需求

### 3.1 核心功能模块

#### 模块 1：版本管理

| 功能 | 优先级 | 说明 |
|------|--------|------|
| 获取版本清单 | P0 | 从 Mojang 镜像获取 `version_manifest_v2.json` |
| 版本列表展示 | P0 | 显示所有正式版/快照版，支持筛选 |
| 版本详情查看 | P0 | 显示版本号、类型、发布时间 |
| 加载器版本管理 | P0 | Fabric/Quilt/Forge/NeoForge 版本选择与安装 |
| 版本继承处理 | P0 | 正确解析 `inheritsFrom`，合并库和参数 |
| 版本缓存 | P1 | 本地缓存 24 小时，减少网络请求 |
| 离线模式 | P1 | 使用本地缓存版本列表 |

#### 模块 2：账号管理

| 功能 | 优先级 | 说明 |
|------|--------|------|
| 离线账号 | P0 | 输入用户名，生成 UUID |
| Microsoft 账号登录 | P0 | OAuth 设备码流程（5步认证） |
| 账号切换 | P0 | 多账号列表，快速切换 |
| 账号编辑 | P1 | 修改用户名、皮肤 |
| Token 自动刷新 | P0 | 检测 `expiresAt`，自动刷新 |
| 皮肤显示 | P1 | 3D 皮肤预览（参考 skinview3d） |

**Microsoft 认证流程（与 Amethyst 一致）：**

```
1. POST login.live.com/oauth20_token.srf → Access Token
2. POST user.auth.xboxlive.com/user/authenticate → XBL Token
3. POST xsts.auth.xboxlive.com/xsts/authorize → XSTS Token + UHS
4. POST api.minecraftservices.com/authentication/login_with_xbox → Minecraft Token
5. GET api.minecraftservices.com/minecraft/profile → 玩家 UUID + 用户名
```

#### 模块 3：实例管理

| 功能 | 优先级 | 说明 |
|------|--------|------|
| 创建实例 | P0 | 选择版本、加载器、配置参数 |
| 删除实例 | P0 | 确认删除，可选保留游戏目录 |
| 实例列表 | P0 | 卡片式展示，支持分组 |
| 实例详情 | P1 | 版本信息、Mod 列表、游戏参数 |
| 分组管理 | P1 | 创建/编辑/删除分组 |
| 导入整合包 | P0 | 支持 `.mrpack` (Modrinth) 和 `.zip` 格式 |
| 导入现有文件夹 | P1 | 选择本地 Minecraft 目录 |
| 实例备份 | P2 | 手动/自动备份实例数据 |

#### 模块 4：游戏启动

| 功能 | 优先级 | 说明 |
|------|--------|------|
| Java 运行时检测 | P0 | 检查游戏所需 Java 版本 |
| 自动 JRE 安装 | P0 | 内置 JRE 17/21，按需下载 |
| JVM 参数构建 | P0 | 内存分配、GC 参数、系统属性 |
| 类路径生成 | P0 | 合并版本 JAR + 库文件 + LWJGL |
| Natives 提取 | P0 | 解压 LWJGL natives 到临时目录 |
| 游戏进程管理 | P0 | 启动、监控、退出处理 |
| 崩溃分析 | P1 | 解析 `crash-reports/`，提供诊断建议 |
| 快速启动 | P2 | 1.20.2+ 快速启动优化 |

#### 模块 5：内容中心（Mod/资源包/着色器）

| 功能 | 优先级 | 说明 |
|------|--------|------|
| Modrinth 浏览 | P0 | 搜索、筛选、详情查看 |
| CurseForge 浏览 | P0 | 搜索、筛选、详情查看 |
| Mod 安装 | P0 | 一键安装到当前实例 |
| Mod 更新检查 | P1 | 检测已安装 Mod 的更新 |
| 资源包管理 | P1 | 浏览、安装、启用/禁用 |
| 着色器管理 | P2 | 浏览、安装、配置 |
| 图标缓存 | P1 | 本地缓存 Mod 图标，减少重复下载 |

#### 模块 6：多人游戏

| 功能 | 优先级 | 说明 |
|------|--------|------|
| 服务器列表 | P0 | 添加/编辑/删除服务器 |
| 服务器 Ping | P0 | 显示在线玩家、延迟 |
| 快速连接 | P0 | 点击直接启动游戏并连接 |
| 本地服务器 | P1 | 扫描局域网服务器 |
| 陶瓦联机 | P2 | Terracotta 多人联机支持 |

#### 模块 7：皮肤管理

| 功能 | 优先级 | 说明 |
|------|--------|------|
| 皮肤浏览 | P1 | 从 Modrinth/CurseForge 浏览皮肤 |
| 皮肤应用 | P1 | 应用到当前账号 |
| 3D 皮肤预览 | P2 | 参考 skinview3d 的渲染效果 |

#### 模块 8：设置

| 功能 | 优先级 | 说明 |
|------|--------|------|
| 内存分配 | P0 | 滑动条设置 JVM 最大堆内存 |
| 渲染器选择 | P0 | OpenGL ES 2/3、Vulkan、Zink 等 |
| 控制设置 | P0 | 按钮大小、鼠标缩放、陀螺仪灵敏度 |
| 下载设置 | P1 | 线程数、镜像源、代理 |
| Java 设置 | P1 | 自定义 JVM 参数、自定义 Java 路径 |
| 主题设置 | P2 | 深色/浅色模式、主题色 |
| 应用更新 | P2 | 检查更新、自动下载 |

### 3.2 安卓特有功能

| 功能 | 优先级 | 说明 |
|------|--------|------|
| 自定义控制布局 | P0 | 按钮、摇杆、抽屉式菜单，可编辑 |
| 虚拟触摸板 | P0 | 模拟鼠标，支持多点触控 |
| 虚拟键盘 | P0 | 游戏内文本输入 |
| 陀螺仪控制 | P1 | 视角控制，灵敏度可调 |
| 手柄支持 | P1 | 蓝牙手柄映射 |
| 前台服务 | P0 | 游戏运行时保持进程存活 |
| 通知栏 | P0 | 显示游戏运行状态、控制按钮 |
| 存储权限 | P0 | SAF (Storage Access Framework) 访问游戏目录 |
| 深链接 | P1 | `qookix://` 协议启动游戏 |
| 后台下载 | P1 | 游戏/Mod 下载时允许后台运行 |

### 3.3 与电脑端的功能对齐

| 功能 | 电脑端 | 安卓端 | 差异处理 |
|------|--------|--------|----------|
| 版本管理 | ✅ 完整 | ✅ 完整 | 共享相同数据结构 |
| 账号管理 | ✅ 离线+MS | ✅ 离线+MS | 共享 `accounts.json` 格式 |
| 实例管理 | ✅ 完整 | ✅ 完整 | 共享 `launcher_profiles.json` |
| 游戏启动 | ✅ 本地 Java | ✅ 内置 JRE | 安卓需内置运行时 |
| 内容中心 | ✅ MF+CF | ✅ MF+CF | 共享 API 接口 |
| 皮肤管理 | ✅ 3D 预览 | ⚠️ 2D+3D | 3D 预览可选 |
| 崩溃分析 | ✅ 完整 | ⚠️ 基础 | 简化为错误日志 |
| 系统托盘 | ✅ 可用 | ❌ 不可用 | 改为通知栏 |
| 标题栏 | ✅ 自定义 | ❌ 不可用 | 使用系统标题栏 |
| 侧边栏 | ✅ 可折叠 | ⚠️ 底部导航 | 适配手机屏幕 |

---

## 4. 非功能需求

### 4.1 性能需求

| 指标 | 要求 |
|------|------|
| 冷启动时间 | ≤ 3 秒（首次）/ ≤ 1 秒（热启动） |
| 版本列表加载 | ≤ 2 秒（含缓存） |
| 游戏启动时间 | ≤ 30 秒（从点击到游戏画面） |
| 内存占用 | 启动器本身 ≤ 200MB |
| 下载速度 | 充分利用带宽，多线程并发 |

### 4.2 兼容性需求

| 项目 | 要求 |
|------|------|
| 最低 SDK | **Android 6.0 (API 23)** |
| 目标 SDK | **Android 14 (API 34)** |
| 架构支持 | arm64-v8a, armeabi-v7a, x86_64 |
| Java 版本 | JRE 8, 17, 21, 25 |
| Minecraft 版本 | 1.7.10 ~ 最新版 |
| 加载器 | Fabric, Quilt, Forge, NeoForge |

### 4.3 安全需求

| 项目 | 要求 |
|------|------|
| 账号安全 | Token 加密存储，不上传服务器 |
| 下载校验 | SHA-1 校验所有下载文件 |
| 权限最小化 | 仅申请必要权限 |
| 网络安全 | HTTPS 传输，证书校验 |
| 本地安全 | 游戏目录隔离，防止越权访问 |

### 4.4 可用性需求

| 项目 | 要求 |
|------|------|
| 语言 | 简体中文、英文 |
| 无障碍 | 支持 TalkBack，对比度 ≥ 4.5:1 |
| 离线模式 | 已下载版本可离线启动 |
| 错误恢复 | 下载中断可断点续传 |

---

## 5. 系统架构

### 5.1 整体架构

```
┌─────────────────────────────────────────────────────────────────┐
│                      QookiX-Launcher-Android                    │
│                         (Compose UI Layer)                      │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌───────────┐ │
│  │  版本管理   │ │  账号管理   │ │  实例管理   │ │  内容中心 │ │
│  │  ViewModel  │ │  ViewModel  │ │  ViewModel  │ │  ViewModel│ │
│  └─────────────┘ └─────────────┘ └─────────────┘ └───────────┘ │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌───────────┐ │
│  │  多人游戏   │ │  皮肤管理   │ │  设置管理   │ │  崩溃分析 │ │
│  │  ViewModel  │ │  ViewModel  │ │  ViewModel  │ │  ViewModel│ │
│  └─────────────┘ └─────────────┘ └─────────────┘ └───────────┘ │
├─────────────────────────────────────────────────────────────────┤
│                      Domain Layer (Kotlin)                      │
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────────────┐│
│  │  VersionRepo  │  │ AccountRepo   │  │ InstanceRepo          ││
│  │  (版本仓库)   │  │  (账号仓库)   │  │  (实例仓库)           ││
│  └───────────────┘  └───────────────┘  └───────────────────────┘│
│  ┌───────────────┐  ┌───────────────┐  ┌───────────────────────┐│
│  │  ModpackRepo  │  │ ServerRepo    │  │ SettingsRepo          ││
│  │  (整合包仓库) │  │  (服务器仓库) │  │  (设置仓库)           ││
│  └───────────────┘  └───────────────┘  └───────────────────────┘│
├─────────────────────────────────────────────────────────────────┤
│                     Data Layer (Kotlin + JNI)                   │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                    MultiRT (Java 运行时管理)                 ││
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────────┐ ││
│  │  │ JRE 8    │ │ JRE 17   │ │ JRE 21   │ │ JRE 25         │ ││
│  │  │ (内置)   │ │ (内置)   │ │ (内置)   │ │ (可选)         │ ││
│  │  └──────────┘ └──────────┘ └──────────┘ └────────────────┘ ││
│  └─────────────────────────────────────────────────────────────┘│
│  ┌─────────────────────────────────────────────────────────────┐│
│  │                    Game Engine (JNI + NDK)                   ││
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────────┐ ││
│  │  │ LWJGL 3  │ │ GLFW     │ │ OpenGL   │ │ Caciocavallo   │ ││
│  │  │ (绑定)   │ │ (窗口)   │ │ ES 2/3   │ │ (AWT 模拟)     │ ││
│  │  └──────────┘ └──────────┘ └──────────┘ └────────────────┘ ││
│  └─────────────────────────────────────────────────────────────┘│
├─────────────────────────────────────────────────────────────────┤
│                    Platform Layer (Android)                     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │ GameService  │  │ DownloadMgr  │  │ NotificationManager  │  │
│  │ (前台服务)   │  │ (下载管理)   │  │ (通知管理)           │  │
│  └──────────────┘  └──────────────┘  └──────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

### 5.2 模块依赖关系

```
UI Layer
  └── Domain Layer
       ├── VersionRepo ──┐
       ├── AccountRepo ──┼──▶ MultiRT
       ├── InstanceRepo ─┤      └──▶ Game Engine (JNI)
       ├── ModpackRepo ─┤
       ├── ServerRepo  ─┴──▶ DownloadManager
       └── SettingsRepo ───▶ SharedPreferences / DataStore
```

### 5.3 启动流程

```
用户点击启动
    │
    ▼
检查条件 ─── 是否有进行中的任务？ ──→ 提示"有任务进行中"
    │
    ▼
检查 profile ─── 是否配置了版本？ ──→ 提示"请先选择版本"
    │
    ▼
检查账户 ─── 是否已登录？ ──→ 跳转到登录页
    │
    ▼
检查 Java 版本 ─── 是否兼容？ ──→ 自动安装 JRE
    │
    ▼
下载资源 ─── MinecraftDownloader
    │   ├── 版本 JSON
    │   ├── 资源索引 (assets)
    │   ├── 客户端 JAR
    │   ├── 库文件 (libraries)
    │   └── 日志配置
    │
    ▼
提取 Natives ─── 解压 LWJGL natives
    │
    ▼
构建类路径 ─── 版本 JAR + 库 + LWJGL
    │
    ▼
启动 Java VM ─── JREUtils.launchJavaVM()
    │
    ▼
游戏运行 ─── MainActivity (MinecraftGLSurface + ControlLayout)
```

---

## 6. 数据模型

### 6.1 核心数据结构

#### MinecraftProfile（实例配置）

```kotlin
data class MinecraftProfile(
    val id: String,                    // UUID
    val name: String,                  // 实例名称
    val mcVersion: String,             // Minecraft 版本
    val loader: Loader,                // vanilla/fabric/quilt/forge/neoforge
    val loaderVersion: String?,        // 加载器版本
    val created: Long,                 // 创建时间戳
    val lastPlayed: Long?,             // 最后游玩时间
    val totalPlayTime: Long,           // 累计游玩时长（秒）
    val gameDir: String,               // 游戏目录
    val javaDir: String,               // Java 运行时路径 (amethyst://Internal-17)
    val javaArgs: String?,             // 自定义 JVM 参数
    val gameArgs: String?,             // 自定义游戏参数
    val resolution: Pair<Int, Int>?,   // 分辨率
    val maxMemoryMb: Int?,             // 最大内存 (MB)
    val memoryMode: String?,           // 内存分配模式
    val accountId: String?,            // 关联账号 ID
    val icon: String?,                 // 图标 Base64
    val mods: List<InstalledContent>,  // 已安装 Mod
    val resourcePacks: List<InstalledContent>,
    val shaders: List<InstalledContent>,
    val group: String?,                // 所属分组
    val isSymlink: Boolean?,
    val sourcePath: String?            // 整合包来源路径
)

enum class Loader {
    VANILLA, FABRIC, QUILT, FORGE, NEOFORGE
}

data class InstalledContent(
    val id: String,
    val name: String,
    val version: String,
    val fileUrl: String,
    val installedAt: Long
)
```

#### MinecraftAccount（账号）

```kotlin
sealed interface Account {
    data class Offline(
        val uuid: String,
        val username: String,
        val created: Long
    ) : Account

    data class Microsoft(
        val uuid: String,
        val username: String,
        val created: Long,
        val msaRefreshToken: String,
        val xuid: String,
        val expiresAt: Long,
        val skinFaceBase64: String?
    ) : Account
}
```

#### Runtime（Java 运行时）

```kotlin
data class Runtime(
    val name: String,                  // "Internal-17" / "External-21"
    val versionString: String,         // "17.0.1"
    val javaVersion: Int,              // 17
    val arch: String,                  // "arm64" / "x86_64"
    val path: String,                  // 运行时安装路径
    val isInternal: Boolean            // 是否内置
)
```

#### VersionList（版本清单）

```kotlin
data class VersionList(
    val latest: Map<String, String>,   // {"latest-release": "1.20.1"}
    val versions: List<VersionInfo>
)

data class VersionInfo(
    val id: String,                    // "1.20.1"
    val type: String,                  // "release" / "snapshot"
    val url: String,                   // 版本 JSON URL
    val sha1: String?,
    val releaseTime: String,
    val inheritsFrom: String?,
    val javaVersion: JavaVersionInfo?,
    val libraries: List<DependentLibrary>,
    val downloads: Map<String, ClientDownload>,
    val assetIndex: AssetIndex?,
    val arguments: GameArguments?,
    val mainClass: String
)

data class JavaVersionInfo(
    val component: String,             // "jre-legacy"
    val majorVersion: Int              // 8, 17, 21
)
```

### 6.2 存储目录结构

```
/storage/emulated/0/games/QookiX/
├── .minecraft/
│   ├── versions/                    # Minecraft 版本
│   │   ├── 1.7.10/
│   │   │   ├── 1.7.10.jar
│   │   │   └── 1.7.10.json
│   │   └── ...
│   ├── libraries/                   # 库文件
│   ├── assets/                      # 资源文件
│   │   ├── indexes/
│   │   ├── objects/
│   │   └── skins/
│   ├── launcher_profiles.json       # 启动器配置
│   ├── instance_groups.json         # 实例分组
│   └── crash-reports/               # 崩溃报告
├── custom_instances/                # 整合包实例
│   └── <instance-name>/
├── accounts/                        # 账户信息
│   ├── <uuid>.json
│   └── ...
├── runtimes/                        # Java 运行时
│   ├── Internal-17/
│   ├── Internal-21/
│   └── External-8/
├── cache/                           # 缓存目录
│   ├── version_list.json            # 版本列表缓存
│   ├── natives/                     # Natives 临时目录
│   └── icons/                       # Mod 图标缓存
├── controlmap/                      # 控制布局
│   └── default.json
├── caciocavallo/                    # AWT 实现
├── lwjgl3/                          # LWJGL 3 JARs
└── latestlog.txt                    # 最新日志
```

---

## 7. UI/UX 设计

### 7.1 整体布局

采用 **底部导航 + 顶部栏** 的经典安卓布局，适配手机和平板：

```
┌─────────────────────────────────────┐
│  [≡] QookiX Launcher        [🔔]    │  ← 顶部栏
├─────────────────────────────────────┤
│                                     │
│         内容区域 (NavHost)           │
│                                     │
│                                     │
│                                     │
├─────────────────────────────────────┤
│  [🏠]  [📦]  [✨]  [⚙️]  [🎮]     │  ← 底部导航
│  首页  实例  内容  设置  多人      │
└─────────────────────────────────────┘
```

### 7.2 页面结构

| 页面 | 路由 | 说明 |
|------|------|------|
| 首页 | `/` | 欢迎信息、最近游玩、新闻 |
| 实例列表 | `/instances` | 卡片式实例列表、分组筛选 |
| 实例详情 | `/instance/{id}` | Mod 管理、参数配置 |
| 创建实例 | `/create` | 向导式创建流程 |
| 内容中心 | `/browse` | Mod/资源包/着色器浏览 |
| 下载中心 | `/downloads` | 进行中的下载任务 |
| 服务器列表 | `/multiplayer` | 服务器列表、Ping |
| 设置 | `/settings` | 分类设置页面 |
| 皮肤中心 | `/skins` | 皮肤浏览与应用 |
| 登录 | `/login` | 离线/Microsoft 登录 |

### 7.3 核心页面设计

#### 首页 (HomeView)

```
┌─────────────────────────────────────┐
│  QookiX Launcher                    │
├─────────────────────────────────────┤
│                                     │
│  ┌─────────────────────────────────┐│
│  │  最近游玩                        ││
│  │  ┌──────┐ ┌──────┐ ┌──────┐   ││
│  │  │ 实例1 │ │ 实例2 │ │ 实例3 │   ││
│  │  │ 1.20 │ │ 1.19 │ │ 1.18 │   ││
│  │  └──────┘ └──────┘ └──────┘   ││
│  └─────────────────────────────────┘│
│                                     │
│  ┌─────────────────────────────────┐│
│  │  新闻                            ││
│  │  • QookiX v0.6.0 发布！         ││
│  │  • Fabric 0.15.0 更新           ││
│  └─────────────────────────────────┘│
│                                     │
│  ┌─────────────────────────────────┐│
│  │  快速操作                        ││
│  │  [新建实例] [导入整合包] [刷新]  ││
│  └─────────────────────────────────┘│
└─────────────────────────────────────┘
```

#### 实例卡片 (InstanceCard)

```
┌─────────────────────────────────────┐
│  ┌─────┐                              │
│  │ 🖼️  │  我的生存世界                │
│  │图标  │  Minecraft 1.20.1           │
│  └─────┘  Fabric 0.14.0              │
│           128 个 Mod                  │
│           上次游玩: 2 小时前          │
│                                     │
│  [▶ 启动] [⋮ 更多]                   │
└─────────────────────────────────────┘
```

#### 游戏界面 (MainActivity)

```
┌─────────────────────────────────────┐
│  [← 返回]                    [⏹]  │
├─────────────────────────────────────┤
│                                     │
│          Minecraft 游戏画面          │
│          (MinecraftGLSurface)       │
│                                     │
│                                     │
│                                     │
├─────────────────────────────────────┤
│  🕹️  ───  ───  ───  ⌨️  📋  ⚙️  │  ← 控制栏
│  摇杆  按键  按键  按键 键盘 剪贴板 设置
└─────────────────────────────────────┘
```

### 7.4 交互流程

#### 启动游戏流程

```
用户点击启动按钮
    │
    ▼
显示启动进度对话框
    │
    ▼
1. 检查 Java 版本 ─── 不兼容？ ──→ 自动安装 JRE
    │
    ▼
2. 下载资源 ─── 显示下载进度
    │
    ▼
3. 提取 Natives
    │
    ▼
4. 启动游戏 ─── 关闭进度对话框
    │
    ▼
进入 MainActivity
```

#### 创建实例流程

```
点击"新建实例"
    │
    ▼
步骤 1: 选择版本
    ├── 搜索版本
    ├── 筛选 (正式版/快照版)
    └── 选择版本
    │
    ▼
步骤 2: 选择加载器
    ├── 原版 (Vanilla)
    ├── Fabric / Quilt
    ├── Forge / NeoForge
    └── 选择加载器版本
    │
    ▼
步骤 3: 配置参数
    ├── 实例名称
    ├── 游戏目录
    ├── 内存分配
    ├── JVM 参数
    └── 分辨率
    │
    ▼
创建完成 ──→ 返回实例列表
```

---

## 8. API 设计

### 8.1 版本 API

| 端点 | 方法 | 说明 |
|------|------|------|
| `/version/manifest` | GET | 获取版本清单 |
| `/version/{id}/json` | GET | 获取版本详情 |
| `/version/{id}/download` | GET | 获取版本下载信息 |

### 8.2 认证 API

| 端点 | 方法 | 说明 |
|------|------|------|
| `/auth/microsoft/device_code` | POST | 获取设备码 |
| `/auth/microsoft/token` | POST | 交换 Access Token |
| `/auth/microsoft/refresh` | POST | 刷新 Token |
| `/auth/minecraft/profile` | GET | 获取玩家资料 |
| `/auth/minecraft/entitlements` | GET | 检查游戏所有权 |

### 8.3 内容 API

| 端点 | 方法 | 说明 |
|------|------|------|
| `/content/modrinth/search` | GET | Modrinth 搜索 |
| `/content/modrinth/mod/{id}` | GET | Modrinth Mod 详情 |
| `/content/modrinth/version` | GET | Modrinth 版本列表 |
| `/content/curseforge/search` | GET | CurseForge 搜索 |
| `/content/curseforge/mod/{id}` | GET | CurseForge Mod 详情 |
| `/content/curseforge/files` | GET | CurseForge 文件列表 |

### 8.4 内部 API (Tauri IPC 复用)

```kotlin
// 通过 Tauri IPC 与 Rust 后端通信
interface QookixApi {
    // 版本管理
    suspend fun getVersionList(): VersionList
    suspend fun getVersionInfo(versionId: String): VersionInfo
    suspend fun installVersion(versionId: String): Result

    // 实例管理
    suspend fun createInstance(config: InstanceConfig): Result
    suspend fun deleteInstance(instanceId: String): Result
    suspend fun listInstances(): List<MinecraftProfile>

    // 游戏启动
    suspend fun launchGame(instanceId: String): Result
    suspend fun killGame(): Result

    // 账号管理
    suspend fun listAccounts(): List<Account>
    suspend fun addAccount(account: Account): Result
    suspend fun removeAccount(accountId: String): Result

    // 下载管理
    suspend fun downloadFile(url: String, dest: String): DownloadProgress
    suspend fun cancelDownload(taskId: String): Result

    // 设置
    suspend fun getSettings(): Settings
    suspend fun updateSettings(settings: Settings): Result
}
```

---

## 9. 里程碑规划

### Phase 1: 基础框架 (4 周)

| 周期 | 任务 | 交付物 |
|------|------|--------|
| W1 | 项目初始化、技术栈搭建 | 可运行的空项目 |
| W1 | 基础 UI 框架（底部导航、主题系统） | 导航框架 |
| W2 | MultiRT 集成（JRE 17/21 内置） | Java 运行时管理 |
| W2 | 版本清单获取与展示 | 版本列表页面 |
| W3 | 账号管理（离线 + Microsoft） | 登录页面 |
| W3 | 实例管理 CRUD | 实例列表/详情页面 |
| W4 | 游戏启动流程（下载 + 启动） | 可启动游戏 |

**Phase 1 里程碑：** 能够选择版本、创建实例、启动 Minecraft 1.20.1

### Phase 2: 内容生态 (4 周)

| 周期 | 任务 | 交付物 |
|------|------|--------|
| W5 | Modrinth API 集成 | Mod 浏览页面 |
| W6 | CurseForge API 集成 | 内容中心完整 |
| W7 | Mod 安装与管理 | 实例 Mod 列表 |
| W7 | 整合包导入（.mrpack） | 导入功能 |
| W8 | 资源包/着色器管理 | 内容管理完整 |

**Phase 2 里程碑：** 能够浏览、安装 Mod 和整合包

### Phase 3: 安卓特化 (3 周)

| 周期 | 任务 | 交付物 |
|------|------|--------|
| W9 | 自定义控制布局 | 控制编辑器 |
| W10 | 虚拟触摸板 + 键盘 | 输入系统 |
| W10 | 陀螺仪 + 手柄支持 | 高级输入 |
| W11 | 前台服务 + 通知 | 后台运行 |

**Phase 3 里程碑：** 完整的触摸控制体验

### Phase 4: 完善与优化 (3 周)

| 周期 | 任务 | 交付物 |
|------|------|--------|
| W12 | 多人游戏（服务器列表 + Ping） | 多人游戏页面 |
| W13 | 皮肤管理 | 皮肤中心 |
| W13 | 崩溃分析 | 崩溃报告 |
| W14 | 性能优化、兼容性测试 | 发布候选版 |

**Phase 4 里程碑：** 功能完整，可发布

### Phase 5: 发布与维护 (持续)

| 任务 | 说明 |
|------|------|
| 应用商店上架 | Google Play、酷安等 |
| 自动更新 | 应用内更新检查 |
| 社区反馈 | Issue 收集与修复 |
| 版本迭代 | 持续功能增强 |

---

## 10. 风险与应对

| 风险 | 影响 | 应对措施 |
|------|------|----------|
| JRE 兼容性问题 | 高 | 参考 PojavLauncher 的 JRE 适配方案 |
| OpenGL ES 版本不足 | 高 | 提供多渲染器选择（GL4ES、Zink） |
| 内存不足导致崩溃 | 高 | 自动检测可用内存，限制最大分配 |
| 存储权限变更 | 中 | 使用 SAF (Storage Access Framework) |
| 性能问题 | 中 | 异步加载、缓存优化 |
| 版本继承解析错误 | 中 | 参考 Pojav 的 `getVersionInfo` 实现 |

---

## 11. 参考项目

| 项目 | 地址 | 参考价值 |
|------|------|----------|
| PojavLauncher | https://github.com/PojavLauncherTeam/PojavLauncher | OpenJDK 集成、OpenGL 渲染、输入处理 |
| Amethyst-Android | https://github.com/AngelAuraMC/Amethyst | 完整启动器架构、Modpack 集成 |
| QookiX-Launcher | https://github.com/weimosheng/QookiX-Launcher | 数据模型、API 设计、UI 逻辑 |

---

## 附录 A：术语表

| 术语 | 说明 |
|------|------|
| MultiRT | 多 Java 运行时管理，支持 JRE 8/17/21/25 |
| LWJGL | Lightweight Java Game Library，Java 游戏开发库 |
| GLFW | 窗口和输入管理库 |
| Caciocavallo | 在 Android 上实现 AWT/Swing 的项目 |
| GL4ES | OpenGL ES 到 OpenGL 的转换层 |
| Zink | Vulkan 实现的 OpenGL 驱动 |
| .mrpack | Modrinth 整合包格式 |
| SAF | Storage Access Framework，安卓存储访问框架 |

---

*文档结束*
