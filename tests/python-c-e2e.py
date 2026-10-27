"""Exercise Python's provider API against a C-ABI receiving application."""
import ctypes as c
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
from dictation_api import Provider
root=Path(__file__).resolve().parent.parent
suffix='dll' if sys.platform=='win32' else 'dylib' if sys.platform=='darwin' else 'so'
name='dictation_api_c.dll' if sys.platform=='win32' else f'libdictation_api_c.{suffix}'
lib=c.CDLL(str(root/'target/debug'/name))
lib.dictation_connect.argtypes=[c.c_char_p,c.POINTER(c.c_void_p)];lib.dictation_connect.restype=c.c_void_p
lib.dictation_request.argtypes=[c.c_void_p,c.c_char_p,c.POINTER(c.c_void_p)];lib.dictation_request.restype=c.c_void_p
lib.dictation_next_event.argtypes=[c.c_void_p,c.c_uint32,c.POINTER(c.c_void_p)];lib.dictation_next_event.restype=c.c_void_p
lib.dictation_string_free.argtypes=[c.c_void_p]
lib.dictation_close.argtypes=[c.c_void_p]
def consume(pointer):
    if not pointer:return None
    try:return c.string_at(pointer).decode()
    finally:lib.dictation_string_free(pointer)
def call(function,*args):
    error=c.c_void_p();value=function(*args,c.byref(error))
    if error.value:raise RuntimeError(consume(error.value))
    return value
def request(connection,**request):return json.loads(consume(call(lib.dictation_request,connection,json.dumps(request).encode())))
def event(connection):return json.loads(consume(call(lib.dictation_next_event,connection,3000)))
with tempfile.TemporaryDirectory(prefix='dictation-python-c-') as home:
    executable=root/'target/debug'/('dictation-broker.exe' if sys.platform=='win32' else 'dictation-broker')
    broker=subprocess.Popen([executable,'run'],env={**os.environ,'DICTATION_API_HOME':home,'DICTATION_API_NO_NATIVE_FOCUS':'1','DICTATION_API_NO_DBUS':'1'})
    connection=None;provider=None
    try:
        endpoint=Path(home)/'endpoint.json'
        for _ in range(250):
            if endpoint.exists():break
            if broker.poll() is not None:raise RuntimeError('broker exited')
            time.sleep(.02)
        if os.environ.get('DICTATION_C_SMOKE'):
            subprocess.run([os.environ['DICTATION_C_SMOKE'],str(endpoint)],check=True)
        connection=call(lib.dictation_connect,str(endpoint).encode())
        request(connection,op='register_client',application=dict(id='c-client',name='C receiving application',icon=None,process_id=os.getpid()))
        request(connection,op='activate')
        provider=Provider('Python provider',str(endpoint));session=provider.begin(level=True)
        assert event(connection)['kind']=='offer'
        request(connection,op='respond',session=session,accept=True,audio=dict(level=True,spectrum=False))
        assert provider.next_event()['application']['name']=='C receiving application'
        provider.audio(session,level=.5);assert event(connection)['frame']['level']==.5
        provider.complete(session,'Python to C: α 🗣',submit=True)
        assert event(connection)['transcript']==dict(text='Python to C: α 🗣',submit=True)
        request(connection,op='acknowledge',session=session,applied=True,error=None)
        assert provider.next_event()['applied'] is True
        print('Python/PyO3 and C/cbindgen socket E2E passed')
    finally:
        if provider:provider.close()
        if connection:lib.dictation_close(connection)
        broker.terminate();broker.wait(timeout=10)
