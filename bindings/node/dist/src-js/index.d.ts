import type { Application, AudioFeatures, AudioFrame, Event, Reply, Request } from '../generated/protocol.js';
export type { Application, AudioFeatures, AudioFrame, Event, Reply, Request, Transcript } from '../generated/protocol.js';
/** Both roles use independent connections. Run one event consumer per connection. */
export declare class Connection {
    private connection;
    private closed;
    constructor(endpoint?: string);
    request(request: Request): Promise<Reply>;
    nextEvent(timeoutMs?: number): Promise<Event | null>;
    events(): AsyncGenerator<Event>;
    close(): void;
}
export declare class Client extends Connection {
    static connect(application: Application, endpoint?: string): Promise<Client>;
    activate(): Promise<number>;
    deactivate(activation: number): Promise<void>;
    respond(session: string, accept: boolean, audio?: AudioFeatures): Promise<void>;
    acknowledge(session: string, applied: boolean, error?: string | null): Promise<void>;
    cancel(session: string): Promise<void>;
}
export declare class Provider extends Connection {
    static connect(name: string, endpoint?: string): Promise<Provider>;
    begin(audio?: AudioFeatures): Promise<string>;
    audio(session: string, frame: AudioFrame): Promise<void>;
    complete(session: string, text: string, submit?: boolean): Promise<void>;
    cancel(session: string): Promise<void>;
}
