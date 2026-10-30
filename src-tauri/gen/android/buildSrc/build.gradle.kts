import org.gradle.api.artifacts.dsl.LockMode

plugins {
    `kotlin-dsl`
}

gradlePlugin {
    plugins {
        create("pluginsForCoolKids") {
            id = "rust"
            implementationClass = "RustPlugin"
        }
    }
}

repositories {
    google()
    mavenCentral()
}

dependencies {
    compileOnly(gradleApi())
    implementation("com.android.tools.build:gradle:8.11.0")
    constraints {
        implementation("org.bouncycastle:bcprov-jdk18on:1.85")
        implementation("org.bouncycastle:bcpkix-jdk18on:1.85")
        implementation("org.bouncycastle:bcutil-jdk18on:1.85")
    }
}

configurations.configureEach {
    resolutionStrategy.eachDependency {
        val patchedVersion = when (requested.group) {
            "com.google.guava" -> if (requested.name == "guava") "33.7.2-jre" else null
            "org.bouncycastle" -> "1.85"
            "io.netty" -> "4.1.137.Final"
            "com.google.protobuf" -> "3.25.5"
            "org.apache.commons" -> when (requested.name) {
                "commons-compress" -> "1.26.0"
                "commons-lang3" -> "3.18.0"
                else -> null
            }
            "org.bitbucket.b_c" -> if (requested.name == "jose4j") "0.9.6" else null
            "org.jdom" -> if (requested.name == "jdom2") "2.0.6.1" else null
            else -> null
        }
        if (patchedVersion != null) useVersion(patchedVersion)
    }
}

dependencyLocking {
    lockAllConfigurations()
    lockMode.set(LockMode.STRICT)
}
