package org.jspecify.annotations;

import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

/**
 * 只为编译 GLFW 补丁类而存在的空注解。
 *
 * <p>fork 版的 {@code GLFW.java} 源码里到处用 {@code @Nullable}（来自 jspecify），
 * 但那份注解 jar 我们没随包分发，编译时缺它就会报「找不到符号」。
 *
 * <p>运行时**不需要**它：类里的注解只是元数据，注解类缺失时 JVM 会直接忽略，
 * 不影响加载与执行。所以这个 stub 只进编译产物、不进 jar。
 */
@Retention(RetentionPolicy.CLASS)
@Target({ElementType.TYPE_USE, ElementType.TYPE_PARAMETER})
public @interface Nullable {
}
