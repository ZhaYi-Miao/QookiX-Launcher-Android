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
# --release 8 不是可选项：这个 jar 里的类全部是按 Java 8（class 版本 52）编译的，
# 那个版本的 javac 会为「嵌套类访问外层私有成员」生成合成访问器（access$000）。
# 用默认版本（JDK 17/22 → class 55+）编译会启用 Nestmates（JEP 181），改成直接读写私有字段、
# **不再生成访问器** —— 而我们只覆盖外层类，jar 里原版的 GLFW$Functions 仍然
# `invokestatic GLFW.access$000()`，于是启动游戏必然
#   NoSuchMethodError: 'org.lwjgl.system.SharedLibrary org.lwjgl.glfw.GLFW.access$000()'
# （2026-10-09 真机 OnePlus8 实测：MC 26.1.1 / 26.3 都会踩）。
# -encoding UTF-8 同样必需：源码是 UTF-8 带中文注释，不指定时 javac 用平台默认编码
# （中文 Windows 上是 GBK）会直接报「不可映射字符」编译失败。
& $javac --release 8 -nowarn -encoding UTF-8 -cp $classpath -d $outDir @sources
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

# ---- 校验 2：合成访问器契约 ---------------------------------------------------
# 上面那条 `NoSuchMethodError: GLFW.access$000()` 的**回归防线**。
# 我们只覆盖外层类（GLFW / SDLMouse），jar 里它们的嵌套类（GLFW$Functions …）仍是原版；
# 原版嵌套类里可能 `invokestatic` 调用外层类按 Java 8 规则生成的合成访问器。
# 所以把「jar 内嵌套类引用到的 access$NNN，注入后的外层类必须全都提供」钉成硬校验，
# 而不是只靠 --release 8 这个约定 —— javac 换版本/换 flag 时会立刻在这里炸出来，不再等到真机。
Add-Type -AssemblyName System.IO.Compression.FileSystem
function Read-ZipEntryText([System.IO.Compression.ZipArchive]$Zip, [string]$Name) {
    $entry = $Zip.GetEntry($Name)
    if (-not $entry) { return $null }
    $ms = New-Object System.IO.MemoryStream
    $stream = $entry.Open()
    $stream.CopyTo($ms)
    $stream.Close()
    return [System.Text.Encoding]::ASCII.GetString($ms.ToArray())
}
$zip = [System.IO.Compression.ZipFile]::OpenRead($targetJar)
try {
    foreach ($outer in @("org/lwjgl/glfw/GLFW", "org/lwjgl/sdl/SDLMouse")) {
        $outerText = Read-ZipEntryText $zip "$outer.class"
        if (-not $outerText) { throw "jar 里找不到 $outer.class（注入没生效？）" }

        # 同一个 jar 里该外层类的所有嵌套类（GFW$Functions…）引用了哪些合成访问器
        $referenced = @()
        foreach ($entry in $zip.Entries) {
            if ($entry.FullName -like "$outer`$*.class") {
                $text = Read-ZipEntryText $zip $entry.FullName
                if ($text) {
                    $referenced += ([regex]::Matches($text, 'access\$\d+') | ForEach-Object { $_.Value })
                }
            }
        }
        $referenced = @($referenced | Sort-Object -Unique)

        $missing = @($referenced | Where-Object { -not $outerText.Contains($_) })
        if ($missing.Count -gt 0) {
            throw ("$outer.class 缺少 jar 内其余类引用的合成访问器：{0}。" +
                   "确认 javac 带了 --release 8（用 Nestmates 版本编译就不会生成这些访问器）。" -f ($missing -join ', '))
        }
        $shown = if ($referenced.Count -gt 0) { $referenced -join ', ' } else { "无（该 jar 内无嵌套类引用）" }
        Write-Host ("  $outer.class 访问器契约 OK：$shown")
    }

    # 补丁标志（sBridgeInitPending 等）必须仍在，否则注入的是没打补丁的原版类
    $glfwText = Read-ZipEntryText $zip "org/lwjgl/glfw/GLFW.class"
    if (-not ($glfwText.Contains("sBridgeInitPending") -and $glfwText.Contains("sErrorCallbackPending"))) {
        throw "jar 内 GLFW.class 不含补丁标志，检查编译流程"
    }
} finally {
    $zip.Dispose()
}
