LOCAL_PATH := $(call my-dir)
HERE_PATH := $(LOCAL_PATH)

# ============================================================================
# Pojav 版 JNI 核心（libpojavexec.so）
#
# 来自 PojavLauncher（app_pojavlauncher/src/main/jni），原样保留：
#   - egl_bridge.c / ctxbridges/*：把 Android Surface 接成 GL4ES 能用的 EGL 上下文，
#     并实现 OpenGL 函数表转发（这是安卓上跑 MC 的关键一层）
#   - input_bridge_v3.c：JNI_OnLoad 里给 org.lwjgl.glfw.CallbackBridge 注册输入相关 native
#   - jvm_hooks/*：dlopen / forkAndExec 等补丁（绕过安卓限制）
#   - driver_helper/nsbypass.c：绕过 linker namespace 直取 GPU 驱动
#
# 与 Pojav 原版的差异（有意为之）：
#   1. 去掉 exithook / linkerhook / pojavexec_awt / 假 awt_headless / 假 awt_xawt
#      这几个模块：前两个依赖 bytehook 且只做退出与权限 hook；
#      后三个是 Caciocavallo(AWT) 专用，当前 launch.rs 里 ENABLE_CACIOCAVALLO=false。
#   2. 不 import prefab/bytehook（已无使用者）。
#
# 模块名必须叫 pojavexec：Lwjglglf stub 里是按 System.loadLibrary("pojavexec") 找的。
# ============================================================================

include $(CLEAR_VARS)
LOCAL_LDLIBS := -ldl -llog -landroid

LOCAL_MODULE := pojavexec
LOCAL_SRC_FILES := \
    bigcoreaffinity.c \
    egl_bridge.c \
    ctxbridges/loader_dlopen.c \
    ctxbridges/gl_bridge.c \
    ctxbridges/osm_bridge.c \
    ctxbridges/egl_loader.c \
    ctxbridges/osmesa_loader.c \
    ctxbridges/swap_interval_no_egl.c \
    environ/environ.c \
    jvm_hooks/emui_iterator_fix_hook.c \
    jvm_hooks/java_exec_hooks.c \
    jvm_hooks/lwjgl_dlopen_hook.c \
    input_bridge_v3.c \
    jre_launcher.c \
    utils.c \
    stdio_is.c \
    driver_helper/nsbypass.c

ifeq ($(TARGET_ARCH_ABI),arm64-v8a)
LOCAL_CFLAGS += -DADRENO_POSSIBLE
endif
include $(BUILD_SHARED_LIBRARY)

# ============================================================================
# linkerhook：Zink + Turnip 路径的必需件。
#
# libpojavexec 的 egl_bridge.c 在 load_turnip_vulkan() 里会
#   linker_ns_dlopen("liblinkerhook.so")
# 拿不到它就**静默返回 NULL**（不打印任何东西）→ Turnip 永不加载 →
# Mesa 退回系统 Vulkan 驱动 → 在 OSMesaCreateContext 里 SIGSEGV。
# 移植时漏了这个模块，所以「渲染器切 Zink」一直是必崩。
# `-z global` 是让它的符号对 linker 可见（hook 需要被 android_dlopen_ext 调回来）。
# ============================================================================
include $(CLEAR_VARS)
LOCAL_MODULE := linkerhook
LOCAL_SRC_FILES := driver_helper/hook.c
LOCAL_LDFLAGS := -z global
include $(BUILD_SHARED_LIBRARY)
