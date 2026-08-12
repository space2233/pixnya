import java.util.Properties
import org.gradle.api.artifacts.dsl.LockMode
import org.gradle.api.artifacts.ExternalModuleDependency
import org.gradle.api.artifacts.ProjectDependency
import org.gradle.api.artifacts.component.ModuleComponentIdentifier
import org.gradle.api.artifacts.component.ProjectComponentIdentifier

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

tasks.register("resolveLockedDependencies") {
    group = "verification"
    description = "Resolves every configuration represented by the reviewed Gradle lock state."

    doLast {
        val lockFile = file("gradle.lockfile")
        if (!lockFile.isFile) {
            throw GradleException("The reviewed app/gradle.lockfile is missing.")
        }
        val lockedCoordinatesByConfiguration = sortedMapOf<String, MutableSet<String>>()
        lockFile.readLines()
            .asSequence()
            .map(String::trim)
            .filter { it.isNotEmpty() && !it.startsWith("#") && it.contains('=') }
            .forEach { line ->
                val coordinate = line.substringBeforeLast('=')
                line.substringAfterLast('=')
                    .split(',')
                    .map(String::trim)
                    .filter(String::isNotEmpty)
                    .forEach { configurationName ->
                        val coordinates = lockedCoordinatesByConfiguration
                            .getOrPut(configurationName) { sortedSetOf() }
                        if (coordinate != "empty") {
                            coordinates.add(coordinate)
                        }
                    }
            }
        val lockedConfigurationNames = lockedCoordinatesByConfiguration.keys

        val requiredReleaseConfigurations = setOf(
            "arm64ReleaseCompileClasspath",
            "arm64ReleaseRuntimeClasspath",
            "armReleaseCompileClasspath",
            "armReleaseRuntimeClasspath",
        )
        val missingReleaseLockState = requiredReleaseConfigurations - lockedConfigurationNames
        if (missingReleaseLockState.isNotEmpty()) {
            throw GradleException(
                "The Android ARM release graph is missing reviewed lock state for: " +
                    missingReleaseLockState.sorted().joinToString(", ")
            )
        }

        val missingConfigurations = lockedConfigurationNames.filter {
            configurations.findByName(it) == null
        }
        if (missingConfigurations.isNotEmpty()) {
            throw GradleException(
                "The lockfile references configurations that no longer exist: " +
                    missingConfigurations.joinToString(", ")
            )
        }
        val unresolvableConfigurations = lockedConfigurationNames.filter {
            configurations.getByName(it).isCanBeResolved.not()
        }
        if (unresolvableConfigurations.isNotEmpty()) {
            throw GradleException(
                "The lockfile references configurations that can no longer be resolved: " +
                    unresolvableConfigurations.joinToString(", ")
            )
        }

        fun declaredCoordinate(group: String?, name: String, version: String?): String {
            if (group.isNullOrBlank() || version.isNullOrBlank()) {
                throw GradleException("Unlocked external declaration is not pinned: $group:$name:$version")
            }
            return "$group:$name:$version"
        }

        val releaseCompileConfigurations = listOf(
            "arm64ReleaseCompileClasspath",
            "armReleaseCompileClasspath",
        )
        val releaseRuntimeConfigurations = listOf(
            "arm64ReleaseRuntimeClasspath",
            "armReleaseRuntimeClasspath",
        )
        val releaseCompileLocks = releaseCompileConfigurations.map(
            lockedCoordinatesByConfiguration::getValue
        )
        val releaseRuntimeLocks = releaseRuntimeConfigurations.map(
            lockedCoordinatesByConfiguration::getValue
        )
        if (releaseCompileLocks.map { it.toSortedSet() }.distinct().size != 1) {
            throw GradleException("Android ARM64 and ARMv7 release compile lock graphs differ")
        }
        if (releaseRuntimeLocks.map { it.toSortedSet() }.distinct().size != 1) {
            throw GradleException("Android ARM64 and ARMv7 release runtime lock graphs differ")
        }
        val reviewedMetadataConfiguration = "implementationDependenciesMetadata"
        var reviewedMetadataConfigurationObserved = false
        val unlockedExternalGraphs = mutableListOf<String>()
        configurations
            .filter { it.isCanBeResolved && it.name !in lockedConfigurationNames }
            .sortedBy { it.name }
            .forEach { configuration ->
                val modules = configuration.allDependencies
                    .filterIsInstance<ExternalModuleDependency>()
                    .map { declaredCoordinate(it.group, it.name, it.version) }
                    .toSortedSet()
                val constraints = configuration.allDependencyConstraints
                    .map {
                        declaredCoordinate(
                            it.group,
                            it.name,
                            it.versionConstraint.requiredVersion.takeIf(String::isNotBlank),
                        )
                    }
                    .toSortedSet()
                if (modules.isEmpty() && constraints.isEmpty()) {
                    return@forEach
                }

                val projectPaths = configuration.allDependencies
                    .filterIsInstance<ProjectDependency>()
                    .map { it.path }
                    .toSortedSet()
                val reviewedCoordinates = modules + constraints
                val coveredByAllReleaseGraphs = reviewedCoordinates.all { coordinate ->
                    releaseCompileLocks.all { coordinate in it } &&
                        releaseRuntimeLocks.all { coordinate in it }
                }
                if (
                    configuration.name == reviewedMetadataConfiguration &&
                    projectPaths == setOf(":tauri-android") &&
                    coveredByAllReleaseGraphs
                ) {
                    // Tauri's project dependency makes this Kotlin common-metadata
                    // configuration variant-ambiguous between debug and release.
                    // Formal ARM variants are resolvable, so prove every external
                    // declaration is covered by all reviewed release lock graphs
                    // without attempting to resolve this invalid internal surface.
                    reviewedMetadataConfigurationObserved = true
                    logger.lifecycle(
                        "Verified {} against all Android ARM release lock graphs without resolving it",
                        reviewedMetadataConfiguration,
                    )
                } else {
                    unlockedExternalGraphs.add(
                        "${configuration.name} [modules=${modules.joinToString(";")}; " +
                            "constraints=${constraints.joinToString(";")}; " +
                            "projects=${projectPaths.joinToString(";")}]"
                    )
                }
            }
        if (unlockedExternalGraphs.isNotEmpty()) {
            throw GradleException(
                "Resolvable configurations with external dependencies lack reviewed lock state:\n" +
                    unlockedExternalGraphs.joinToString("\n")
            )
        }
        if (!reviewedMetadataConfigurationObserved) {
            throw GradleException(
                "The exact $reviewedMetadataConfiguration variant-ambiguity exception " +
                    "was not observed; review the dependency-lock boundary again."
            )
        }

        lockedConfigurationNames.forEach { name ->
            val componentCount = configurations.getByName(name)
                .incoming
                .resolutionResult
                .allComponents
                .size
            logger.lifecycle("Resolved locked configuration {} ({} components)", name, componentCount)
        }

        releaseRuntimeConfigurations.forEach { configurationName ->
            val releaseRuntimeComponents = configurations
                .getByName(configurationName)
                .incoming
                .resolutionResult
                .allComponents
                .map { it.id }
            val includesTauriAndroid = releaseRuntimeComponents.any {
                it is ProjectComponentIdentifier && it.projectPath == ":tauri-android"
            }
            if (!includesTauriAndroid) {
                throw GradleException("$configurationName does not contain project :tauri-android")
            }
            val includesReviewedJackson = releaseRuntimeComponents.any {
                it is ModuleComponentIdentifier &&
                    it.group == "com.fasterxml.jackson.core" &&
                    it.module == "jackson-databind" &&
                    it.version == "2.22.1"
            }
            if (!includesReviewedJackson) {
                throw GradleException(
                    "$configurationName does not contain " +
                        "com.fasterxml.jackson.core:jackson-databind:2.22.1"
                )
            }
        }
    }
}
