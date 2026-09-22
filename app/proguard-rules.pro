# Proguard rules for QookiX Launcher

# Keep data classes with serialization
-keep class com.qookix.launcher.model.** { *; }
-keep,allowobfuscation interface com.qookix.launcher.model.**

# Keep Kotlinx Serialization
-keepattributes RuntimeVisibleAnnotations
-keepattributes RuntimeVisibleParameterAnnotations
-keepattributes RuntimeInvisibleAnnotations
-keepattributes RuntimeInvisibleParameterAnnotations
-keepattributes Signature
-keepattributes InnerClasses

# Keep Hilt
-keep class dagger.hilt.** { *; }
-keep class javax.inject.** { *; }
-dontwarn javax.inject.**

# Keep OkHttp
-keepattributes Signature
-keepattributes AnonymousInnerClass
-dontwarn okhttp3.**
-dontwarn okio3.**

# Keep Retrofit
-keepattributes Signature
-keepattributes Exceptions
-keep class retrofit2.** { *; }
-dontwarn retrofit2.**

# Keep Coil
-keep class coil.** { *; }
-dontwarn coil.**

# Keep Coroutines
-keep class kotlinx.coroutines.** { *; }
-dontwarn kotlinx.coroutines.**

# Keep AndroidX
-keep class androidx.** { *; }
-dontwarn androidx.**

# Keep Compose
-keep class androidx.compose.** { *; }
-dontwarn androidx.compose.**

# Keep Navigation
-keep class androidx.navigation.** { *; }
-dontwarn androidx.navigation.**

# Keep DataStore
-keep class androidx.datastore.** { *; }
-dontwarn androidx.datastore.**

# Keep Security
-keep class androidx.security.** { *; }
-dontwarn androidx.security.**

# Keep Worker
-keep class androidx.work.** { *; }
-dontwarn androidx.work.**
