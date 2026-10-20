[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
$root='C:\tools\ui';$paths=[Collections.Generic.List[string]]::new()
foreach($directory in @('lib','examples\file_explorer')) {
 Get-ChildItem (Join-Path $root $directory) -Filter '*.dart' -Recurse -File | ForEach-Object {
  $relative=$_.FullName.Substring($root.Length+1).Replace('\','/')
  if($relative -notmatch '/(\.dart_tool|build|\.git)/'){$paths.Add($relative)}
 }
}
$paths.Sort([StringComparer]::Ordinal);$hash=[Security.Cryptography.SHA256]::Create();$zero=[byte[]]@(0)
foreach($relative in $paths){
 $name=[Text.Encoding]::UTF8.GetBytes($relative);[void]$hash.TransformBlock($name,0,$name.Length,$null,0);[void]$hash.TransformBlock($zero,0,1,$null,0)
 $text=[Text.Encoding]::UTF8.GetString([IO.File]::ReadAllBytes((Join-Path $root $relative))).Replace("`r`n","`n");$bytes=[Text.Encoding]::UTF8.GetBytes($text)
 [void]$hash.TransformBlock($bytes,0,$bytes.Length,$null,0);[void]$hash.TransformBlock($zero,0,1,$null,0)
}
[void]$hash.TransformFinalBlock([byte[]]@(),0,0)
@{files=$paths.Count;normalized_dart_sha256=([BitConverter]::ToString($hash.Hash)).Replace('-','').ToLowerInvariant()} | ConvertTo-Json -Compress
