//! Transport-independent focus and session ownership rules.
use dictation_protocol::*;
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

type Id = String;
#[derive(Clone)]
enum Role {
    Client(Application),
    Provider(String),
}
struct Session {
    provider: Id,
    target: Option<Id>,
    audio: AudioFeatures,
    phase: Phase,
    deadline: Instant,
}
#[derive(PartialEq)]
enum Phase {
    Offered,
    Recording,
    Delivering,
}
#[derive(Default)]
pub struct Core {
    peers: HashMap<Id, Role>,
    client_order: Vec<Id>,
    focus: Option<(Id, u32)>,
    epoch: u32,
    observed: Option<(u32, String)>,
    observed_epoch: u32,
    sessions: HashMap<Id, Session>,
    pub events: Vec<(Id, Event)>,
}
fn invalid(message: &str) -> ApiError {
    ApiError::new("invalid_state", message)
}
impl Core {
    pub fn initialize_focus(&mut self, pid: u32, window: String) {
        if self.epoch == 0 {
            self.observe_activation(pid, window);
        }
    }
    pub fn disconnect(&mut self, peer: &str) {
        self.peers.remove(peer);
        self.client_order.retain(|id| id != peer);
        if self.focus.as_ref().is_some_and(|(id, _)| id == peer) {
            self.focus = None;
        }
        let sessions: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.provider == peer || s.target.as_deref() == Some(peer))
            .map(|(id, _)| id.clone())
            .collect();
        for id in sessions {
            self.cancel(&id, "peer disconnected");
        }
    }
    pub fn observe_activation(&mut self, pid: u32, window: String) {
        self.epoch = self.epoch.wrapping_add(1);
        self.observed = Some((pid, window));
        self.focus = None;
        if let Some(id) = self
            .client_order
            .iter()
            .rev()
            .find(|id| matches!(self.peers.get(*id),Some(Role::Client(a)) if a.process_id == pid))
            .cloned()
        {
            self.activate(&id);
        }
        self.observed_epoch = self.epoch;
    }
    pub fn observe_deactivation(&mut self, pid: u32, window: &str) {
        if self
            .observed
            .as_ref()
            .is_some_and(|(p, w)| *p == pid && w == window)
        {
            self.observed = None;
            if self.observed_epoch == self.epoch {
                self.focus = None;
            }
        }
    }
    fn activate(&mut self, id: &str) -> u32 {
        self.client_order.retain(|peer| peer != id);
        self.client_order.push(id.into());
        self.epoch = self.epoch.wrapping_add(1);
        self.focus = Some((id.into(), self.epoch));
        self.epoch
    }
    fn emit(&mut self, id: &str, event: Event) {
        self.events.push((id.into(), event));
    }
    fn cancel(&mut self, id: &str, reason: &str) {
        if let Some(s) = self.sessions.remove(id) {
            let event = Event::Cancelled {
                session: id.into(),
                reason: reason.into(),
            };
            self.emit(&s.provider, event.clone());
            if let Some(target) = s.target {
                self.emit(&target, event);
            }
        }
    }
    pub fn expire(&mut self) {
        let expired: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.deadline <= Instant::now())
            .map(|(id, _)| id.clone())
            .collect();
        for id in expired {
            if self.sessions[&id].phase == Phase::Offered {
                let s = self.sessions.get_mut(&id).unwrap();
                let target = s.target.take();
                s.phase = Phase::Recording;
                s.audio = AudioFeatures::default();
                s.deadline = Instant::now() + Duration::from_secs(300);
                let provider = s.provider.clone();
                if let Some(target) = target {
                    self.emit(
                        &target,
                        Event::Cancelled {
                            session: id.clone(),
                            reason: "offer timed out".into(),
                        },
                    );
                }
                self.emit(
                    &provider,
                    Event::Ready {
                        session: id,
                        application: None,
                        audio: AudioFeatures::default(),
                    },
                );
            } else {
                self.cancel(&id, "session timed out");
            }
        }
    }
    pub fn request(&mut self, peer: &str, request: Request) -> Result<Reply, ApiError> {
        match request {
            Request::RegisterClient { application } => {
                if self.peers.contains_key(peer) {
                    return Err(invalid("already registered"));
                }
                if application.id.is_empty()
                    || application.name.is_empty()
                    || application
                        .icon
                        .as_ref()
                        .is_some_and(|x| x.len() > MAX_ICON_BYTES)
                {
                    return Err(invalid("invalid application metadata"));
                }
                let pid = application.process_id;
                self.peers.insert(peer.into(), Role::Client(application));
                self.client_order.push(peer.into());
                if self.focus.is_none()
                    && self.observed_epoch == self.epoch
                    && self.observed.as_ref().is_some_and(|(p, _)| *p == pid)
                {
                    self.activate(peer);
                    self.observed_epoch = self.epoch;
                }
                Ok(Reply::Registered {
                    connection: peer.into(),
                })
            }
            Request::RegisterProvider { name } => {
                if self.peers.contains_key(peer) || name.is_empty() {
                    return Err(invalid("invalid provider registration"));
                }
                self.peers.insert(peer.into(), Role::Provider(name));
                Ok(Reply::Registered {
                    connection: peer.into(),
                })
            }
            Request::Activate => {
                if !matches!(self.peers.get(peer), Some(Role::Client(_))) {
                    return Err(invalid("client required"));
                }
                Ok(Reply::Activated {
                    activation: self.activate(peer),
                })
            }
            Request::Deactivate { activation } => {
                if self
                    .focus
                    .as_ref()
                    .is_some_and(|(id, epoch)| id == peer && *epoch == activation)
                {
                    self.focus = None;
                }
                Ok(Reply::Ok)
            }
            Request::Begin { audio } => {
                let Some(Role::Provider(name)) = self.peers.get(peer).cloned() else {
                    return Err(invalid("provider required"));
                };
                if self
                    .sessions
                    .values()
                    .filter(|s| s.provider == peer)
                    .count()
                    >= 8
                {
                    return Err(invalid("session limit reached"));
                }
                let id = uuid::Uuid::new_v4().to_string();
                let target = self.focus.as_ref().map(|(id, _)| id.clone());
                let offered = target.is_some();
                self.sessions.insert(
                    id.clone(),
                    Session {
                        provider: peer.into(),
                        target: target.clone(),
                        audio: if offered {
                            audio
                        } else {
                            AudioFeatures::default()
                        },
                        phase: if offered {
                            Phase::Offered
                        } else {
                            Phase::Recording
                        },
                        deadline: Instant::now()
                            + Duration::from_secs(if offered { 5 } else { 300 }),
                    },
                );
                if let Some(target) = target {
                    self.emit(
                        &target,
                        Event::Offer {
                            session: id.clone(),
                            provider: name,
                            audio,
                        },
                    );
                } else {
                    self.emit(
                        peer,
                        Event::Ready {
                            session: id.clone(),
                            application: None,
                            audio: AudioFeatures::default(),
                        },
                    );
                }
                Ok(Reply::Started { session: id })
            }
            Request::Respond {
                session,
                accept,
                audio,
            } => {
                let s = self
                    .sessions
                    .get_mut(&session)
                    .ok_or_else(|| invalid("unknown session"))?;
                if s.target.as_deref() != Some(peer) || s.phase != Phase::Offered {
                    return Err(invalid("not the offered client"));
                }
                let application = if accept {
                    match self.peers.get(peer) {
                        Some(Role::Client(a)) => Some(a.clone()),
                        _ => None,
                    }
                } else {
                    None
                };
                s.audio = if accept {
                    s.audio.intersection(audio)
                } else {
                    AudioFeatures::default()
                };
                if !accept {
                    s.target = None;
                }
                s.phase = Phase::Recording;
                s.deadline = Instant::now() + Duration::from_secs(300);
                let (provider, audio) = (s.provider.clone(), s.audio);
                self.emit(
                    &provider,
                    Event::Ready {
                        session,
                        application,
                        audio,
                    },
                );
                Ok(Reply::Ok)
            }
            Request::Audio { session, frame } => {
                let s = self
                    .sessions
                    .get(&session)
                    .ok_or_else(|| invalid("unknown session"))?;
                if s.provider != peer || s.phase != Phase::Recording {
                    return Err(invalid("not recording provider"));
                }
                let valid = |x: f32| x.is_finite() && (0.0..=1.0).contains(&x);
                if frame.level.is_some_and(|x| !s.audio.level || !valid(x))
                    || frame.spectrum.as_ref().is_some_and(|v| {
                        !s.audio.spectrum || v.len() > 128 || v.iter().any(|x| !valid(*x))
                    })
                {
                    return Err(invalid("invalid or unnegotiated audio frame"));
                }
                if let Some(target) = s.target.clone() {
                    self.emit(&target, Event::Audio { session, frame });
                }
                Ok(Reply::Ok)
            }
            Request::Complete {
                session,
                transcript,
            } => {
                let s = self
                    .sessions
                    .get_mut(&session)
                    .ok_or_else(|| invalid("unknown session"))?;
                if s.provider != peer || s.phase != Phase::Recording {
                    return Err(invalid("not recording provider"));
                }
                if let Some(target) = s.target.clone() {
                    s.phase = Phase::Delivering;
                    s.deadline = Instant::now() + Duration::from_secs(10);
                    self.emit(
                        &target,
                        Event::Transcript {
                            session,
                            transcript,
                        },
                    );
                } else {
                    self.sessions.remove(&session);
                    self.emit(
                        peer,
                        Event::Finished {
                            session,
                            applied: false,
                            error: Some("no receiving application".into()),
                        },
                    );
                }
                Ok(Reply::Ok)
            }
            Request::Acknowledge {
                session,
                applied,
                error,
            } => {
                let s = self
                    .sessions
                    .get(&session)
                    .ok_or_else(|| invalid("unknown session"))?;
                if s.target.as_deref() != Some(peer) || s.phase != Phase::Delivering {
                    return Err(invalid("not delivering client"));
                }
                let provider = s.provider.clone();
                self.sessions.remove(&session);
                self.emit(
                    &provider,
                    Event::Finished {
                        session,
                        applied,
                        error,
                    },
                );
                Ok(Reply::Ok)
            }
            Request::Cancel { session } => {
                let s = self
                    .sessions
                    .get(&session)
                    .ok_or_else(|| invalid("unknown session"))?;
                if s.provider != peer && s.target.as_deref() != Some(peer) {
                    return Err(invalid("not session participant"));
                }
                self.cancel(&session, "cancelled by participant");
                Ok(Reply::Ok)
            }
            Request::Ping => Ok(Reply::Ok),
        }
    }
}
