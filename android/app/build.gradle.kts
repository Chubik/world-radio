plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.serialization")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "net.vchub.r4dio"
    compileSdk = 37

    defaultConfig {
        applicationId = "net.vchub.r4dio"
        minSdk = 26
        targetSdk = 37
        versionCode = 10
        versionName = "1.22.4"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    // the release key is passed in by the environment, never committed. without
    // it a release build is unsigned rather than silently falling back to the
    // debug key: a debug key is regenerated per machine, and android refuses to
    // update an app whose signature changed, which costs the user every setting
    // they had.
    signingConfigs {
        create("release") {
            val store = System.getenv("R4DIO_KEYSTORE")
            if (!store.isNullOrBlank()) {
                storeFile = file(store)
                storePassword = System.getenv("R4DIO_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("R4DIO_KEY_ALIAS") ?: "r4dio"
                keyPassword = System.getenv("R4DIO_KEY_PASSWORD")
                    ?: System.getenv("R4DIO_KEYSTORE_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            // an unshrunk release ships 11.9 mb where a shrunk one is a fifth of
            // that. what r8 cannot see as reached is listed in proguard-rules.pro,
            // each rule naming what breaks without it.
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
            if (!System.getenv("R4DIO_KEYSTORE").isNullOrBlank()) {
                signingConfig = signingConfigs.getByName("release")
            }
        }
    }

    testOptions {
        // android.util.Log throws in unit tests unless stubbed; CatalogCache logs on
        // its recovery paths, and those tests assert recovery, not logging.
        unitTests.isReturnDefaultValues = true
        unitTests {
            isIncludeAndroidResources = true
        }
    }

    buildFeatures {
        compose = true
    }

}

// robolectric reaches into jdk internals that jdk 17+ hides by default; the
// project's own toolchain is 17, but newer local jdks need these opened
// explicitly or android-all's native shared-memory setup fails.
tasks.withType<Test>().configureEach {
    jvmArgs(
        "--add-opens=java.base/java.lang=ALL-UNNAMED",
        "--add-opens=java.base/java.util=ALL-UNNAMED",
        "--add-opens=java.base/java.io=ALL-UNNAMED",
        "--add-opens=java.base/jdk.internal.access=ALL-UNNAMED",
    )
}

dependencies {
    implementation("androidx.core:core-ktx:1.16.0")
    implementation("androidx.activity:activity:1.10.1")
    implementation("androidx.media3:media3-exoplayer:1.6.1")
    implementation("androidx.media3:media3-exoplayer-hls:1.6.1")
    implementation("androidx.media3:media3-session:1.6.1")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.1")
    implementation("androidx.datastore:datastore-preferences:1.1.1")
    implementation("com.journeyapps:zxing-android-embedded:4.3.0")
    implementation(platform("androidx.compose:compose-bom:2025.09.00"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.9.2")
    debugImplementation("androidx.compose.ui:ui-tooling")
    testImplementation("junit:junit:4.13.2")
    testImplementation("com.squareup.okhttp3:mockwebserver:4.12.0")
    // 4.14.1 caps at sdk 35; this project targets sdk 37 (needs 4.17-beta-1, the
    // first robolectric release with sdk 37 support).
    testImplementation("org.robolectric:robolectric:4.17-beta-1")
}
