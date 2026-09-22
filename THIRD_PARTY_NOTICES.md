# 第三方代码来源与许可

本项目包含来自 **PojavLauncher** 的代码/二进制，遵循 **LGPL-3.0**。

- 上游：https://github.com/PojavLauncherTeam/PojavLauncher
- 许可证：GNU Lesser General Public License v3.0（见 `LICENSE-LGPL-3.0.txt`）
- 版权归 PojavLauncherTeam 及其贡献者所有；修改部分同样以 LGPL-3.0 提供。

## 具体来自 PojavLauncher 的部分

| 位置 | 内容 |
|---|---|
| `src-tauri/gen/android/app/src/main/jni/` | `egl_bridge.c`、`input_bridge_v3.c`、`ctxbridges/*`、`jvm_hooks/*`、`driver_helper/*`、`utils.c`、`stdio_is.c`、`environ/*`（编译为 `libpojavexec.so`） |
| `src-tauri/gen/android/app/src/main/java/org/lwjgl/glfw/CallbackBridge.java` | 输入桥（应用侧） |
| `src-tauri/gen/android/app/src/main/java/net/kdt/pojavlaunch/` | `LwjglGlfwKeycode`、`EfficientAndroidLWJGLKeycode`、`GrabListener`、`CriticalNativeTest`、`Logger` |
| `src-tauri/gen/android/app/src/main/java/dalvik/annotation/optimization/` | `CriticalNative` 注解桩 |
| `src-tauri/assets/lwjgl/lwjgl-glfw-classes.jar` | Pojav 版 LWJGL（含纯 Java GLFW stub） |
| `src-tauri/assets/cacio8|cacio17/*.jar` | Caciocavallo（AWT on Android） |
| `src-tauri/gen/android/app/src/main/jniLibs/<abi>/*.so` | `liblwjgl*.so`、`libgl4es_114.so`、`libopenal.so`、`libfreetype.so`、`libOSMesa.so`、`libunpack200.so`、`libjnidispatch.so` |

## 游戏内控制层（2026-09 从 PojavLauncher 原样移植，LGPL-3.0）

「进入游戏之后的逻辑」整体对齐 PojavLauncher 的 `MainActivity` 与 `customcontrols` 包。
逐文件来源如下（相对 `src-tauri/gen/android/app/src/main/java/`），
除下表「改动」列标注者外均为**逐字节照搬上游**：

| 位置 | 内容 | 改动 |
|---|---|---|
| `net/kdt/pojavlaunch/customcontrols/`（9 个文件） | `ControlData`、`CustomControls`、`ControlLayout`、`ControlDrawerData`、`ControlJoystickData`、`LayoutConverter`、`LayoutSanitizer`、`ControlButtonMenuListener`、`EditorExitable` | 仅把 `import net.kdt.pojavlaunch.R` 改为 `import com.zhayi.qookix.R` |
| `net/kdt/pojavlaunch/customcontrols/buttons/`（5） | `ControlButton`、`ControlDrawer`、`ControlInterface`、`ControlJoystick`、`ControlSubButton` | 同上；`ControlButton` 把 `MainActivity.*` 指向 `com.zhayi.qookix.GameActivity`，`R.attr.colorAccent` 改为 `androidx.appcompat.R.attr.colorAccent` |
| `net/kdt/pojavlaunch/customcontrols/mouse/`（15） | `Touchpad`、`AbstractTouchpad`、`AndroidPointerCapture`、`GyroControl`、`HotbarView`、`InGUIEventProcessor`、`InGameEventProcessor`、`TouchEventProcessor`、`LeftClickGesture`、`RightClickGesture`、`TapDetector`、`Scroller`、`DropGesture`、`ValidatorGesture`、`PointerTracker` | R 导入；`InGUIEventProcessor` 删掉未使用的 `SingleTapConfirm` 导入 |
| `net/kdt/pojavlaunch/customcontrols/keyboard/`（3） | `TouchCharInput`、`LwjglCharSender`、`CharacterSenderStrategy` | R 导入；`R.attr.editTextStyle` → `androidx.appcompat.R.attr.editTextStyle` |
| `net/kdt/pojavlaunch/customcontrols/handleview/`（8） | `ControlHandleView`、`ActionRow`、`ActionButtonInterface`、`AddSubButton`、`CloneButton`、`DeleteButton`、`DrawerPullButton`、`EditControlSideDialog` | R 导入 |
| `net/kdt/pojavlaunch/customcontrols/gamepad/` | `GamepadJoystick`、`direct/DirectGamepadEnableHandler`、`direct/GamepadKeycodes` | R 导入 |
| `net/kdt/pojavlaunch/colorselector/`（10） | 颜色选择器控件 | R 导入 |
| `net/kdt/pojavlaunch/MinecraftGLSurface.java` | 渲染面 + 触摸事件分发 | 剥离「实体手柄重映射」部分（依赖 JitPack 的 `fr.spse.gamepad_remapper`，本机不可得），其余照搬 |
| `net/kdt/pojavlaunch/{CustomControlsActivity,ImportControlActivity}.java` | 控制布局编辑器 / 导入 | `BaseActivity` 内联为 `AppCompatActivity`；导出改用本项目的 `FileProvider`（见下） |
| `net/kdt/pojavlaunch/utils/{MathUtils,FileUtils,MCOptionUtils,JSONUtils}.java`、`utils/interfaces/` | 工具类 | 照搬 |
| `net/kdt/pojavlaunch/{LwjglGlfwKeycode,EfficientAndroidLWJGLKeycode,GrabListener,Logger}.java` | 键码表 / Grab 监听 / 日志 | 照搬 |
| `com/kdt/` | `SideDialogView`、`LoggerView`、`DefocusableScrollView`、`CustomSeekbar`、`SimpleArrayAdapter`、`pickafile/*` | 照搬 |
| `com/ipaulpro/afilechooser/FileListAdapter.java` | 文件选择器适配器 | 照搬 |
| `res/values/pojav_*.xml`、`res/values-zh-rCN/pojav_strings.xml`、`res/layout/activity_basemain.xml` 等 | Pojav 的字符串/数组/样式/尺寸/布局/图标 | 见 `POJAV_INGAME_PORT.md` 的「资源」一节 |
| `app/src/main/assets/pojav_default_control.json` | Pojav 的默认控制布局 | 照搬 |

### 自行编写的替身（shim）与内嵌第三方库

| 文件 | 说明 |
|---|---|
| `net/kdt/pojavlaunch/Tools.java` | Pojav 同名类的精简替身（只实现控制层用到的 22 个成员），语义与原版一致 |
| `net/kdt/pojavlaunch/prefs/LauncherPreferences.java` | 精简替身（17 个字段，键名与上游一致） |
| `net/kdt/pojavlaunch/utils/JREUtils.java` | 只保留 `setupBridgeWindow`，转发到 `TauriBridge` |
| `io/github/controlwear/virtual/joystick/android/JoystickView.java` | 自行实现，API 对齐 `virtual-joystick-android`（Apache-2.0，本机无法访问 JitPack） |
| `top/defaults/checkerboarddrawable/CheckerboardDrawable.java` | 自行实现，API 对齐 `checkerboarddrawable`（Apache-2.0） |
| `org/apache/commons/io/IOUtils.java` | 自行实现 `copy`，替代 474KB 的 `ExagearApacheCommons.jar` |
| `app/libs/exp4j-0.4.9-SNAPSHOT.jar` | 来自 PojavLauncher 的 `libs/`（PojavLauncherTeam 的 exp4j fork） |

> 上述 Pojav 来源文件仍以 **LGPL-3.0** 提供；修改（R 包名、shim 接线、剥离手柄重映射）同样以 LGPL-3.0 提供。
> 自行编写的替身类（`Tools`/`LauncherPreferences`/`JREUtils`/`JoystickView`/`CheckerboardDrawable`/`IOUtils`）
> 是为了在没有网络、无法引入上游依赖的环境下复现相同行为而重写的，不包含上游源码。

## 我们自己编写的部分

`src-tauri/src/**`（Rust 核心、启动器逻辑）、`src/**`（Tauri 前端）、
`com/zhayi/qookix/**`（GameActivity、虚拟控件、服务、桥接层）等，不在此许可覆盖范围内 ——
它们通过**动态库**（`libpojavexec.so`）与上述 LGPL 组件交互，符合 LGPL-3.0 的「允许替换库」要求。

## LGPL-3.0 的自定义义务（我们已遵守）

1. 保留版权与许可声明（本文件）；
2. 修改过的 LGPL 部分仍然以 LGPL-3.0 提供（本仓库公开这些源码）；
3. 允许用户替换该库：`libpojavexec.so` 以独立 `.so` 形式随 APK 分发，可被替换后重新打包。

## 渲染后端（本次新增，用于「设置 → 游戏内 → 渲染器」）

为了让渲染器能在 GL4ES 之外提供 **Zink + Turnip**，本仓库新增 3 个**预编译原生库**，
直接取自 `I:\program\vibe\mc\PojavLauncher\app_pojavlauncher\src\main\jniLibs\arm64-v8a\`：

| 库 | 来源 | 许可 | 与 GPL-3.0 的兼容性 |
|---|---|---|---|
| `libOSMesa.so` | Mesa 3D（OSMesa + Zink + gallium） | **MIT** | ✔ 宽松许可，可并入 |
| `libvulkan_freedreno.so` | Mesa 3D（Turnip，Adreno 的 Vulkan 驱动） | **MIT** | ✔ |
| `libVkLayer_khronos_timeline_semaphore.so` | Khronos Vulkan-ExtensionLayer | **Apache-2.0** | ✔（Apache-2.0 可并入 GPL-3.0） |

随 PojavLauncher 一并取得的既有原生库（此前已在用，这里补全许可信息）：

| 库 | 来源 | 许可 |
|---|---|---|
| `libgl4es_114.so` | GL4ES（ptitSeb） | MIT |
| `liblwjgl*.so` | LWJGL 3 | BSD-3-Clause |
| `libopenal.so` | OpenAL Soft | LGPL（上游为 LGPL-2.0-or-later，**上架前建议再核对其 COPYING**） |
| `libfreetype.so` | FreeType | FTL 或 GPL-2.0（双许可） |
| `libjnidispatch.so` | JNA | LGPL-2.1-or-later 或 Apache-2.0（双许可） |
| `libunpack200.so` | OpenJDK（JRE 内工具） | GPL-2.0 with Classpath Exception |

> 注意：这些 `.so` 都是 **4 KB 页对齐**（`PT_LOAD.p_align = 0x1000`）。
> Android 15+ 的 16 KB 页设备上可能无法 dlopen —— 详见 `MOBILE_AUDIT_NATIVE.md` 第一节。
> `libqookix_lib.so` 由我们自己编译，已经改成 16 KB 对齐。

## 本项目的许可与「LGPL 并入 GPL」声明

本项目以 **GPL-3.0** 发布，其中包含 **LGPL-3.0** 的 PojavLauncher 代码（游戏内控制层）
以及上表的第三方原生库。合并是**明确允许**的：

- **LGPL-3.0 → GPL-3.0**：LGPL-3.0 第 2 条允许把「基于库的作品」按 GPL-3.0 传递；
- **MIT / BSD-3-Clause / Apache-2.0**：均为宽松许可，可并入 GPL-3.0 作品，只需保留其版权与许可声明（即本文件）；
- 因此整个 APK 可以合法地以 **GPL-3.0** 分发。

为满足 LGPL-3.0 的义务，我们：
1. 在本文件保留全部版权与许可声明；
2. 以 LGPL-3.0 提供 Pojav 部分的源码（本仓库公开这些文件）；
3. **允许替换 LGPL 组件**：`libpojavexec.so` 与上表各原生库都以独立 `.so` 形式随 APK 分发，
   用户可自行替换后重新打包。
