<#
.SYNOPSIS
  把打了补丁的 GLFW.class 编译并注入 LWJGL 3.4.1 组件 jar。

.DESCRIPTION
  MC 26.3 用的 fork 版 liblwjgl 会在自己的 JNI_OnLoad 里触发 GLFW 类初始化，而那一刻
  LibFFI（在同一个库）还没加载完、GLFW 的桥函数也解析不到，导致类初始化失败并崩游戏。
  补丁把这两件事延后到 glfwInit()。详见
  `scripts/lwjgl-glfw-patch/src/org/lwjgl/glfw/GLFW.java` 里那段补丁注释。

  脚本是**幂等**的：改完补丁源码重跑一次即可覆盖式更新 jar 里那个 class。
  打包插件（make-plugin-zip.ps1）与内置组件（assets-components）都从同一个 jar 取，
  所以这里只改一处。

.PARAMETER JarDir
  组件 jar 所在目录，默认取 assets-components 里那份。
#>
param(
    [string]$JarDir = (Join-Path $PSScriptRoot "..\src-tauri\gen\android\app\src\main\assets-components\components\lwjgl3\3.4.1")
)

$ErrorActionPreference = "Stop"

function Find-JdkTool([string]$Name) {
    $candidates = @()
    if ($env:JAVA_HOME) { $candidates += (Join-Path $env:JAVA_HOME "bin\$Name.exe") }
    $cmd = Get-Command "$Name.exe" -ErrorAction SilentlyContinue
    if ($cmd) { $candidates += $cmd.Source }
    $candidates += "C:\Program Files\Java\jdk-17\bin\$Name.exe"
    foreach ($c in $candidates) { if ($c -and (Test-Path $c)) { return $c } }
    throw "找不到 $Name（需要 JDK，不是 JRE）。可用 JAVA_HOME 指定。"
}

$javac = Find-JdkTool "javac"
$jar   = Find-JdkTool "jar"

$srcDir = Join-Path $PSScriptRoot "lwjgl-glfw-patch\src"
$outDir = Join-Path $PSScriptRoot "lwjgl-glfw-patch\build"

$targetJar = Join-Path $JarDir "lwjgl-3.4.1-merged-modules.jar"
if (-not (Test-Path $targetJar)) { throw "找不到目标 jar：$targetJar" }
if (-not (Test-Path $srcDir)) { throw "找不到补丁源码目录：$srcDir" }

Write-Host "== 编译补丁类 =="
if (Test-Path $outDir) { Remove-Item $outDir -Recurse -Force }
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

$classpath = (Get-ChildItem $JarDir -Filter *.jar | ForEach-Object { $_.FullName }) -join ";"
$sources = Get-ChildItem $srcDir -Recurse -Filter *.java | ForEach-Object { $_.FullName }
& $javac -nowarn -cp $classpath -d $outDir @sources
if ($LASTEXITCODE -ne 0) { throw "javac 失败（退出码 $LASTEXITCODE）" }

$patched = Join-Path $outDir "org\lwjgl\glfw\GLFW.class"
if (-not (Test-Path $patched)) { throw "没有产出 GLFW.class" }
$patchedSdl = Join-Path $outDir "org\lwjgl\sdl\SDLMouse.class"

Write-Host "== 注入 jar =="
$work = Join-Path $outDir "_inject"
New-Item -ItemType Directory -Force -Path (Join-Path $work "org\lwjgl\glfw") | Out-Null
Copy-Item $patched (Join-Path $work "org\lwjgl\glfw\GLFW.class") -Force
Push-Location $work
& $jar uf $targetJar "org/lwjgl/glfw/GLFW.class"
$code = $LASTEXITCODE
Pop-Location
if ($code -ne 0) { throw "jar 注入失败（退出码 $code）" }

# SDLMouse：MC 26.3 的鼠标抓取走 SDL（视角问题排查/桥接），同样打补丁注入
if (Test-Path $patchedSdl) {
    New-Item -ItemType Directory -Force -Path (Join-Path $work "org\lwjgl\sdl") | Out-Null
    Copy-Item $patchedSdl (Join-Path $work "org\lwjgl\sdl\SDLMouse.class") -Force
    Push-Location $work
    & $jar uf $targetJar "org/lwjgl/sdl/SDLMouse.class"
    $code = $LASTEXITCODE
    Pop-Location
    if ($code -ne 0) { throw "jar 注入 SDLMouse 失败（退出码 $code）" }
    Write-Host "  SDLMouse.class 已注入"
}

Write-Host "== 校验 =="
$verifyDir = Join-Path $outDir "_verify"
New-Item -ItemType Directory -Force -Path $verifyDir | Out-Null
Push-Location $verifyDir
& $jar xf $targetJar "org/lwjgl/glfw/GLFW.class"
Pop-Location
$bytes = [System.IO.File]::ReadAllBytes((Join-Path $verifyDir "org\lwjgl\glfw\GLFW.class"))
$text = [System.Text.Encoding]::ASCII.GetString($bytes)
$ok1 = $text.Contains("sBridgeInitPending")
$ok2 = $text.Contains("sErrorCallbackPending")
Write-Host ("  jar 内 GLFW.class 含补丁标志: bridge=$ok1 errorCallback=$ok2")
if (-not ($ok1 -and $ok2)) { throw "注入后的 class 不含补丁标志，检查编译流程" }
Write-Host ("  目标 jar: " + (Resolve-Path $targetJar).Path)
