$ErrorActionPreference = 'Stop'
$project = 'C:\dictation-api'
$tools = 'C:\dictation-tools'
New-Item -ItemType Directory -Force $project, $tools | Out-Null
Expand-Archive C:\Users\user\windows-test.zip -DestinationPath $project -Force
Expand-Archive C:\Users\user\python-windows.zip -DestinationPath $tools -Force
Expand-Archive C:\Users\user\node-windows.zip -DestinationPath $tools -Force
$env:PATH = "$tools\node-v26.12.1-win-x64;$env:PATH"
$env:PYTHONPATH = "$project\bindings\python\python"
$env:DICTATION_BROKER_EXE = "$project\target\debug\dictation-broker.exe"
Set-Location $project
& .\tests\windows\ownership.exe
if ($LASTEXITCODE) { throw 'Ownership tests failed' }
& .\tests\windows\socket.exe
if ($LASTEXITCODE) { throw 'Socket tests failed' }
& node.exe tests/node-e2e.mjs
if ($LASTEXITCODE) { throw 'Node binding E2E failed' }
& "$tools\python\python.exe" tests/python-c-e2e.py
if ($LASTEXITCODE) { throw 'Python/C binding E2E failed' }
