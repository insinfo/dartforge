<#
Build and run the real dart_ui File Explorer and native menu smoke with DartForge.
Run in a Windows graphical user session with the matching LLVM DLL available.
#>
param(
    [Parameter(Mandatory=$true)][string]$Compiler,
    [Parameter(Mandatory=$true)][string]$UiRoot,
    [Parameter(Mandatory=$true)][string]$Sdk,
    [Parameter(Mandatory=$true)][string]$Out,
    [string]$LlvmDirectory=$env:DARTFORGE_LLVM_DIR,
    [string]$ExportTarget,
    [ValidateSet('all','aot','jit')][string]$Mode='all'
)
$ErrorActionPreference='Continue'
$ProgressPreference='SilentlyContinue'
$Compiler=(Resolve-Path -LiteralPath $Compiler).Path
$UiRoot=(Resolve-Path -LiteralPath $UiRoot).Path
$Sdk=(Resolve-Path -LiteralPath $Sdk).Path
$Out=[IO.Path]::GetFullPath($Out)
if($ExportTarget){$ExportTarget=[IO.Path]::GetFullPath($ExportTarget)}
New-Item -ItemType Directory -Force $Out | Out-Null
$env:DARTFORGE_NATIVE_CACHE=Join-Path $Out 'native-cache'
$env:DARTFORGE_SDK_LIB=$Sdk
$env:DART_UI_FONTS=Join-Path $UiRoot 'assets\fonts'
if($LlvmDirectory){
    $env:DARTFORGE_LLVM_DIR=$LlvmDirectory
    $env:DARTFORGE_CLANG=Join-Path $LlvmDirectory 'bin\clang.exe'
    $env:Path=(Join-Path $LlvmDirectory 'bin')+';'+$env:Path
}
$example=Join-Path $UiRoot 'examples\file_explorer'
$script:records=@()
$producerHash=(Get-FileHash -Algorithm SHA256 -LiteralPath $Compiler).Hash
function Run-Case($label,$program,$arguments,$marker='') {
    $watch=[Diagnostics.Stopwatch]::StartNew()
    & $program @arguments 1> "$Out\$label.stdout" 2> "$Out\$label.stderr"
    $code=$LASTEXITCODE
    $passed=($code -eq 0)
    if($marker){$passed=$passed -and ((Get-Content "$Out\$label.stdout" -Raw) -match $marker)}
    $record=[pscustomobject]@{name=$label;program=$program;arguments=$arguments;exit=$code;passed=$passed;seconds=$watch.Elapsed.TotalSeconds}
    $script:records += $record
    [pscustomobject]@{compiler=$Compiler;compiler_sha256=$producerHash;mode=$Mode;runs=$script:records} |
        ConvertTo-Json -Depth 6 | Set-Content "$Out\results.json" -Encoding UTF8
    $record | ConvertTo-Json -Compress -Depth 5
    if(-not $passed){Get-Content "$Out\$label.stderr" -Tail 20;throw "$label failed"}
}
Push-Location $example
try {
    if($ExportTarget){Run-Case 'export-target' $Compiler @('export-target',$ExportTarget)}
    $common=@('--sdk',$Sdk,'--packages',"$example\.dart_tool\package_config.json")
    foreach($name in @('explorer','context-smoke')) {
        $entry=if($name -eq 'explorer'){"$example\bin\file_explorer.dart"}else{"$example\tool\context_menu_retarget_smoke.dart"}
        $exe=Join-Path $Out ($name+'.exe')
        $window=@('--backend=win32','--presentation=direct3d11')
        $programArgs=if($name -eq 'explorer'){@($example,'--lang=pt','--frames=2')+$window}else{$window}
        $marker=if($name -eq 'context-smoke'){'CONTEXT_RETARGET=PASS'}else{''}
        if($Mode -ne 'jit'){
            Run-Case "$name-aot-build" $Compiler (@('compile-native',$entry,'--optimize')+$common+@('-o',$exe))
            Run-Case "$name-aot-run" $exe $programArgs $marker
        }
        if($Mode -ne 'aot'){Run-Case "$name-jit-run" $Compiler (@('run',$entry)+$common+@('--')+$programArgs) $marker}
    }
    Write-Output 'WINDOWS_DESKTOP_MATRIX=PASS'
} finally {Pop-Location}
