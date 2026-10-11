param(
    [Parameter(Mandatory)][int]$ProcessId,
    [int]$Seconds = 20
)
$ErrorActionPreference = 'Stop'
$records = [System.Collections.Generic.List[object]]::new()
$previous = @{}
$timer = [Diagnostics.Stopwatch]::StartNew()
$workerId = $null
do {
    if ($null -eq $workerId) {
        $worker = Get-CimInstance Win32_Process -Filter ("ParentProcessId = {0} AND Name = 'embed-worker.exe'" -f $ProcessId) -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($worker) { $workerId = [int]$worker.ProcessId }
    }
    $elapsed = $timer.Elapsed.TotalSeconds
    $ids = @($ProcessId)
    if ($null -ne $workerId) { $ids += $workerId }
    $workingSet = 0L
    $privateBytes = 0L
    $cpu = 0.0
    foreach ($sampleId in $ids) {
        $process = Get-Process -Id $sampleId -ErrorAction SilentlyContinue
        if (-not $process) { continue }
        $cpuSeconds = $process.TotalProcessorTime.TotalSeconds
        if ($previous.ContainsKey($sampleId)) {
            $interval = $elapsed - $previous[$sampleId].Elapsed
            if ($interval -gt 0) { $cpu += 100 * ($cpuSeconds - $previous[$sampleId].Cpu) / $interval / [Environment]::ProcessorCount }
        }
        $previous[$sampleId] = @{Elapsed=$elapsed;Cpu=$cpuSeconds}
        $workingSet += $process.WorkingSet64
        $privateBytes += $process.PrivateMemorySize64
    }
    $records.Add([pscustomobject]@{Seconds=[math]::Round($elapsed,3);WorkingSetBytes=$workingSet;PrivateBytes=$privateBytes;CpuPercentMachine=[math]::Round($cpu,3)})
    Start-Sleep -Milliseconds 500
} while ($timer.Elapsed.TotalSeconds -lt $Seconds -and (Get-Process -Id $ProcessId -ErrorAction SilentlyContinue))
$stable = @($records | Where-Object Seconds -ge 5)
$report = [pscustomobject]@{
    ProcessId=$ProcessId;WorkerProcessId=$workerId;DurationSeconds=[math]::Round($timer.Elapsed.TotalSeconds,3)
    PeakWorkingSetMB=[math]::Round(($records.WorkingSetBytes | Measure-Object -Maximum).Maximum / 1000000,2)
    FinalWorkingSetMB=[math]::Round($records[-1].WorkingSetBytes / 1000000,2)
    PeakPrivateBytesMB=[math]::Round(($records.PrivateBytes | Measure-Object -Maximum).Maximum / 1000000,2)
    MeanCpuAfterFiveSeconds=[math]::Round(($stable.CpuPercentMachine | Measure-Object -Average).Average,3)
    Notes='Ventana y worker de texto con el modelo cargado; CPU normalizada al equipo. Muestreo de aproximadamente 500 ms. Working set suma páginas compartidas; private bytes es memoria comprometida. No es una prueba prolongada ni un límite garantizado.'
}
$records | Export-Csv -LiteralPath (Join-Path $PSScriptRoot 'app-data\gui-samples.csv') -NoTypeInformation -Encoding utf8
$report | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $PSScriptRoot 'app-data\gui-resources.json') -Encoding utf8
$report | ConvertTo-Json
