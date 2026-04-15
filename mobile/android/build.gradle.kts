allprojects {
    repositories {
        google()
        mavenCentral()
    }
}

val newBuildDir: Directory =
    rootProject.layout.buildDirectory
        .dir("../../build")
        .get()
rootProject.layout.buildDirectory.value(newBuildDir)

subprojects {
    val newSubprojectBuildDir: Directory = newBuildDir.dir(project.name)
    project.layout.buildDirectory.value(newSubprojectBuildDir)

    // Back-fill `namespace` for older Flutter plugins (e.g.
    // flutter_bluetooth_serial 0.4.0) that only declare their package in
    // AndroidManifest.xml — AGP 8+ requires it in build.gradle.
    afterEvaluate {
        extensions.findByName("android")?.let { ext ->
            try {
                val cls = ext.javaClass
                val getNs = cls.getMethod("getNamespace")
                if (getNs.invoke(ext) == null) {
                    val manifest = file("src/main/AndroidManifest.xml")
                    if (manifest.exists()) {
                        val pkg = Regex("""package\s*=\s*"([^"]+)"""")
                            .find(manifest.readText())?.groupValues?.getOrNull(1)
                        if (pkg != null) {
                            cls.getMethod("setNamespace", String::class.java)
                                .invoke(ext, pkg)
                        }
                    }
                }
                // Bump compileSdk to 34 for older plugins built against
                // API 28/29 — fixes `attr/lStar` resource-link failures.
                try {
                    cls.getMethod("setCompileSdkVersion", Int::class.javaPrimitiveType)
                        .invoke(ext, 36)
                } catch (_: Throwable) { /* newer AGP uses setCompileSdk */ }
            } catch (_: Throwable) { /* not an android subproject */ }
        }
    }
}
subprojects {
    project.evaluationDependsOn(":app")
}

tasks.register<Delete>("clean") {
    delete(rootProject.layout.buildDirectory)
}
