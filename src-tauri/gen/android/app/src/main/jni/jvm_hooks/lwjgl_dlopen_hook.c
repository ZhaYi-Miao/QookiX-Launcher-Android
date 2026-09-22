//
// Created by maks on 06.01.2025.
//

#include "jvm_hooks.h"

#include <android/api-level.h>

#include "environ/environ.h"

#include <dlfcn.h>
#include <string.h>
#include <stdlib.h>
#include <stdint.h>

#define TAG __FILE_NAME__
#include <log.h>

extern void* maybe_load_vulkan();

/**
 * Basically a verbatim implementation of ndlopen(), found at
 * https://github.com/PojavLauncherTeam/lwjgl3/blob/3.3.1/modules/lwjgl/core/src/generated/c/linux/org_lwjgl_system_linux_DynamicLinkLoader.c#L11
 * but with our own additions for stuff like vulkanmod.
 */
static jlong ndlopen_bugfix(__attribute__((unused)) JNIEnv *env,
                     __attribute__((unused)) jclass class,
                     jlong filename_ptr,
                     jint jmode) {
    const char* filename = (const char*) filename_ptr;

    // Oveeride vulkan loading to let us load vulkan ourselves
    if(strstr(filename, "libvulkan.so") == filename) {
        printf("LWJGL linkerhook: replacing load for libvulkan.so with custom driver\n");
        return (jlong) maybe_load_vulkan();
    }

    // This hook also serves the task of mitigating a bug: the idea is that since, on Android 10 and
    // earlier, the linker doesn't really do namespace nesting.
    // It is not a problem as most of the libraries are in the launcher path, but when you try to run
    // VulkanMod which loads shaderc outside of the default jni libs directory through this method,
    // it can't load it because the path is not in the allowed paths for the anonymous namesapce.
    // This method fixes the issue by being in libpojavexec, and thus being in the classloader namespace

    int mode = (int)jmode;
    return (jlong) dlopen(filename, mode);
}

/**
 * glGetString 拦截：把 GL_VENDOR 换成我们自己的串。
 *
 * 为什么在**取值处**拦，而不是改渲染器的二进制：
 * MC 的 F3「Display: WxH (…)」那行取的就是 `glGetString(GL_VENDOR)`
 * （1.18.2 里是 `dsg.a()` → `GlStateManager._getString(7936)`，已用字节码确认），
 * 而实际报出来的是 GL4ES 那套上游署名串；那个字面量在上游二进制里查不到
 * （运行期拼装），改文件改不掉，所以在取值处拦截最确定 —— 与渲染器实现无关。
 *
 * 拦截点选 `org.lwjgl.opengl.GL11C.nglGetString`（`GL11.nglGetString` 只是转调它）：
 * 它是 JNI native，我们直接 RegisterNatives 换实现即可，**完全不动 LWJGL 的
 * 符号解析链路**。之前试过覆盖 `DynamicLinkLoader.ndlsym`，结果把 LWJGL 的函数表
 * 加载弄坏、游戏在 `Setting user` 之后静默退出（教训：能不动加载器就别动）。
 */
#define QOOKIX_GL_VENDOR_ENUM 0x1F00 /* GL_VENDOR */
static const char *const QOOKIX_GL_VENDOR = "QookiX & ptitSeb";

typedef const unsigned char *(*glGetString_fn)(unsigned int);

static glGetString_fn resolve_glGetString(void) {
    static glGetString_fn fn = NULL;
    static int tried = 0;
    if (fn != NULL) return fn;

    /* 1) 全局作用域。渲染器库多半是以 RTLD_GLOBAL 载入的，这里通常就能拿到。 */
    fn = (glGetString_fn) dlsym(RTLD_DEFAULT, "glGetString");
    if (fn != NULL) return fn;

    /* 2) 按 soname 直接认已加载的那份（RTLD_NOLOAD 不会重复加载）。
          这里列了三种渲染器的库名：GL4ES / OSMesa(zink) / MobileGlues。 */
    if (!tried) {
        tried = 1;
        const char *candidates[] = {"libgl4es_114.so", "libOSMesa.so", "libmobileglues.so", NULL};
        for (int i = 0; candidates[i] != NULL && fn == NULL; ++i) {
            void *handle = dlopen(candidates[i], RTLD_LAZY | RTLD_NOLOAD);
            if (handle == NULL) continue;
            fn = (glGetString_fn) dlsym(handle, "glGetString");
            if (fn != NULL) LOGI("已从 %s 解析到真实的 glGetString", candidates[i]);
        }
    }
    if (fn == NULL) LOGE("未能解析真实的 glGetString，本次调用原样返回 0");
    return fn;
}

/* 保留上游署名：MIT 许可要求保留版权声明，所以是「追加」而不是「抹掉」。 */
static jlong nglGetString_bugfix(__attribute__((unused)) JNIEnv *env,
                                 __attribute__((unused)) jclass class,
                                 jint name) {
    if (name == QOOKIX_GL_VENDOR_ENUM) {
        LOGI("glGetString(GL_VENDOR) -> %s", QOOKIX_GL_VENDOR);
        return (jlong) (intptr_t) QOOKIX_GL_VENDOR;
    }
    glGetString_fn fn = resolve_glGetString();
    if (fn == NULL) return 0;
    return (jlong) (intptr_t) fn((unsigned int) name);
}

/** 把 `GL11C.nglGetString` 换成我们的实现（GL_VENDOR 走品牌串，其余透传）。
 *  **不要在 JNI_OnLoad 里调用**：那时线程没有类加载器上下文，FindClass 会把进程搞崩
 *  （实测：游戏在 `Setting user` 处静默退出）。改由 JVM 侧的 `PojavRendererInit`
 *  native 调用 —— 那是 LWJGL 的类调用过来的，同包同加载器，FindClass 正常。 */
void installGlGetStringHook(JNIEnv *env) {
    jclass gl11c = (*env)->FindClass(env, "org/lwjgl/opengl/GL11C");
    if (gl11c == NULL) {
        LOGE("找不到 org/lwjgl/opengl/GL11C，跳过 GL_VENDOR 品牌串拦截");
        (*env)->ExceptionClear(env);
        return;
    }
    JNINativeMethod methods[] = {
            {"nglGetString", "(I)J", &nglGetString_bugfix}
    };
    if ((*env)->RegisterNatives(env, gl11c, methods, 1) != 0) {
        LOGE("注册 GL11C.nglGetString 失败");
        (*env)->ExceptionClear(env);
        return;
    }
    LOGI("已接管 GL11C.nglGetString（GL_VENDOR -> %s）", QOOKIX_GL_VENDOR);
}

/**
 * Install the LWJGL dlopen hook. This allows us to mitigate linker bugs and add custom library overrides.
 */
void installLwjglDlopenHook(JNIEnv *env) {
    LOGI("Installing LWJGL dlopen() hook");
    jclass dynamicLinkLoader = (*env)->FindClass(env, "org/lwjgl/system/linux/DynamicLinkLoader");
    if(dynamicLinkLoader == NULL) {
        LOGE("Failed to find the target class");
        (*env)->ExceptionClear(env);
        return;
    }
    JNINativeMethod ndlopenMethod[] = {
            {"ndlopen", "(JI)J", &ndlopen_bugfix}
    };
    if((*env)->RegisterNatives(env, dynamicLinkLoader, ndlopenMethod, 1) != 0) {
        LOGE("Failed to register the hooked method");
        (*env)->ExceptionClear(env);
    }
}