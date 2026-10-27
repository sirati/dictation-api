$ErrorActionPreference = 'Stop'
Start-Transcript -Path C:\dictation-api\native-task.log -Force
$project = 'C:\dictation-api'
Remove-Item "$project\native-result.txt" -ErrorAction SilentlyContinue
$broker = "$project\target\debug\dictation-broker.exe"
& $broker install
if ($LASTEXITCODE) { throw 'User task installation failed' }
try {
    Start-Sleep -Seconds 2
    & $broker status
    if ($LASTEXITCODE) { throw 'User task did not start broker' }
    Add-Type -AssemblyName System.Windows.Forms
    $form = New-Object System.Windows.Forms.Form
    $form.Text = 'Dictation native focus fixture'
    $entry = New-Object System.Windows.Forms.TextBox
    $entry.Text = 'Original target'
    $form.Controls.Add($entry)
    $form.Show()
    $form.Activate()
    $entry.Focus() | Out-Null
    [System.Windows.Forms.Application]::DoEvents()
    $probe = Start-Process "$project\target\debug\examples\focus_probe.exe" -ArgumentList $PID -WindowStyle Hidden -PassThru -RedirectStandardOutput "$project\native-focus.out" -RedirectStandardError "$project\native-focus.err"
    while (-not $probe.HasExited) {
        [System.Windows.Forms.Application]::DoEvents()
        Start-Sleep -Milliseconds 20
    }
    $probe.WaitForExit()
    $output = Get-Content "$project\native-focus.out" -Raw
    $output
    Get-Content "$project\native-focus.err"
    if (($null -ne $probe.ExitCode -and $probe.ExitCode -ne 0) -or $output -notmatch 'native-focus-confirmed') { throw 'Native Windows foreground routing failed' }
    $form.Close()
} finally {
    & $broker uninstall
    if ($LASTEXITCODE) { throw 'Task removal failed' }
    $check = Start-Process $broker -ArgumentList status -WindowStyle Hidden -Wait -PassThru -RedirectStandardOutput "$project\stopped.out" -RedirectStandardError "$project\stopped.err"
    if ($check.ExitCode -eq 0) { throw 'Broker survived user task removal' }
}
'Windows interactive user task and native foreground focus E2E passed' | Set-Content "$project\native-result.txt"
