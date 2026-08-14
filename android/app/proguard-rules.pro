# r8 keeps what it can prove is reached. everything below is reached in a way it
# cannot see — through generated code, reflection, or the platform calling in —
# so each rule names what breaks without it rather than being defensive.

# --- kotlinx.serialization -------------------------------------------------
# the serializers are generated companions that nothing calls by name, so r8
# removes them and every decode fails at runtime. this holds the whole station
# catalogue (catalog.json, ~54k stations), the favourites cache, the play
# history and the sync payloads — losing it empties the app.
-keepattributes *Annotation*, InnerClasses
-dontnote kotlinx.serialization.**
-keepclassmembers class kotlinx.serialization.json.** {
    *** Companion;
}
-keepclasseswithmembers class kotlinx.serialization.json.** {
    kotlinx.serialization.KSerializer serializer(...);
}
-keep,includedescriptorclasses class net.vchub.r4dio.**$$serializer { *; }
-keepclassmembers class net.vchub.r4dio.** {
    *** Companion;
}
-keepclasseswithmembers class net.vchub.r4dio.** {
    kotlinx.serialization.KSerializer serializer(...);
}

# --- media3 ----------------------------------------------------------------
# the session is resolved by the platform from the manifest, and the notification
# and steering-wheel keys route through it. a renamed service is a player that
# never starts.
-keep class androidx.media3.** { *; }
-dontwarn androidx.media3.**

# --- okhttp ----------------------------------------------------------------
# platform-specific TLS paths are picked reflectively; the warnings are for
# classes that only exist on other platforms.
-dontwarn okhttp3.**
-dontwarn okio.**
-dontwarn org.conscrypt.**
-dontwarn org.bouncycastle.**
-dontwarn org.openjsse.**

# --- zxing / qr scanning ---------------------------------------------------
# the capture activity is named in the manifest and launched by an intent, so
# nothing in kotlin references it directly. this is the path that links devices.
-keep class com.journeyapps.barcodescanner.** { *; }
-keep class com.google.zxing.** { *; }
-keep class net.vchub.r4dio.PortraitCaptureActivity { *; }
-dontwarn com.google.zxing.**

# --- app entry points ------------------------------------------------------
# the platform instantiates these by name from the manifest.
-keep class net.vchub.r4dio.PlaybackService { *; }
-keep class net.vchub.r4dio.MainActivity { *; }
-keep class net.vchub.r4dio.SyncActivity { *; }
-keep class net.vchub.r4dio.RadioWidgetProvider { *; }

# --- crash reports ---------------------------------------------------------
# a stack trace without line numbers cannot be read back to source.
-keepattributes SourceFile, LineNumberTable
-renamesourcefileattribute SourceFile
