import org.gradle.api.artifacts.dsl.LockMode

buildscript {
    repositories {
        google()
        mavenCentral()
    }
    dependencies {
        classpath("com.android.tools.build:gradle:8.11.0")
        classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:1.9.25")
        constraints {
            classpath("org.bouncycastle:bcprov-jdk18on:1.80.2")
            classpath("org.bouncycastle:bcpkix-jdk18on:1.80.2")
            classpath("org.bouncycastle:bcutil-jdk18on:1.80.2")
        }
    }
    configurations.classpath {
        resolutionStrategy.activateDependencyLocking()
    }
}

dependencyLocking {
    lockMode.set(LockMode.STRICT)
}

allprojects {
    repositories {
        google()
        mavenCentral()
    }
}

tasks.register("clean").configure {
    delete("build")
}
