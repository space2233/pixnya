import org.gradle.api.artifacts.dsl.LockMode

buildscript {
    repositories {
        google()
        mavenCentral()
    }
    dependencies {
        classpath("com.android.tools.build:gradle:8.11.0")
        classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:2.4.20")
        constraints {
            classpath("org.bouncycastle:bcprov-jdk18on:1.85")
            classpath("org.bouncycastle:bcpkix-jdk18on:1.85")
            classpath("org.bouncycastle:bcutil-jdk18on:1.85")
        }
    }
    configurations.classpath {
        resolutionStrategy.activateDependencyLocking()
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
}

dependencyLocking {
    lockMode.set(LockMode.STRICT)
}

allprojects {
    repositories {
        google()
        mavenCentral()
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
    dependencies.components {
        withModule("com.android.tools:sdk-common") {
            allVariants {
                withDependencies {
                    filter { dependency ->
                        dependency.group == "org.bouncycastle" &&
                            dependency.name in setOf("bcprov-jdk18on", "bcpkix-jdk18on")
                    }.forEach { dependency ->
                        dependency.version { strictly("1.85") }
                    }
                }
            }
        }
    }
}

tasks.register("clean").configure {
    delete("build")
}
