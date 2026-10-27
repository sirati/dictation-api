"""Real GTK/AT-SPI focus routing in a private, software-rendered Wayland desktop."""
import os
from pathlib import Path
import shutil
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import time

if "--gtk" in sys.argv:
    import gi
    gi.require_version("Gtk", "4.0")
    from gi.repository import Gtk
    app = Gtk.Application(application_id="org.dictationapi.FocusFixture")
    def activate(app):
        window = Gtk.ApplicationWindow(application=app, title="Dictation focus fixture")
        window.set_child(Gtk.Entry(text="original input"))
        window.present()
    app.connect("activate", activate)
    app.run([])
    raise SystemExit

root = Path(__file__).resolve().parent.parent
processes = []
with tempfile.TemporaryDirectory(prefix="dictation-wayland-") as temporary:
    home = Path(temporary)
    runtime = home / "runtime"
    runtime.mkdir(mode=0o700)
    env = {**os.environ, "XDG_RUNTIME_DIR":str(runtime), "DICTATION_API_HOME":str(home/"broker"),
           "WLR_BACKENDS":"headless", "WLR_RENDERER":"pixman", "WLR_LIBINPUT_NO_DEVICES":"1",
           "GTK_A11Y":"atspi", "GDK_BACKEND":"wayland", "SWAYSOCK":str(runtime/"sway.sock")}
    env.pop("DISPLAY",None)
    env["GIO_USE_VFS"]="local"
    env["GTK_USE_PORTAL"]="0"
    # Bypass the distribution wrapper's preference for the host systemd bus broker.
    # The private desktop uses dbus-daemon and must never use the host's runtime socket.
    dbus = shutil.which("dbus-daemon")
    sway = shutil.which("sway")
    wayvnc = shutil.which("wayvnc")
    env["PATH"] = str(Path(dbus).parent)
    def spawn(args, **options):
        process = subprocess.Popen(args, env=env, start_new_session=True, **options)
        processes.append(process)
        return process
    def wait_for(predicate, description):
        deadline=time.monotonic()+15
        while not predicate():
            if time.monotonic()>deadline:raise RuntimeError(f"Timed out waiting for {description}")
            time.sleep(.02)
    try:
        session_config=home/"session.conf"
        session_config.write_text(f'<busconfig><type>session</type><listen>unix:tmpdir={runtime}</listen><auth>EXTERNAL</auth><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
        daemon=spawn([dbus,"--config-file="+str(session_config),"--nofork","--print-address=1"],stdout=subprocess.PIPE,text=True)
        env["DBUS_SESSION_BUS_ADDRESS"]=daemon.stdout.readline().strip()
        atspi=Path(os.environ["DICTATION_TEST_ATSPI"])
        launcher=atspi/"libexec/.at-spi-bus-launcher-wrapped"
        spawn([str(launcher),"--launch-immediately"])
        config=home/"sway.conf"
        config.write_text("output * resolution 1024x768\nxwayland disable\nseat seat0 fallback true\n")
        with (home/"sway.log").open("w+") as log:
            spawn([sway,"--unsupported-gpu","-c",str(config)],stdout=log,stderr=log)
            wait_for(lambda:any(p.is_socket() for p in runtime.glob("wayland-*")),"private Wayland socket")
            env["WAYLAND_DISPLAY"]=next(p.name for p in runtime.glob("wayland-*") if p.is_socket())
            with socket.socket() as reservation:
                reservation.bind(("127.0.0.1",0));port=reservation.getsockname()[1]
            spawn([wayvnc,"127.0.0.1",str(port)],stdout=log,stderr=log)
            viewer=None
            def connect_viewer():
                nonlocal_viewer=None
                try:nonlocal_viewer=socket.create_connection(("127.0.0.1",port),timeout=.2)
                except OSError:return False
                viewers.append(nonlocal_viewer);return True
            viewers=[]
            wait_for(connect_viewer,"virtual keyboard server")
            viewer=viewers[0]
            def receive(count):
                data=b""
                while len(data)<count:data+=viewer.recv(count-len(data))
                return data
            receive(12);viewer.sendall(b"RFB 003.008\n")
            security=receive(receive(1)[0]);assert 1 in security
            viewer.sendall(b"\x01");assert receive(4)==b"\0\0\0\0"
            viewer.sendall(b"\x01");header=receive(24);receive(struct.unpack(">I",header[20:24])[0])
            broker=spawn([str(root/"target/debug/dictation-broker"),"run"])
            wait_for(lambda:(home/"broker/endpoint.json").exists(),"broker endpoint")
            gtk=spawn([sys.executable,__file__,"--gtk"])
            probe=subprocess.run([str(root/"target/debug/examples/focus_probe"),str(gtk.pid)],env=env,timeout=30,check=True)
            viewer.close()
            print("Linux Wayland + GTK + AT-SPI native focus E2E passed")
    finally:
        for process in reversed(processes):
            try:os.killpg(process.pid,signal.SIGTERM)
            except ProcessLookupError:pass
        for process in processes:
            try:process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid,signal.SIGKILL);process.wait()
