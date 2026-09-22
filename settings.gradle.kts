pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        maven { setUrl("https://maven.pkg.jetbrains.space/public/p/compose/jetbrains") }
    }
}

rootProject.name = "QookiX-Launcher"
include(":app")
