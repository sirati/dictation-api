"""Package cross-built artifacts for the throwaway Windows QA machine."""
from pathlib import Path
import zipfile
root=Path(__file__).resolve().parent.parent
output=root/'target/qa-tools/windows-test.zip'
with zipfile.ZipFile(output,'w',zipfile.ZIP_DEFLATED) as archive:
    for directory in ['tests','bindings/node/dist','bindings/node/generated','bindings/python/python']:
        for path in (root/directory).rglob('*'):
            if path.is_file() and '__pycache__' not in path.parts:
                archive.write(path,path.relative_to(root))
    for name in ['package.json','native.cjs']:
        archive.write(root/'bindings/node'/name,Path('bindings/node')/name)
    target=root/'target/x86_64-pc-windows-msvc'
    archive.write(target/'debug/dictation-broker.exe','target/debug/dictation-broker.exe')
    archive.write(target/'debug/examples/focus_probe.exe','target/debug/examples/focus_probe.exe')
    archive.write(target/'release/dictation_api_c.dll','target/debug/dictation_api_c.dll')
    archive.write(target/'release/dictation_api_node.dll','bindings/node/dictation-api.win32-x64-msvc.node')
    archive.write(target/'release/dictation_api_python.dll','bindings/python/python/dictation_api/_native.pyd')
    for test in ['ownership','socket']:
        executable=max((target/'debug/deps').glob(test+'-*.exe'),key=lambda p:p.stat().st_mtime_ns)
        archive.write(executable,Path('tests/windows')/(test+'.exe'))
print(output)
