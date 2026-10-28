export type Application = {
    id: string;
    name: string;
    /**
     * A data URL or an absolute icon filename. Consumers must not execute SVG markup.
     */
    icon: string | null;
    process_id: number;
};
export type AudioFeatures = {
    level: boolean;
    spectrum: boolean;
};
export type AudioFrame = {
    level: number | null;
    /**
     * Normalized magnitudes, ordered from low to high frequency.
     */
    spectrum: Array<number> | null;
};
export type Transcript = {
    text: string;
    submit: boolean;
};
export type Request = {
    "op": "register_client";
    application: Application;
    processing: boolean;
} | {
    "op": "register_provider";
    name: string;
} | {
    "op": "activate";
} | {
    "op": "deactivate";
    activation: number;
} | {
    "op": "begin";
    audio: AudioFeatures;
} | {
    "op": "respond";
    session: string;
    accept: boolean;
    audio: AudioFeatures;
} | {
    "op": "audio";
    session: string;
    frame: AudioFrame;
} | {
    "op": "processing";
    session: string;
} | {
    "op": "complete";
    session: string;
    transcript: Transcript;
} | {
    "op": "acknowledge";
    session: string;
    applied: boolean;
    error: string | null;
} | {
    "op": "cancel";
    session: string;
} | {
    "op": "ping";
};
export type Reply = {
    "kind": "registered";
    connection: string;
} | {
    "kind": "activated";
    activation: number;
} | {
    "kind": "started";
    session: string;
} | {
    "kind": "ok";
};
export type Event = {
    "kind": "offer";
    session: string;
    provider: string;
    audio: AudioFeatures;
} | {
    "kind": "ready";
    session: string;
    application: Application | null;
    audio: AudioFeatures;
} | {
    "kind": "audio";
    session: string;
    frame: AudioFrame;
} | {
    "kind": "processing";
    session: string;
} | {
    "kind": "transcript";
    session: string;
    transcript: Transcript;
} | {
    "kind": "finished";
    session: string;
    applied: boolean;
    error: string | null;
} | {
    "kind": "cancelled";
    session: string;
    reason: string;
} | {
    "kind": "disconnected";
    reason: string;
};
export type ApiError = {
    code: string;
    message: string;
};
export type Command = {
    id: number;
    request: Request;
};
export type Message = {
    "type": "response";
    id: number;
    reply: Reply | null;
    error: ApiError | null;
} | {
    "type": "event";
    event: Event;
};
