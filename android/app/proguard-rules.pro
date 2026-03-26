# JNA (used by UniFFI for Rust FFI)
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-dontwarn com.sun.jna.**

# UniFFI generated Kotlin bindings
-keep class dev.disobey.speedreadingapp.rust.** { *; }
-keepclassmembers class dev.disobey.speedreadingapp.rust.** { *; }

# Kotlin coroutines
-keepnames class kotlinx.coroutines.internal.MainDispatcherFactory {}
-keepnames class kotlinx.coroutines.CoroutineExceptionHandler {}

# DataStore
-keepclassmembers class * extends com.google.protobuf.GeneratedMessageLite { <fields>; }
