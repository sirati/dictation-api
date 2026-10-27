"""Validate the installer and a private systemd user-service instance."""
import os
from pathlib import Path
import subprocess
import tempfile
import time
import uuid

root=Path(__file__).resolve().parent.parent
executable=root/"target/debug/dictation-broker"
unit="dictation-api-e2e-"+uuid.uuid4().hex
with tempfile.TemporaryDirectory(prefix="dictation-service-") as temporary:
    home=Path(temporary)
    env={**os.environ,"XDG_CONFIG_HOME":str(home/"config"),"DICTATION_API_HOME":str(home/"broker"),"DICTATION_API_NO_NATIVE_FOCUS":"1","DICTATION_API_NO_DBUS":"1"}
    subprocess.run([executable,"install","--no-start"],env=env,check=True)
    subprocess.run(["systemd-analyze","--user","verify",str(home/"config/systemd/user/dictation-broker.service")],env=env,check=True)
    started=False
    try:
        subprocess.run(["systemd-run","--user","--collect","--unit="+unit,"--setenv=DICTATION_API_HOME="+str(home/"broker"),"--setenv=DICTATION_API_NO_NATIVE_FOCUS=1","--setenv=DICTATION_API_NO_DBUS=1",str(executable),"run"],check=True)
        started=True
        deadline=time.monotonic()+5
        while not (home/"broker/endpoint.json").exists():
            if time.monotonic()>deadline:raise RuntimeError("User service did not publish endpoint")
            time.sleep(.02)
        subprocess.run([executable,"status"],env=env,check=True)
        state=subprocess.check_output(["systemctl","--user","show",unit,"--property=ActiveState","--value"],text=True).strip()
        assert state=="active",state
        print("Linux systemd user-service E2E passed")
    finally:
        if started:subprocess.run(["systemctl","--user","stop",unit],check=True)
