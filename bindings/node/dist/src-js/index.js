import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const native = require('../../native.cjs');
const noAudio = { level: false, spectrum: false };
/** Both roles use independent connections. Run one event consumer per connection. */
export class Connection {
    connection;
    closed = false;
    constructor(endpoint) { this.connection = new native.BrokerConnection(endpoint); }
    async request(request) { return JSON.parse(await this.connection.request(JSON.stringify(request))); }
    async nextEvent(timeoutMs = 1000) {
        const json = await this.connection.nextEvent(timeoutMs);
        return json == null ? null : JSON.parse(json);
    }
    async *events() { while (!this.closed) {
        const event = await this.nextEvent();
        if (event) {
            yield event;
            if (event.kind === 'disconnected')
                break;
        }
    } }
    close() { this.closed = true; this.connection.close(); }
}
export class Client extends Connection {
    static async connect(application, endpoint) {
        const client = new Client(endpoint);
        try {
            await client.request({ op: 'register_client', application, processing: true });
            return client;
        }
        catch (error) {
            client.close();
            throw error;
        }
    }
    async activate() {
        const reply = await this.request({ op: 'activate' });
        if (reply.kind !== 'activated')
            throw new Error('Invalid activation response');
        return reply.activation;
    }
    async deactivate(activation) { await this.request({ op: 'deactivate', activation }); }
    async respond(session, accept, audio = noAudio) { await this.request({ op: 'respond', session, accept, audio }); }
    async acknowledge(session, applied, error = null) { await this.request({ op: 'acknowledge', session, applied, error }); }
    async cancel(session) { await this.request({ op: 'cancel', session }); }
}
export class Provider extends Connection {
    static async connect(name, endpoint) {
        const provider = new Provider(endpoint);
        try {
            await provider.request({ op: 'register_provider', name });
            return provider;
        }
        catch (error) {
            provider.close();
            throw error;
        }
    }
    async begin(audio = noAudio) {
        const reply = await this.request({ op: 'begin', audio });
        if (reply.kind !== 'started')
            throw new Error('Invalid begin response');
        return reply.session;
    }
    async audio(session, frame) { await this.request({ op: 'audio', session, frame }); }
    async processing(session) { await this.request({ op: 'processing', session }); }
    async complete(session, text, submit = false) { await this.request({ op: 'complete', session, transcript: { text, submit } }); }
    async cancel(session) { await this.request({ op: 'cancel', session }); }
}
