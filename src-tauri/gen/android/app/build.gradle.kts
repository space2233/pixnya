import java.util.Properties
import org.gradle.api.artifacts.dsl.LockMode

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

val releaseSigningVariables = mapOf(
    "PIXNYA_ANDROID_KEYSTORE_PATH" to System.getenv("PIXNYA_ANDROID_KEYSTORE_PATH"),
    "PIXNYA_ANDROID_KEYSTORE_PASSWORD" to System.getenv("PIXNYA_ANDROID_KEYSTORE_PASSWORD"),
    "PIXNYA_ANDROID_KEY_ALIAS" to System.getenv("PIXNYA_ANDROID_KEY_ALIAS"),
    "PIXNYA_ANDROID_KEY_PASSWORD" to System.getenv("PIXNYA_ANDROID_KEY_PASSWORD"),
)
val hasCompleteReleaseSigning = releaseSigningVariables.values.all { !it.isNullOrBlank() }

android {
    compileSdk = 36
    namespace = "io.github.space2233.pixnya"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        applicationId = "io.github.space2233.pixnya"
        minSdk = 29
        targetSdk = 36
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
    }
    signingConfigs {
        if (hasCompleteReleaseSigning) {
            create("release") {
                storeFile = file(releaseSigningVariables.getValue("PIXNYA_ANDROID_KEYSTORE_PATH")!!)
                storePassword = releaseSigningVariables.getValue("PIXNYA_ANDROID_KEYSTORE_PASSWORD")
                keyAlias = releaseSigningVariables.getValue("PIXNYA_ANDROID_KEY_ALIAS")
                keyPassword = releaseSigningVariables.getValue("PIXNYA_ANDROID_KEY_PASSWORD")
            }
        }
    }
    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
                jniLibs.keepDebugSymbols.add("*/armeabi-v7a/*.so")
                jniLibs.keepDebugSymbols.add("*/x86/*.so")
                jniLibs.keepDebugSymbols.add("*/x86_64/*.so")
            }
        }
        getByName("release") {
            if (hasCompleteReleaseSigning) {
                signingConfig = signingConfigs.getByName("release")
            }
            isMinifyEnabled = true
            proguardFiles(
                *fileTree(".") { include("**/*.pro") }
                    .plus(getDefaultProguardFile("proguard-android-optimize.txt"))
                    .toList().toTypedArray()
            )
        }
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
    buildFeatures {
        buildConfig = true
    }
}

gradle.taskGraph.whenReady {
    val requestsReleaseArtifact = allTasks.any {
        it.project == project && it.name.contains("Release", ignoreCase = true)
    }
    if (requestsReleaseArtifact && !hasCompleteReleaseSigning) {
        throw GradleException(
            "Release builds require all four Android signing variables: " +
                releaseSigningVariables.keys.joinToString(", ")
        )
    }
}

rust {
    rootDirRel = "../../../"
}

dependencyLocking {
    lockAllConfigurations()
    lockMode.set(LockMode.STRICT)
}

dependencies {
    // Tauri 2.11.5 currently requests Jackson 2.15.3. Pin a current patched
    // release at the application boundary so the vulnerable transitive
    // version is never packaged in the APK.
    implementation("com.fasterxml.jackson.core:jackson-databind:2.22.1")
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = "tauri.build.gradle.kts")
