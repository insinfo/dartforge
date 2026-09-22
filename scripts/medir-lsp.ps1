<#
.SYNOPSIS
    Mede o RSS do servidor LSP sob a mesma sequência de mensagens no mesmo projeto.

.DESCRIPTION
    Inicia o `dartforge-lsp --stdio` (ou, com `-Dart`, o `dart
    language-server --protocol=lsp`), envia `initialize`, abre todos os
    `.dart` do corpus, aplica edições incrementais distribuídas e amostra o
    RSS via `Get-Process` a cada 50 mensagens enviadas. No fim envia
    `shutdown` + `exit` e imprime pico, platô (mediana das últimas 10
    amostras) e quadros recebidos.

    A comparação honesta é no mesmo projeto, com a mesma sequência: rode uma
    vez sem e uma vez com `-Dart` e compare as tabelas (ver `docs/LSP.md`).

.EXAMPLE
    .\scripts\medir-lsp.ps1
    .\scripts\medir-lsp.ps1 -Dart
    .\scripts\medir-lsp.ps1 -Corpus C:\outro\projeto -Edicoes 100
#>
[CmdletBinding()]
param(
    [string]$BinPath = "",
    [string]$Corpus = "C:/MyDartProjects/new_sali",
    [switch]$Dart,
    [string]$DartExe = "dart",
    [int]$Edicoes = 200,
    [int]$AmostraACada = 50,
    [int]$TimeoutSeg = 1200,
    [int]$EsperaFinalSeg = 0
)

$ErrorActionPreference = "Stop"

if ($BinPath -eq "" -and -not $Dart) {
    $BinPath = Join-Path (Split-Path $PSScriptRoot -Parent) "target\release\dartforge-lsp.exe"
    if (-not (Test-Path $BinPath)) {
        $BinPath = Join-Path (Split-Path $PSScriptRoot -Parent) "target\debug\dartforge-lsp.exe"
    }
}

Add-Type -TypeDefinition @"
using System;
using System.IO;
using System.Text;
using System.Threading.Tasks;

public static class DrenoLsp {
    // Drena o stdout até o EOF numa thread do pool, sem runspace do
    // PowerShell: conta quadros Content-Length e grava o bruto em arquivo.
    public static Task<int> Iniciar(Stream origem, string destino) {
        return Task.Run(() => Drenar(origem, destino));
    }

    static int Drenar(Stream origem, string destino) {
        using (var saida = new FileStream(destino, FileMode.Create, FileAccess.Write)) {
            var quadros = 0;
            var cabeca = new StringBuilder();
            int anterior = -1;
            while (true) {
                int b = origem.ReadByte();
                if (b < 0) break;
                saida.WriteByte((byte)b);
                cabeca.Append((char)b);
                bool fimLinha = (anterior == '\r' && b == '\n');
                anterior = b;
                if (!fimLinha) continue;
                string linha = cabeca.ToString();
                cabeca.Clear();
                if (linha == "\r\n") continue;
                if (linha.StartsWith("Content-Length:")) {
                    int n = int.Parse(linha.Substring("Content-Length:".Length).Trim());
                    // Linha vazia após o cabeçalho.
                    int c1 = origem.ReadByte(); saida.WriteByte((byte)c1);
                    int c2 = origem.ReadByte(); saida.WriteByte((byte)c2);
                    var corpo = new byte[n];
                    int lidos = 0;
                    while (lidos < n) {
                        int r = origem.Read(corpo, lidos, n - lidos);
                        if (r <= 0) break;
                        lidos += r;
                    }
                    saida.Write(corpo, 0, lidos);
                    anterior = -1;
                    quadros++;
                }
            }
            return quadros;
        }
    }
}
"@

function Get-ArquivosDart([string]$raiz) {
    Get-ChildItem -LiteralPath $raiz -Recurse -Filter *.dart -File |
        Where-Object { $_.FullName -notmatch '[\\/]\.(dart_tool|build)[\\/]' } |
        Sort-Object FullName
}

function ConvertTo-UriArquivo([string]$caminho) {
    $abs = [System.IO.Path]::GetFullPath($caminho) -replace '\\', '/'
    "file:///$abs"
}

function New-Quadro([string]$json) {
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($json)
    @{ Cabeca = "Content-Length: $($bytes.Length)`r`n`r`n"; Corpo = $json }
}

$arquivos = Get-ArquivosDart $Corpus
if ($arquivos.Count -eq 0) { throw "nenhum .dart em $Corpus" }
Write-Host ("arquivos: {0}" -f $arquivos.Count)

# --- sequência de mensagens (idêntica nos dois modos) ---
$mensagens = [System.Collections.Generic.List[string]]::new()
$mensagens.Add((@{ jsonrpc = "2.0"; id = 1; method = "initialize"; params = @{
    processId = $PID; rootUri = (ConvertTo-UriArquivo $Corpus); capabilities = @{} } } |
    ConvertTo-Json -Depth 10 -Compress))
$mensagens.Add((@{ jsonrpc = "2.0"; method = "initialized"; params = @{} } |
    ConvertTo-Json -Depth 10 -Compress))

$uris = @()
$versoes = @{}
foreach ($arq in $arquivos) {
    $uri = ConvertTo-UriArquivo $arq.FullName
    $uris += $uri
    $versoes[$uri] = 1
    $texto = [System.IO.File]::ReadAllText($arq.FullName)
    $mensagens.Add((@{ jsonrpc = "2.0"; method = "textDocument/didOpen"; params = @{
        textDocument = @{ uri = $uri; languageId = "dart"; version = 1; text = $texto } } } |
        ConvertTo-Json -Depth 10 -Compress))
}
$alternar = @{}
for ($j = 0; $j -lt $Edicoes; $j++) {
    $uri = $uris[$j % $uris.Count]
    $versoes[$uri]++
    if (-not $alternar.ContainsKey($uri)) { $alternar[$uri] = $true }
    if ($alternar[$uri]) {
        $mudanca = @(@{ range = @{ start = @{ line = 0; character = 0 }; end = @{ line = 0; character = 0 } }; text = " " })
    } else {
        $mudanca = @(@{ range = @{ start = @{ line = 0; character = 0 }; end = @{ line = 0; character = 1 } }; text = "" })
    }
    $alternar[$uri] = -not $alternar[$uri]
    $mensagens.Add((@{ jsonrpc = "2.0"; method = "textDocument/didChange"; params = @{
        textDocument = @{ uri = $uri; version = $versoes[$uri] }; contentChanges = $mudanca } } |
        ConvertTo-Json -Depth 10 -Compress))
}
$mensagens.Add((@{ jsonrpc = "2.0"; id = 2; method = "shutdown" } | ConvertTo-Json -Compress))
$mensagens.Add((@{ jsonrpc = "2.0"; method = "exit" } | ConvertTo-Json -Compress))
Write-Host ("mensagens: {0} (didOpen: {1}, edições: {2})" -f $mensagens.Count, $arquivos.Count, $Edicoes)

# --- processo ---
if ($Dart) {
    $psi = New-Object System.Diagnostics.ProcessStartInfo($DartExe, "language-server --protocol=lsp")
    $modo = "dart"
} else {
    if (-not (Test-Path $BinPath)) { throw "binário não encontrado: $BinPath (rode cargo build --release -p dartforge-lsp)" }
    $psi = New-Object System.Diagnostics.ProcessStartInfo($BinPath, "--stdio")
    $modo = "dartforge"
}
$psi.UseShellExecute = $false
$psi.RedirectStandardInput = $true
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$proc = [System.Diagnostics.Process]::Start($psi)

$stdoutArquivo = Join-Path ([System.IO.Path]::GetTempPath()) ("lsp-stdout-{0}.bin" -f $proc.Id)
$tarefa = [DrenoLsp]::Iniciar($proc.StandardOutput.BaseStream, $stdoutArquivo)
$escritor = New-Object System.IO.StreamWriter($proc.StandardInput.BaseStream, (New-Object System.Text.UTF8Encoding($false)))

$amostras = [System.Collections.Generic.List[double]]::new()
$enviadas = 0
$cronometro = [System.Diagnostics.Stopwatch]::StartNew()
try {
    $corpoSeq = $mensagens.Count - 2  # shutdown + exit vão por último, após a espera
    foreach ($json in $mensagens[0..($corpoSeq - 1)]) {
        if ($proc.HasExited) { break }
        $quadro = New-Quadro $json
        # Tudo pelo StreamWriter (UTF-8 sem BOM): misturar escrita
        # bufferizada com escrita direta no BaseStream reordena os bytes.
        $escritor.Write($quadro.Cabeca)
        $escritor.Write($quadro.Corpo)
        $enviadas++
        if ($enviadas % $AmostraACada -eq 0) {
            $escritor.Flush()
            Start-Sleep -Milliseconds 50
            if (-not $proc.HasExited) {
                $rss = (Get-Process -Id $proc.Id -ErrorAction SilentlyContinue).WorkingSet64
                if ($rss) { $amostras.Add($rss / 1MB) }
            }
        }
        if ($cronometro.Elapsed.TotalSeconds -gt $TimeoutSeg) { throw "tempo esgotado com o servidor ainda processando" }
    }
    # Espera final (antes do shutdown): dá ao servidor assíncrono a chance de
    # analisar o que foi aberto, amostrando o RSS a cada segundo. A sequência
    # de mensagens não muda — só o tempo antes de encerrar.
    $fimEspera = [DateTime]::UtcNow.AddSeconds($EsperaFinalSeg)
    while ([DateTime]::UtcNow -lt $fimEspera) {
        if ($proc.HasExited) { break }
        Start-Sleep -Seconds 1
        if (-not $proc.HasExited) {
            $rss = (Get-Process -Id $proc.Id -ErrorAction SilentlyContinue).WorkingSet64
            if ($rss) { $amostras.Add($rss / 1MB) }
        }
    }
    if (-not $proc.HasExited) {
        foreach ($json in $mensagens[($mensagens.Count - 2)..($mensagens.Count - 1)]) {
            $quadro = New-Quadro $json
            $escritor.Write($quadro.Cabeca)
            $escritor.Write($quadro.Corpo)
            $enviadas++
        }
    }
    $escritor.Flush()
    $escritor.Close()
    if (-not $proc.WaitForExit($TimeoutSeg * 1000)) { throw "servidor não encerrou após exit" }
} finally {
    if (-not $escritor.BaseStream.CanWrite) { } else { $escritor.Close() }
}
$quadrosRecebidos = $tarefa.GetAwaiter().GetResult()
$codigoSaida = $proc.ExitCode
$tempoTotal = $cronometro.Elapsed

$pico = ($amostras | Measure-Object -Maximum).Maximum
$ultimas = $amostras | Select-Object -Last 10
$plato = ($ultimas | Measure-Object -Average).Average
$final = ($amostras | Select-Object -Last 1)

Write-Host ""
Write-Host ("modo: {0} | projeto: {1}" -f $modo, $Corpus)
Write-Host ("mensagens enviadas: {0} | quadros recebidos: {1} | saída: {2} | tempo: {3}" -f $enviadas, $quadrosRecebidos, $codigoSaida, $tempoTotal)
Write-Host ("amostras RSS: {0} (1 a cada {1} mensagens)" -f $amostras.Count, $AmostraACada)
Write-Host ("pico RSS: {0:N1} MiB | platô RSS (média das últimas 10): {1:N1} MiB | última: {2:N1} MiB" -f $pico, $plato, $final)
[PSCustomObject]@{
    modo = $modo; projeto = $Corpus; arquivos = $arquivos.Count; edicoes = $Edicoes
    enviadas = $enviadas; quadrosRecebidos = $quadrosRecebidos
    picoMiB = [math]::Round($pico, 1); platoMiB = [math]::Round($plato, 1)
    ultimaMiB = [math]::Round($final, 1); segundos = [math]::Round($tempoTotal.TotalSeconds, 1)
}
