import { createRequire } from 'node:module';
import type { Application, AudioFeatures, AudioFrame, Event, Reply, Request, Transcript } from '../generated/protocol.js';
export type { Application, AudioFeatures, AudioFrame, Event, Reply, Request, Transcript } from '../generated/protocol.js';
const require = createRequire(import.meta.url);
const native = require('../../native.cjs') as typeof import('../generated/native.js');
const noAudio: AudioFeatures = { level: false, spectrum: false };
/** Both roles use independent connections. Run one event consumer per connection. */
export class Connection {
  private connection: InstanceType<typeof native.BrokerConnection>;
  private closed = false;
  constructor(endpoint?: string) { this.connection = new native.BrokerConnection(endpoint); }
  async request(request: Request): Promise<Reply> { return JSON.parse(await this.connection.request(JSON.stringify(request))); }
  async nextEvent(timeoutMs = 1000): Promise<Event | null> {
    const json = await this.connection.nextEvent(timeoutMs);
    return json == null ? null : JSON.parse(json);
  }
  async *events(): AsyncGenerator<Event> { while (!this.closed) { const event = await this.nextEvent(); if (event) { yield event; if (event.kind === 'disconnected') break; } } }
  close(): void { this.closed = true; this.connection.close(); }
}
export class Client extends Connection {
  static async connect(application: Application, endpoint?: string): Promise<Client> {
    const client = new Client(endpoint);
    try { await client.request({op:'register_client', application}); return client; } catch(error) { client.close(); throw error; }
  }
  async activate(): Promise<number> {
    const reply = await this.request({op:'activate'});
    if(reply.kind !== 'activated') throw new Error('Invalid activation response');
    return reply.activation;
  }
  async deactivate(activation: number): Promise<void> { await this.request({op:'deactivate',activation}); }
  async respond(session: string, accept: boolean, audio = noAudio): Promise<void> { await this.request({op:'respond',session,accept,audio}); }
  async acknowledge(session: string, applied: boolean, error: string|null = null): Promise<void> { await this.request({op:'acknowledge',session,applied,error}); }
  async cancel(session: string): Promise<void> { await this.request({op:'cancel',session}); }
}
export class Provider extends Connection {
  static async connect(name: string, endpoint?: string): Promise<Provider> {
    const provider = new Provider(endpoint);
    try { await provider.request({op:'register_provider',name}); return provider; } catch(error) { provider.close(); throw error; }
  }
  async begin(audio = noAudio): Promise<string> {
    const reply = await this.request({op:'begin',audio});
    if(reply.kind !== 'started') throw new Error('Invalid begin response');
    return reply.session;
  }
  async audio(session: string, frame: AudioFrame): Promise<void> { await this.request({op:'audio',session,frame}); }
  async complete(session: string, text: string, submit = false): Promise<void> { await this.request({op:'complete',session,transcript:{text,submit}}); }
  async cancel(session: string): Promise<void> { await this.request({op:'cancel',session}); }
}
