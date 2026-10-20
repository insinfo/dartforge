$ErrorActionPreference='Stop';[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
function Get-NormalizedBytes([string]$Path) {
 $text=[Text.Encoding]::UTF8.GetString([IO.File]::ReadAllBytes($Path)).Replace("`r`n","`n")
 return ,([Text.Encoding]::UTF8.GetBytes($text))
}
function Get-NormalizedFileSet([string]$Root,[string[]]$Paths) {
 $ordered=[Collections.Generic.List[string]]::new();foreach($path in $Paths){$ordered.Add($path)};$ordered.Sort([StringComparer]::Ordinal)
 $hash=[Security.Cryptography.SHA256]::Create();$zero=[byte[]]@(0)
 foreach($relative in $ordered){
  $name=[Text.Encoding]::UTF8.GetBytes($relative);[void]$hash.TransformBlock($name,0,$name.Length,$null,0);[void]$hash.TransformBlock($zero,0,1,$null,0)
  $bytes=Get-NormalizedBytes (Join-Path $Root $relative);[void]$hash.TransformBlock($bytes,0,$bytes.Length,$null,0);[void]$hash.TransformBlock($zero,0,1,$null,0)
 }
 [void]$hash.TransformFinalBlock([byte[]]@(),0,0)
 return @{files=$ordered.Count;normalized_sha256=([BitConverter]::ToString($hash.Hash)).Replace('-','').ToLowerInvariant()}
}
$ui='C:\tools\ui';$sdk='C:\tools\dart\3.6.2\dart-sdk';$paths=@()
foreach($directory in @('lib','examples\file_explorer')) {
 Get-ChildItem (Join-Path $ui $directory) -Filter '*.dart' -Recurse -File | ForEach-Object {
  $relative=$_.FullName.Substring($ui.Length+1).Replace('\','/')
  if($relative -notmatch '/(\.dart_tool|build|\.git)/'){$paths+=@($relative)}
 }
}
$locks=@{}
foreach($relative in @('pubspec.yaml','pubspec.lock','examples/file_explorer/pubspec.yaml','examples/file_explorer/pubspec.lock')){
 $hash=[Security.Cryptography.SHA256]::Create();$bytes=Get-NormalizedBytes (Join-Path $ui $relative)
 $locks[$relative]=([BitConverter]::ToString($hash.ComputeHash($bytes))).Replace('-','').ToLowerInvariant()
}
$sdkPaths=@();Get-ChildItem (Join-Path $sdk 'lib') -Recurse -File | Where-Object {$_.Extension -eq '.dart' -or $_.Name -eq 'libraries.json'} | ForEach-Object {$sdkPaths+=@($_.FullName.Substring($sdk.Length+1).Replace('\','/'))}
$report=@{dart=(Get-NormalizedFileSet $ui $paths);locks=$locks;sdk=(Get-NormalizedFileSet $sdk $sdkPaths);sdk_version=(Get-Content (Join-Path $sdk 'version') -Raw).Trim()}
$report | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 '\\Mac\ssd_dock\MyRustProjects\dartforge\target\current-desktop-validation\ui-inputs-windows.json'
$report | ConvertTo-Json -Depth 8 -Compress
