param([switch]$TextOnly)
$ErrorActionPreference = 'Stop'
$taskRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$modelRevision = 'bfcd298762cc34d0357ece5ebdd31791a3a374d8'
$sourceRevision = '4d756bc72bf00a4aacf410ae15a2d315f3db400d'
$assets = @(
    @{Path='models/embeddinggemma-2-Q8_0.gguf';Url="https://huggingface.co/ggml-org/embeddinggemma-2-GGUF/resolve/$modelRevision/embeddinggemma-2-Q8_0.gguf";Sha='2188ac1deca4b77dffefd603c2776a9d76d9d74ec01841392982ebb840b09135'}
)
if (-not $TextOnly) {
    $assets += @(
        @{Path='models/mmproj-embeddinggemma-2-Q8_0.gguf';Url="https://huggingface.co/ggml-org/embeddinggemma-2-GGUF/resolve/$modelRevision/mmproj-embeddinggemma-2-Q8_0.gguf";Sha='c4a8a52691ecef40618438928bdf9e68379b854e24166f292592353db0aab64f'},
        @{Path='models/ggml-base.bin';Url='https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-base.bin';Sha='60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe'},
        @{Path='tools/whisper-b5454-x64.zip';Url='https://github.com/ggml-org/whisper.cpp/releases/download/b5454/whisper-bin-x64.zip';Sha='6ba69e3482d7826214f90a6a9c84ca07782aec1e1d0c6a7c30c994fd5d816ccb'}
    )
}
foreach ($asset in $assets) {
    $target = Join-Path $taskRoot $asset.Path
    New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
    if (Test-Path -LiteralPath $target) {
        if ((Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash -ne $asset.Sha) {
            throw "El archivo existente tiene un hash diferente: $target. No se reemplazó."
        }
        Write-Host "Verificado: $($asset.Path)"
        continue
    }
    $download = "$target.download"
    Invoke-WebRequest -Uri $asset.Url -OutFile $download
    if ((Get-FileHash -LiteralPath $download -Algorithm SHA256).Hash -ne $asset.Sha) {
        Remove-Item -LiteralPath $download
        throw "SHA-256 incorrecto: $($asset.Path)"
    }
    Move-Item -LiteralPath $download -Destination $target
}
if (-not $TextOnly) {
    Expand-Archive -LiteralPath (Join-Path $taskRoot 'tools/whisper-b5454-x64.zip') -DestinationPath (Join-Path $taskRoot 'tools/whisper-b5454') -Force
    if (-not (Test-Path -LiteralPath (Join-Path $taskRoot 'tools/whisper-b5454/Release/whisper-cli.exe'))) {
        throw 'La distribución de Whisper no tiene la estructura esperada.'
    }
}
$source = Join-Path $taskRoot 'llama-source'
if (-not (Test-Path -LiteralPath $source)) {
    git clone --depth 1 --branch b11461 https://github.com/ggml-org/llama.cpp.git $source
    if ($LASTEXITCODE -ne 0) { throw 'Falló la descarga de llama.cpp.' }
}
$actualRevision = git -C $source rev-parse HEAD
if ($LASTEXITCODE -ne 0 -or $actualRevision -ne $sourceRevision) { throw 'La revisión de llama.cpp no coincide con la fijada.' }
$patch = Join-Path $taskRoot 'patches/llama-no-logits.patch'
git -C $source apply --reverse --check $patch 2>$null
if ($LASTEXITCODE -ne 0) {
    git -C $source apply --check $patch
    if ($LASTEXITCODE -ne 0) { throw 'No se puede aplicar el parche; revisar cambios locales.' }
    git -C $source apply $patch
    if ($LASTEXITCODE -ne 0) { throw 'Falló el parche de embeddings no causales.' }
}
Write-Host 'Dependencias preparadas. Compila el runtime y el worker según docs/BUILD.md.'
