param(
    [Parameter(Mandatory)][ValidateSet('index-folder','index-selected','index-images','probe-queries')][string]$Mode,
    [Parameter(Mandatory)][string]$InputPath,
    [Parameter(Mandatory)][string]$ReportName
)
$ErrorActionPreference = 'Stop'
$exe = Join-Path $PSScriptRoot 'app\target\release\local-search-preview.exe'
$outputDirectory = Join-Path $PSScriptRoot 'app-data'
$records = [System.Collections.Generic.List[object]]::new()
$previous = @{}
$logical = [Environment]::ProcessorCount
$started = [DateTimeOffset]::UtcNow.ToString('o')
$timer = [Diagnostics.Stopwatch]::StartNew()
$probeProcess = Start-Process -FilePath $exe -ArgumentList ('--{0} "{1}"' -f $Mode, $InputPath) -WindowStyle Hidden -PassThru
$childId = $null
do {
    if ($null -eq $childId) {
        $child = Get-CimInstance Win32_Process -Filter ("ParentProcessId = {0} AND Name = 'embed-worker.exe'" -f $probeProcess.Id) -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($child) { $childId = [int]$child.ProcessId }
    }
    $elapsed = $timer.Elapsed.TotalSeconds
    $sample = @($probeProcess.Id)
    if ($null -ne $childId) { $sample += $childId }
    $ws = 0L; $privateBytes = 0L; $cpu = 0.0
    foreach ($processId in $sample) {
        $process = Get-Process -Id $processId -ErrorAction SilentlyContinue
        if (-not $process) { continue }
        try {
            $seconds = $process.TotalProcessorTime.TotalSeconds
            if ($previous.ContainsKey($processId)) {
                $delta = $elapsed - $previous[$processId].Elapsed
                if ($delta -gt 0) { $cpu += 100 * ($seconds - $previous[$processId].Cpu) / $delta / $logical }
            }
            $previous[$processId] = @{ Elapsed = $elapsed; Cpu = $seconds }
            $ws += $process.WorkingSet64
            $privateBytes += $process.PrivateMemorySize64
        } catch { }
    }
    $records.Add([pscustomobject]@{Seconds=[math]::Round($elapsed,3);WorkingSetBytes=$ws;PrivateBytes=$privateBytes;CpuPercentMachine=[math]::Round($cpu,3)})
    if ($timer.Elapsed.TotalMinutes -ge 45 -and -not $probeProcess.HasExited) {
        # Stop only the process created by this script. Its Job Object closes the worker.
        $probeProcess.Kill()
        throw 'La prueba excedió 45 minutos. Se conserva el índice publicado anterior.'
    }
    if (-not $probeProcess.HasExited) { Start-Sleep -Milliseconds 500 }
} while (-not $probeProcess.HasExited)
$probeProcess.WaitForExit()
$records | Export-Csv -LiteralPath (Join-Path $outputDirectory ($ReportName + '-samples.csv')) -NoTypeInformation -Encoding utf8
$summary = [pscustomobject]@{
    StartedUtc=$started; DurationSeconds=[math]::Round($timer.Elapsed.TotalSeconds,3); ExitCode=$probeProcess.ExitCode
    ProbeProcessId=$probeProcess.Id; WorkerProcessId=$childId; Samples=$records.Count
    PeakWorkingSetMB=[math]::Round(($records.WorkingSetBytes | Measure-Object -Maximum).Maximum / 1000000,2)
    PeakPrivateBytesMB=[math]::Round(($records.PrivateBytes | Measure-Object -Maximum).Maximum / 1000000,2)
    MeanCpuPercentMachine=[math]::Round(($records.CpuPercentMachine | Measure-Object -Average).Average,3)
    PeakCpuPercentMachine=($records.CpuPercentMachine | Measure-Object -Maximum).Maximum
    Notes='CLI de evaluación y su worker, sin interfaz gráfica. Pico conjunto muestreado aproximadamente cada 500 ms; working set suma páginas compartidas. Private bytes es memoria comprometida. CPU normalizada al total de procesadores lógicos. Equipo en uso.'
}
$summary | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $outputDirectory ($ReportName + '-resources.json')) -Encoding utf8
$summary | ConvertTo-Json
