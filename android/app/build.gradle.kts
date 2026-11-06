plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.ulticp.android"
    compileSdk = 35
    buildToolsVersion = "35.0.0"

    defaultConfig {
        applicationId = "com.ulticp.android"
        minSdk = 26
        targetSdk = 35
        versionCode = 5
        versionName = "0.2.2"
    }

    signingConfigs {
        create("release") {
            storeFile = file("ulticp-release.jks")
            storePassword = "UltiCP2026!"
            keyAlias = "ulticp_release"
            keyPassword = "UltiCP2026!"
            enableV1Signing = true
            enableV2Signing = true
            enableV3Signing = true
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("release")
        }
        debug {
            applicationIdSuffix = ""
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }
}

dependencies {
}
