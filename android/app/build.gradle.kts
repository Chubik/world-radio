plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.serialization")
}

android {
    namespace = "net.vchub.r4dio"
    compileSdk = 37

    defaultConfig {
        applicationId = "net.vchub.r4dio"
        minSdk = 26
        targetSdk = 37
        versionCode = 9
        versionName = "1.4.5"
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
            isMinifyEnabled = false
            if (!System.getenv("R4DIO_KEYSTORE").isNullOrBlank()) {
                signingConfig = signingConfigs.getByName("release")
            }
        }
    }

    testOptions {
        unitTests.isReturnDefaultValues = true
    }

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
    testImplementation("junit:junit:4.13.2")
    testImplementation("com.squareup.okhttp3:mockwebserver:4.12.0")
}
