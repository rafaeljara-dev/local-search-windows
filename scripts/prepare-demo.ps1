$ErrorActionPreference = 'Stop'
$taskRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$corpus = Get-Content -Raw -LiteralPath (Join-Path $taskRoot 'examples/demo-corpus.json') | ConvertFrom-Json
$vectors = Get-Content -Raw -LiteralPath (Join-Path $taskRoot 'examples/demo-vectors.json') | ConvertFrom-Json
$byId = @{}
foreach ($entry in $vectors) { $byId[$entry.id] = $entry.vector }
$folder = Join-Path $taskRoot 'app-data/examples'
New-Item -ItemType Directory -Path $folder -Force | Out-Null
$utf8NoBom = [Text.UTF8Encoding]::new($false)
$documents = foreach ($entry in $corpus.documents) {
    $identifier = [string]$entry[0]
    if ($identifier -notmatch '^[a-z0-9-]+$' -or $byId[$identifier].Count -ne 768) {
        throw 'Entrada de demostración inválida.'
    }
    $path = Join-Path $folder "$identifier.txt"
    [IO.File]::WriteAllText($path,[string]$entry[2],$utf8NoBom)
    @{title=$entry[1];path=$path;kind=$(if ($entry[1].EndsWith('.py')) {'Código'} else {'Texto'});segments=@(@{text=$entry[2];line=1;vector=$byId[$identifier]})}
}
$catalog = @{pipeline='embeddinggemma2-q8-b11461-avx2-768-v1';source=$folder;demo=$true;documents=@($documents);notice='Documentos sintéticos de demostración; no son archivos personales.'}
[IO.File]::WriteAllText((Join-Path $taskRoot 'app-data/demo.json'),($catalog | ConvertTo-Json -Depth 10),$utf8NoBom)
Write-Host "Demostración lista: $(@($documents).Count) documentos. El catálogo personal no se modificó."
