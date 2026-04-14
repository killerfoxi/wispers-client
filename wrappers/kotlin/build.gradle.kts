plugins {
    id("com.android.library")
    kotlin("android")
    `maven-publish`
    signing
}

android {
    namespace = "dev.wispers.connect"
    compileSdk = 34

    defaultConfig {
        minSdk = 23
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        consumerProguardFiles("proguard-rules.pro")
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    publishing {
        singleVariant("release") {
            withSourcesJar()
        }
    }
}

dependencies {
    implementation("net.java.dev.jna:jna:5.17.0@aar")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.7.3")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.7.3")
    implementation("androidx.security:security-crypto:1.1.0-alpha06")

    testImplementation("junit:junit:4.13.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.7.3")
    testImplementation("org.mockito:mockito-core:5.8.0")
    testImplementation("org.mockito.kotlin:mockito-kotlin:5.2.1")

    androidTestImplementation("androidx.test.ext:junit:1.1.5")
    androidTestImplementation("androidx.test:runner:1.5.2")
}

publishing {
    publications {
        register<MavenPublication>("release") {
            groupId = "dev.wispers"
            artifactId = "connect"
            version = findProperty("VERSION_NAME") as String? ?: "0.8.0-rc1"

            afterEvaluate {
                from(components["release"])
            }

            pom {
                name.set("Wispers Connect")
                description.set("Android wrapper for the Wispers Connect peer-to-peer connectivity library")
                url.set("https://wispers.dev")

                licenses {
                    license {
                        name.set("MIT License")
                        url.set("https://github.com/s-te-ch/wispers-client/blob/main/LICENSE")
                    }
                }

                developers {
                    developer {
                        id.set("mbs")
                        name.set("Matthias Scheidegger")
                        email.set("mbs@s-te.ch")
                    }
                }

                scm {
                    url.set("https://github.com/s-te-ch/wispers-client")
                    connection.set("scm:git:git://github.com/s-te-ch/wispers-client.git")
                    developerConnection.set("scm:git:ssh://github.com/s-te-ch/wispers-client.git")
                }
            }
        }
    }

    repositories {
        maven {
            name = "sonatype"
            url = uri("https://central.sonatype.com/repository/maven-central/")

            credentials {
                username = findProperty("ossrhUsername") as String? ?: System.getenv("OSSRH_USERNAME")
                password = findProperty("ossrhPassword") as String? ?: System.getenv("OSSRH_PASSWORD")
            }
        }
    }
}

signing {
    useGpgCmd()
    sign(publishing.publications["release"])
}
