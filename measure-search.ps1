param(
    [ValidateRange(2, 3600)][int]$DurationSeconds = 30,
    [ValidateRange(1, 60)][int]$IntervalSeconds = 2,
    [string[]]$ProcessNames = @('SearchIndexer', 'SearchHost', 'SearchProtocolHost', 'SearchFilterHost', 'SearchApp'),
    [string]$OutputDirectory = (Join-Path $PSScriptRoot 'measurements')
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$logicalProcessors = [Environment]::ProcessorCount
$rows = [System.Collections.Generic.List[object]]::new()
$previous = @{}
$timer = [Diagnostics.Stopwatch]::StartNew()
$started = [DateTimeOffset]::Now.ToString('o')
do {
    $elapsed = $timer.Elapsed.TotalSeconds
    $processes = @(Get-Process -Name $ProcessNames -ErrorAction SilentlyContinue)
    foreach ($process in $processes) {
        try {
            $key = '{0}:{1}' -f $process.Id, $process.StartTime.Ticks
            $cpuSeconds = $process.TotalProcessorTime.TotalSeconds
            $cpuPercent = $null
            if ($previous.ContainsKey($key)) {
                $delta = $elapsed - $previous[$key].Elapsed
                if ($delta -gt 0) {
                    $cpuPercent = 100 * ($cpuSeconds - $previous[$key].Cpu) / $delta / $logicalProcessors
                }
            }
            $rows.Add([pscustomobject]@{
                ElapsedSeconds = [math]::Round($elapsed, 3)
                Name = $process.ProcessName
                ProcessId = $process.Id
                WorkingSetMiB = [math]::Round($process.WorkingSet64 / 1MB, 3)
                PrivateBytesMiB = [math]::Round($process.PrivateMemorySize64 / 1MB, 3)
                CpuPercentMachine = if ($null -eq $cpuPercent) { $null } else { [math]::Round($cpuPercent, 4) }
                CpuSecondsLifetime = $cpuSeconds
            })
            $previous[$key] = @{ Elapsed = $elapsed; Cpu = $cpuSeconds }
        } catch {
            Write-Warning ('No se pudo leer PID {0}: {1}' -f $process.Id, $_.Exception.Message)
        }
    }
    if ($timer.Elapsed.TotalSeconds -ge $DurationSeconds) { break }
    Start-Sleep -Milliseconds ([int][math]::Min($IntervalSeconds * 1000, ($DurationSeconds - $timer.Elapsed.TotalSeconds) * 1000))
} while ($true)
$samplingDurationSeconds = $timer.Elapsed.TotalSeconds
$csvPath = Join-Path $OutputDirectory 'process-samples.csv'
$rows | Export-Csv -LiteralPath $csvPath -NoTypeInformation -Encoding utf8
$summary = @($rows | Group-Object Name | ForEach-Object {
    $cpuRows = @($_.Group | Where-Object { $null -ne $_.CpuPercentMachine })
    [pscustomobject]@{
        Name = $_.Name
        Samples = $_.Count
        ProcessIds = @($_.Group.ProcessId | Sort-Object -Unique)
        MeanWorkingSetMiBPerProcess = [math]::Round(($_.Group.WorkingSetMiB | Measure-Object -Average).Average, 2)
        PeakWorkingSetMiBPerProcess = ($_.Group.WorkingSetMiB | Measure-Object -Maximum).Maximum
        MeanPrivateBytesMiBPerProcess = [math]::Round(($_.Group.PrivateBytesMiB | Measure-Object -Average).Average, 2)
        MeanCpuPercentMachinePerProcess = if ($cpuRows.Count) { [math]::Round(($cpuRows.CpuPercentMachine | Measure-Object -Average).Average, 4) } else { $null }
    }
})
$hardware = try {
    $system = Get-CimInstance Win32_ComputerSystem
    $os = Get-CimInstance Win32_OperatingSystem
    [pscustomobject]@{
        Os = $os.Caption
        OsVersion = $os.Version
        RamGiB = [math]::Round($system.TotalPhysicalMemory / 1GB, 2)
        Cpu = @((Get-CimInstance Win32_Processor).Name)
        Gpu = @((Get-CimInstance Win32_VideoController).Name)
    }
} catch { @{ Error = $_.Exception.Message } }
$service = Get-Service -Name WSearch -ErrorAction SilentlyContinue
$result = [pscustomobject]@{
    Started = $started
    DurationSeconds = [math]::Round($timer.Elapsed.TotalSeconds, 2)
    SamplingDurationSeconds = [math]::Round($samplingDurationSeconds, 2)
    LogicalProcessors = $logicalProcessors
    SearchServiceStatus = if ($service) { [string]$service.Status } else { 'Not found' }
    Hardware = $hardware
    Summary = $summary
    Notes = 'Observación sin provocar búsquedas. Working set incluye páginas compartidas; private bytes es memoria comprometida, no RAM residente privada. CPU normalizada a todos los procesadores lógicos. No incluye GPU, caché del sistema ni todos los posibles procesos de búsqueda semántica.'
}
$result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'baseline.json') -Encoding utf8
$result | ConvertTo-Json -Depth 6
