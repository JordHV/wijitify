# From Windows: copy provision script + public key into the guest via vmrun, then run it.
# Requires guest username/password. Example:
#   .\scripts\vm-provision.ps1 -GuestUser surfacetest -GuestPassword 'YOUR_PASSWORD'

param(
    [string]$GuestUser = "surfacetest",
    [Parameter(Mandatory = $true)]
    [string]$GuestPassword,
    [string]$Vmx = "C:\Users\jordh\Documents\Virtual Machines\Ubuntu 64-bit 26.04.1 Surfacetesting\Ubuntu 64-bit 26.04.1 Surfacetesting.vmx"
)

$ErrorActionPreference = "Stop"
$vmrun = "C:\Program Files (x86)\VMware\VMware Workstation\vmrun.exe"
$root = Split-Path -Parent $PSScriptRoot
$pub = Join-Path $env:USERPROFILE ".ssh\id_ed25519.pub"
$script = Join-Path $root "scripts\vm-provision.sh"

if (-not (Test-Path $pub)) { throw "Missing $pub — run ssh-keygen -t ed25519" }
if (-not (Test-Path $script)) { throw "Missing $script" }

& $vmrun -gu $GuestUser -gp $GuestPassword copyFileFromHostToGuest $Vmx $pub "/tmp/wijitify_id_ed25519.pub"
& $vmrun -gu $GuestUser -gp $GuestPassword copyFileFromHostToGuest $Vmx $script "/tmp/vm-provision.sh"
& $vmrun -gu $GuestUser -gp $GuestPassword runProgramInGuest $Vmx /bin/bash "/tmp/vm-provision.sh"

Write-Host "If SSH works: ssh wijitify-vm"
Write-Host "Then take snapshot: vmrun snapshot `"$Vmx`" clean"
