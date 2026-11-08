# PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
# Check the Windows installer on a disposable machine (the Packages workflow's runner):
# a silent install, the service as registered, the data folders' access list, a service
# start, a health check, a stop, and a silent uninstall that removes the program and
# the service and keeps the settings. Needs an elevated shell. Not for a machine in use:
# it installs and removes Kshana Pro.
#
#   scripts/test-msi.ps1 -Msi dist\kshana-pro-<version>-x64.msi
param([Parameter(Mandatory = $true)][string]$Msi)
$ErrorActionPreference = 'Stop'
$msi = (Resolve-Path $Msi).Path
$data = Join-Path $env:ProgramData 'kshana-pro'
$exe = Join-Path $env:ProgramFiles 'Kshana Pro\kshana-pro.exe'
function Check($ok, $what) { if (-not $ok) { throw "FAILED: $what" } ; Write-Output "ok: $what" }
function Msiexec($verb, $log) {
  $p = Start-Process msiexec.exe -Wait -PassThru -ArgumentList "$verb `"$msi`" /qn /norestart /l*v `"$log`""
  return $p.ExitCode
}
try {
  Check ((Msiexec '/i' 'msi-install.log') -eq 0) 'silent install'
  Check (Test-Path $exe) 'kshana-pro.exe in Program Files'
  Check (Test-Path (Join-Path $data 'serve.conf')) 'serve.conf in ProgramData'
  $svc = Get-CimInstance Win32_Service -Filter "Name='kshana-pro'"
  Check ($null -ne $svc) 'the kshana-pro service is registered'
  Check ($svc.StartMode -eq 'Auto') "automatic start ($($svc.StartMode))"
  Check ($svc.StartName -match 'LocalService') "LocalService account ($($svc.StartName))"
  Check ($svc.PathName -match '--windows-service' -and $svc.PathName -match 'serve.conf') "service command: $($svc.PathName)"
  Check ($svc.State -eq 'Stopped') 'not started by the installer'
  foreach ($d in @($data, (Join-Path $data 'data'), (Join-Path $data 'licence'))) {
    $sddl = (Get-Acl $d).Sddl
    Check ($sddl -match ';;;LS\)' -and $sddl -notmatch ';;;(BU|WD|AU)\)') "access list of $d is SYSTEM, Administrators and LocalService only ($sddl)"
  }
  Start-Service kshana-pro
  (Get-Service kshana-pro).WaitForStatus('Running', [TimeSpan]::FromSeconds(60))
  Check ((Get-Service kshana-pro).Status -eq 'Running') 'service started'
  $code = 0
  for ($i = 0; $i -lt 60 -and $code -ne 200; $i++) {
    try { $code = (Invoke-WebRequest -UseBasicParsing -TimeoutSec 5 'http://127.0.0.1:8088/health').StatusCode } catch { Start-Sleep -Seconds 1 }
  }
  Check ($code -eq 200) 'GET /health answers 200'
  Check (Test-Path (Join-Path $data 'data\jobs')) 'the job store is in ProgramData\kshana-pro\data (written as LocalService)'
  Stop-Service kshana-pro
  (Get-Service kshana-pro).WaitForStatus('Stopped', [TimeSpan]::FromSeconds(120))
  Check ((Get-Service kshana-pro).Status -eq 'Stopped') 'service stopped'
  Add-Content -Path (Join-Path $data 'serve.conf') -Value '# kept'
  Check ((Msiexec '/x' 'msi-uninstall.log') -eq 0) 'silent uninstall'
  Check (-not (Test-Path $exe)) 'the program is removed'
  Check ($null -eq (Get-Service kshana-pro -ErrorAction SilentlyContinue)) 'the service is removed'
  Check ((Get-Content (Join-Path $data 'serve.conf') -Raw) -match '# kept') 'the settings are kept on uninstall'
  Write-Output 'MSI check passed'
} catch {
  Write-Output $_
  Get-CimInstance Win32_Service -Filter "Name='kshana-pro'" | Format-List * | Out-String | Write-Output
  if (Test-Path (Join-Path $data 'service.log')) { Get-Content (Join-Path $data 'service.log') | Write-Output }
  Get-WinEvent -LogName System -MaxEvents 20 -ErrorAction SilentlyContinue | Where-Object { $_.ProviderName -eq 'Service Control Manager' } | Format-List TimeCreated, Message | Out-String | Write-Output
  exit 1
}
