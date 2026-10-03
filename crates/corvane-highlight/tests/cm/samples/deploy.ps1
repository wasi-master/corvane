#Requires -Version 5.1
<#
.SYNOPSIS
    Deploys the Corvane build to a set of servers.
.DESCRIPTION
    Multi-line block comment with # hashes and > arrows that
    only ends at the closing marker — ünïcödé ✓ #>
[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string[]] $ComputerName,
    [ValidateSet('Debug', 'Release')]
    [string] $Configuration = 'Release',
    [int] $Retries = 3,
    [switch] $Force,
    [double] $Timeout = 2.5e3
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ProgressPreference = "SilentlyContinue"

function Write-Log {
    param([string] $Message, [string] $Level = 'Info')
    $stamp = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
    Write-Host "[$stamp] [$Level] $Message" -ForegroundColor Cyan
}

function Get-BuildInfo([string] $Path) {
    if (-not (Test-Path -Path $Path)) {
        throw "Build not found at '$Path'"
    }
    $item = Get-Item $Path
    return [pscustomobject]@{
        Name     = $item.Name
        Size     = $item.Length / 1MB
        Modified = $item.LastWriteTime
        Hash     = (Get-FileHash $Path -Algorithm SHA256).Hash
    }
}

$numbers = 1, 2, 3, 0x1F, 1.5, .25, 7., 10kb, 4GB, 12L, 3d, 1e10, 6.02e+23
$range = 1..10
$hash = @{ Name = 'corvane'; Version = [version]'1.2.3'; Tags = @('a', 'b') }
$array = @(
    'first'
    'second'
)
$empty = @()
$sum = 0
foreach ($n in $range) { $sum += $n }
$sum -= 1; $sum *= 2; $sum /= 3; $sum %= 5
$i++; $i--
$result = $sum -gt 10 -and $sum -le 100 -or -not $Force
$eq = 'abc' -eq 'ABC' -and 'abc' -ceq 'abc' -and 'x' -ine 'y'
$like = 'corvane.exe' -like '*.exe' -and 'foo' -notlike 'bar*'
$match = 'v1.2.3' -match '^v(\d+)\.(\d+)' -and 'x' -cnotmatch 'y'
$contains = @(1, 2) -contains 2 -and @(1) -notcontains 3
$replaced = 'hello world' -replace 'world', 'there' -creplace 'H', 'J'
$parts = 'a,b,c' -split ',' ; $joined = $parts -join ';'
$type = $sum -is [int] -and $sum -isnot [string]; $cast = '5' -as [int]
$bits = 5 -band 3 -bor 8 -bxor 1; $neg = -bnot 0
$fmt = '{0:N2} of {1}' -f 3.14159, 'pi'
$gt = 5 -ge 4 -and 3 -lt 4 -and 2 -ieq 2
$redirect = Get-Process > processes.txt
Get-ChildItem 2>&1 | Out-Null

$single = 'It''s a single-quoted string with $novar'
$double = "Double with `"escaped`" quotes and ""doubled"" and `t tab"
$interp = "Hello $env:USERNAME, you have $($items.Count) items and ${weird name} too"
$nested = "Total: $(($sum + 1) * (2 - 1)) done"
$sub = "Call $(Get-Date -Format 'HH:mm') at $PSScriptRoot and $_ and $$"
$dollar = "cost: $ 5 and trailing $"
$unterminated = "this string is not closed
$next = 'neither is this one
$after = 'ok'

$here = @"
Here-string with $Configuration and $($ComputerName -join ', ')
  `$escaped and "quotes" inside
Multiple lines of text ✓
"@

$literal = @'
Literal here-string: $not $(expanded) at all
'@

$args = @{
    Path        = 'C:\builds\corvane'
    Destination = "\\server\share"
    Recurse     = $true
    Force       = $false
}
Copy-Item @args
Invoke-Command -ComputerName $ComputerName -ScriptBlock { param($p) Get-Service -Name $p } -ArgumentList 'w3svc'

foreach ($computer in $ComputerName) {
    for ($attempt = 1; $attempt -le $Retries; $attempt++) {
        try {
            if ($PSCmdlet.ShouldProcess($computer, 'Deploy')) {
                $session = New-PSSession -ComputerName $computer
                Copy-Item -Path .\out\* -Destination 'C:\Program Files\Corvane' -ToSession $session -Recurse
                Remove-PSSession $session
            }
            break
        }
        catch [System.Net.WebException] {
            Write-Warning "Attempt $attempt failed: $($_.Exception.Message)"
            Start-Sleep -Seconds ([math]::Pow(2, $attempt))
        }
        catch {
            Write-Error $_
        }
        finally {
            Write-Verbose 'attempt finished'
        }
    }
}

switch -Regex ($Configuration) {
    '^Deb' { $flags = '-g' }
    'Rel.*' { $flags = '-O2'; break }
    default { $flags = '' }
}

$procs = Get-Process | Where-Object { $_.CPU -gt 100 } | Sort-Object CPU -Descending | Select-Object -First 5
$procs | ForEach-Object { "{0,-20} {1,10:N1}" -f $_.Name, $_.CPU } | Out-File -FilePath report.txt
ls | ? { $_.Length -gt 1kb } | % { $_.FullName }
$json = Get-Content config.json -Raw | ConvertFrom-Json
$json | ConvertTo-Json -Depth 5 | Set-Content config.out.json
$env:PATH += ";C:\tools"
[Environment]::SetEnvironmentVariable('CORVANE_HOME', $PSHOME, 'User')
$script:counter = 0; $global:total = 10; $using:remote
$null = New-Item -ItemType Directory -Path "$HOME\.corvane" -Force
while ($true) { if ($LastExitCode -ne 0) { exit $LastExitCode } else { break } }
do { $x++ } until ($x -ge 3)
trap { Write-Host "trapped: $_"; continue }
class Server {
    [string] $Name
    hidden [int] $Port = 8080
    Server([string] $name) { $this.Name = $name }
    [string] ToString() { return "$($this.Name):$($this.Port)" }
}
enum Level { Low = 1; High = 2 }
$s = [Server]::new('alpha')
Write-Output $s.ToString()
$bad = @x @"not at eol
$lone = @
$tab	=	"tabs	here"
$emoji = 'héllo 🚀 日本語'
<# single-line block comment #> $afterComment = 1
<# unterminated block comment at end of file
still comment
