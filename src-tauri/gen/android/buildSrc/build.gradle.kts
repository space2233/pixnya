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
        implementation("org.bouncycastle:bcprov-jdk18on:1.80.2")
        implementation("org.bouncycastle:bcpkix-jdk18on:1.80.2")
        implementation("org.bouncycastle:bcutil-jdk18on:1.80.2")
    }
}

dependencyLocking {
    lockAllConfigurations()
    lockMode.set(LockMode.STRICT)
}
