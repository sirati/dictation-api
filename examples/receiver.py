"""CLI receiver. A GUI must capture and retain its actual insertion target at offer time."""
import os
from dictation_api import Client
with Client(dict(id="python.example",name="Python receiver",icon=None,process_id=os.getpid())) as client:
    activation=client.activate()
    while True:
        event=client.next_event()
        if not event:continue
        if event["kind"]=="offer":client.respond(event["session"],True,level=True)
        elif event["kind"]=="audio":print("Voice level:",event["frame"]["level"])
        elif event["kind"]=="transcript":
            print(event["transcript"])
            client.acknowledge(event["session"],True)
            break
    client.deactivate(activation)
