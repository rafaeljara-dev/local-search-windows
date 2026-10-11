$ErrorActionPreference = 'Stop'
$taskRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$files = @(git -C $taskRoot ls-files)
if ($LASTEXITCODE -ne 0 -or $files.Count -eq 0) { throw 'No hay archivos en el índice de Git para revisar.' }
$findings = @()
foreach ($relative in $files) {
    if ($relative -match '^(app-data|models|tools|llama-source|upstream|measurements)/' -or
        $relative -match '\.(exe|dll|pdb|obj|log|gguf|bin|mp4|jpeg|png)$' -or
        $relative -match '(^|/)\.env($|\.)') {
        $findings += "$relative`: archivo de datos/artefacto que no debe publicarse."
        continue
    }
    # The scanner contains its own detection patterns; review this file manually.
    if ($relative -eq 'scripts/audit-publication.ps1') { continue }
    $contents = [IO.File]::ReadAllText((Join-Path $taskRoot $relative))
    if ($contents -match '(?i)[A-Z]:[\\/]Users[\\/]' -or
        $contents -match '(?i)/home/[^/\s]+/' -or
        $contents -match 'gh[pousr]_[A-Za-z0-9]{30,}' -or
        $contents -match 'sk-[A-Za-z0-9_-]{30,}' -or
        $contents -match '-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----') {
        $findings += "$relative`: posible ruta personal o secreto; revisar sin imprimir su contenido."
    }
}
if ($findings.Count) { $findings | Write-Output; throw 'La revisión de publicación encontró contenido pendiente.' }
Write-Host "Revisión aprobada: $($files.Count) archivos. No sustituye revisar manualmente datos y licencias."
