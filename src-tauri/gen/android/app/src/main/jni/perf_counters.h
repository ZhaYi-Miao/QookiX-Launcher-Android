#ifndef POJAVLAUNCHER_PERF_COUNTERS_H
#define POJAVLAUNCHER_PERF_COUNTERS_H

/*
 * 帧计数（性能面板的数据源）。
 *
 * 只做一件事：在「交换缓冲」处记一个单调时间戳，累积在环形缓冲里。
 * 三条渲染路径各有一处调用点：
 *   - GL4ES / MobileGlues → ctxbridges/gl_bridge.c  gl_swap_buffers()
 *   - Zink (OSMesa)       → ctxbridges/osm_bridge.c  osm_swap_buffers()
 * 26.3 的 SDL 路径换帧发生在 SDL 内部（SDL_GL_SwapWindow），当前没有计数点，
 * 面板在 26.3 上显示 CPU/GPU/内存，FPS 一行会停在 0。
 *
 * 每帧开销：一次 clock_gettime + 一次数组写入，可忽略。
 */

/** 每帧调用一次（渲染线程）。 */
void perf_frame(void);

/**
 * 汇总最近约 2 秒的数据，返回形如 "fps;avgMs;low1Fps;maxMs;renderer" 的静态字符串：
 *   fps     最近 1 秒的帧数
 *   avgMs   窗口内平均帧间隔（毫秒，1 位小数）
 *   low1Fps 按「最差 1% 帧间隔」折算的帧数（1% low）
 *   maxMs   窗口内最差的一帧（毫秒）
 *   renderer POJAV_RENDERER 的原值（C 层逻辑名），Java 侧负责转成显示名
 * 返回的是静态缓冲，调用方不必释放，但不要长期持有。
 */
const char* perf_snapshot(void);

#endif /* POJAVLAUNCHER_PERF_COUNTERS_H */
