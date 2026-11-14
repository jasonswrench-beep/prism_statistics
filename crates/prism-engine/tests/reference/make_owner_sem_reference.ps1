# Reference values for the structural equation models in owner_sem_reference.rs, from the product owner's graphical
# SEM reference package (one of "the other reference packages the product owner has"), through its scripting engine.
# Run from this folder in 64-bit Windows PowerShell:
#   powershell -File make_owner_sem_reference.ps1 -Engine <path to the package's engine library (.dll)>
# It writes owner_sem_gr.json. Each value is keyed "<model>.g<group>:<type>:<left>:<right>.<est|se>" with the
# engine's parameter types (1 path, 2 intercept, 3 mean, 4 covariance, 5 variance); standardized paths are
# "<model>.std:g<group>:<to>:<from>". The test maps the names.
param([Parameter(Mandatory = $true)][string]$Engine, [string]$Only)
$ErrorActionPreference = 'Stop'
$here = $PSScriptRoot
$inv = [Globalization.CultureInfo]::InvariantCulture
$models = 'cfa', 'path', 'pd', 'mgconfigural', 'mgmetric', 'lgm'

# The engine keeps state from one model to the next within a process (a two-group model fails after single-group
# ones), so each model is fitted by a fresh copy of this script and the values are gathered here.
if (-not $Only) {
    $all = [ordered]@{}
    foreach ($m in $models) {
        Write-Host "fitting $m"
        $lines = & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $PSCommandPath -Engine $Engine -Only $m
        if ($LASTEXITCODE -ne 0) { throw "$m failed:`n$($lines -join "`n")" }
        foreach ($line in $lines) { if ($line -match '^(\S+)	(\S+)$') { $all[$Matches[1]] = $Matches[2] } }
    }
    $json = foreach ($k in $all.Keys) { '  "{0}": {1}' -f $k, $all[$k] }
    Set-Content -Encoding ASCII (Join-Path $here 'owner_sem_gr.json') ("{`n" + ($json -join ",`n") + "`n}")
    "wrote $($all.Count) values"
    exit 0
}
# The engine's internal checks print instead of opening a dialog.
foreach ($l in [Diagnostics.Trace]::Listeners) { if ($l -is [Diagnostics.DefaultTraceListener]) { $l.AssertUiEnabled = $false; $l.LogFileName = Join-Path ([IO.Path]::GetTempPath()) 'prism-sem-engine.log' } }
$asm = [Reflection.Assembly]::LoadFrom($Engine)
Set-Location (Split-Path $Engine)
# The engine class is the one that takes model statements.
$type = $asm.GetTypes() | Where-Object { $_.GetMethod('AStructure') } | Select-Object -First 1
$ids = $asm.GetTypes() | Where-Object { $_.Name -eq 'TMatrixID' } | Select-Object -First 1
# The result-matrix class is the one with row and column names and a resizer.
$matrix = $asm.GetTypes() | Where-Object { $_.GetMethod('RowName') -and $_.GetMethod('ColumnName') -and $_.GetMethod('SetDimensions') } | Select-Object -First 1
$out = [ordered]@{}
$tmp = Join-Path ([IO.Path]::GetTempPath()) ('prism-sem-' + [guid]::NewGuid())
New-Item -ItemType Directory $tmp | Out-Null

# One CSV per school, for the two-group models.
# (The original lines are copied, so numbers stay unquoted.)
$hs = Get-Content (Join-Path $here 'holzinger.csv')
foreach ($s in 'Pasteur', 'Grant-White') { Set-Content -Encoding ASCII (Join-Path $tmp "$s.csv") (@($hs[0]) + @($hs | Where-Object { $_ -like "*,`"$s`",*" })) }

function Fit([string]$name, [scriptblock]$spec, [string[]]$groups) {
    if ($name -ne $Only) { return }
    $e = [Activator]::CreateInstance($type)
    try {
        $e.FitMLMoments()   # maximum likelihood moments (divisor N), as lavaan and PrismStatistics
        $e.Standardized()
        $e.NeedEstimates([Enum]::Parse($ids, 'StandardizedDirectEffects'))
        & $spec $e
        $code = $e.FitModel()
        if ($code -ne 0) { throw "$name did not converge (engine code $code)" }
        $out["$name.fit.cmin"] = $e.Cmin()
        $out["$name.fit.df"] = $e.Df()
        $out["$name.fit.npar"] = $e.Npar()
        for ($i = 1; $i -le $e.NumberOfParameters(); $i++) {
            $g = 0; $t = 0; $l = 0; $r = 0
            $e.ParameterInfo($i, [ref]$g, [ref]$t, [ref]$l, [ref]$r)
            $ln = $e.VariableName($l, $g)
            $rn = if ($r -gt 0) { $e.VariableName($r, $g) } else { '' }
            $k = "$name.g${g}:${t}:${ln}:${rn}"
            $out["$k.est"] = $e.ParameterValue($i)
            $out["$k.se"] = [math]::Sqrt($e.ParameterCovariance($i, $i))
        }
        for ($g = 1; $g -le $groups.Count; $g++) {
            $m = [Activator]::CreateInstance($matrix)
            $e.GetEstimatesEx([Enum]::Parse($ids, 'StandardizedDirectEffects'), $m, $g)
            for ($a = 0; $a -lt $m.NRows; $a++) {
                for ($b = 0; $b -lt $m.NColumns; $b++) {
                    $v = $m.X($a, $b)
                    if ($v -ne 0) { $out["$name.std:g${g}:$($m.RowName($a)):$($m.ColumnName($b))"] = $v }
                }
            }
        }
    } finally { $e.Shutdown() }
}

# The three-factor model's statements; $labels names the six free loadings (to hold them equal across groups) and
# $intercept is "() + " when intercepts are modeled.
function Cfa($e, [string[]]$labels = @('', '', '', '', '', ''), [string]$intercept = '') {
    $k = 0
    foreach ($f in @(@('Visual', 1, 2, 3), @('Textual', 4, 5, 6), @('Speed', 7, 8, 9))) {
        foreach ($j in 1..3) {
            $x = $f[$j]
            $w = if ($j -eq 1) { '(1) ' } elseif ($labels[$k]) { "($($labels[$k])) " } else { '' }
            if ($j -gt 1) { $k++ }
            Say $e "x$x = $intercept$w$($f[0]) + (1) e$x"
        }
    }
}
function Say($e, [string]$statement) { if ($env:PRISM_TRACE) { Write-Host "  $statement" }; $e.AStructure($statement) }
$data = Join-Path $here 'holzinger.csv'
Fit 'cfa' { param($e) $e.BeginGroup($data); Cfa $e } @('all')
Fit 'path' { param($e) $e.BeginGroup($data); $e.AStructure('x4 = x1 + x2 + (1) e4'); $e.AStructure('x9 = x4 + x1 + (1) e9'); $e.AStructure('x5 = x2 + (1) e5') } @('all')
Fit 'pd' {
    param($e)
    $e.BeginGroup((Join-Path $here 'poldem.csv'))
    $e.AStructure('x1 = (1) Ind60 + (1) e1'); $e.AStructure('x2 = Ind60 + (1) e2'); $e.AStructure('x3 = Ind60 + (1) e3')
    $e.AStructure('y1 = (1) Dem60 + (1) d1'); $e.AStructure('y2 = (a) Dem60 + (1) d2'); $e.AStructure('y3 = (b) Dem60 + (1) d3'); $e.AStructure('y4 = (c) Dem60 + (1) d4')
    $e.AStructure('y5 = (1) Dem65 + (1) d5'); $e.AStructure('y6 = (a) Dem65 + (1) d6'); $e.AStructure('y7 = (b) Dem65 + (1) d7'); $e.AStructure('y8 = (c) Dem65 + (1) d8')
    $e.AStructure('Dem60 = Ind60 + (1) z60'); $e.AStructure('Dem65 = Ind60 + Dem60 + (1) z65')
    foreach ($c in 'd1 <> d5', 'd2 <> d4', 'd2 <> d6', 'd3 <> d7', 'd4 <> d8', 'd6 <> d8') { $e.AStructure($c) }
} @('all')
$groups = @('Pasteur', 'Grant-White')
Fit 'mgconfigural' {
    param($e)
    $e.ModelMeansAndIntercepts()
    foreach ($s in $groups) { $e.BeginGroup((Join-Path $tmp "$s.csv")); $e.GroupName($s); Cfa $e -intercept '() + ' }
} $groups
Fit 'mgmetric' {
    param($e)
    $e.ModelMeansAndIntercepts()
    foreach ($s in $groups) { $e.BeginGroup((Join-Path $tmp "$s.csv")); $e.GroupName($s); Cfa $e @('v2', 'v3', 't5', 't6', 's8', 's9') '() + ' }
} $groups
Fit 'lgm' {
    param($e)
    $e.ModelMeansAndIntercepts()
    $e.BeginGroup((Join-Path $here 'growth.csv'))
    for ($t = 1; $t -le 4; $t++) { $e.AStructure("t$t = (0) + (1) I + ($($t - 1)) S + (1) e$t") }
    $e.Mean('I'); $e.Mean('S')
} @('all')

Remove-Item -Recurse $tmp
foreach ($k in $out.Keys) { "{0}`t{1}" -f $k, ([double]$out[$k]).ToString('R', $inv) }
