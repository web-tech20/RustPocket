plugins {
    id("com.android.application")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

android {
    namespace = "com.rustpocket.rustpocket"
    compileSdk = flutter.compileSdkVersion
    ndkVersion = flutter.ndkVersion

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        // TODO: Specify your own unique Application ID (https://developer.android.com/studio/build/application-id.html).
        applicationId = "com.rustpocket.rustpocket"
        // You can update the following values to match your application needs.
        // For more information, see: https://flutter.dev/to/review-gradle-config.
        minSdk = flutter.minSdkVersion
        targetSdk = flutter.targetSdkVersion
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    buildTypes {
        release {
            // TODO: Add your own signing config for the release build.
            // Signing with the debug keys for now, so `flutter run --release` works.
            signingConfig = signingConfigs.getByName("debug")
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

flutter {
    source = "../.."
}

val engineDir = rootProject.projectDir.parentFile.parentFile.resolve("engine")
val jniLibsDir = projectDir.resolve("src/main/jniLibs")

tasks.register<Exec>("buildRustNativeEngine") {
    val home = System.getenv("HOME") ?: System.getProperty("user.home")
    val cargoBin = file("$home/.cargo/bin").absolutePath
    val currentPath = System.getenv("PATH") ?: ""
    val cargo = file("$cargoBin/cargo").let {
        if (it.exists()) it.absolutePath else "cargo"
    }

    inputs.dir(engineDir.resolve("src"))
    inputs.file(engineDir.resolve("Cargo.toml"))
    outputs.dir(jniLibsDir)

    workingDir = engineDir
    environment("PATH", "$cargoBin:$currentPath")
    environment("ANDROID_NDK_HOME", android.ndkDirectory.absolutePath)

    commandLine(
        cargo,
        "ndk",
        "-t", "arm64-v8a",
        "-t", "armeabi-v7a",
        "-t", "x86_64",
        "-o", jniLibsDir.absolutePath,
        "build", "--release"
    )
}

tasks.matching { it.name == "preBuild" }.configureEach {
    dependsOn("buildRustNativeEngine")
}
