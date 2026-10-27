"""Two-sided dictation SDK. Blocking operations release Python's GIL."""
import json
from ._native import BrokerConnection

class Connection:
    def __init__(self, endpoint=None):
        self._connection = BrokerConnection(endpoint)
    def request(self, **request):
        return json.loads(self._connection.request(json.dumps(request)))
    def next_event(self, timeout_ms=1000):
        event = self._connection.next_event(timeout_ms)
        return None if event is None else json.loads(event)
    def close(self):
        self._connection.close()
    def __enter__(self):
        return self
    def __exit__(self, *_):
        self.close()

class Client(Connection):
    def __init__(self, application, endpoint=None):
        super().__init__(endpoint)
        self.request(op="register_client", application=application)
    def activate(self):
        return self.request(op="activate")["activation"]
    def deactivate(self, activation):
        self.request(op="deactivate", activation=activation)
    def respond(self, session, accept, *, level=False, spectrum=False):
        self.request(op="respond", session=session, accept=accept, audio=dict(level=level, spectrum=spectrum))
    def acknowledge(self, session, applied, error=None):
        self.request(op="acknowledge", session=session, applied=applied, error=error)
    def cancel(self, session):
        self.request(op="cancel", session=session)

class Provider(Connection):
    def __init__(self, name, endpoint=None):
        super().__init__(endpoint)
        self.request(op="register_provider", name=name)
    def begin(self, *, level=False, spectrum=False):
        return self.request(op="begin", audio=dict(level=level, spectrum=spectrum))["session"]
    def audio(self, session, *, level=None, spectrum=None):
        self.request(op="audio", session=session, frame=dict(level=level, spectrum=spectrum))
    def complete(self, session, text, submit=False):
        self.request(op="complete", session=session, transcript=dict(text=text, submit=submit))
    def cancel(self, session):
        self.request(op="cancel", session=session)
