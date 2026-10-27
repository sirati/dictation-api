import { Provider } from '../bindings/node/dist/src-js/index.js';
const provider = await Provider.connect('JavaScript speech recognizer');
try {
  const session = await provider.begin({level:true,spectrum:true});
  for await (const event of provider.events()) {
    if(event.kind === 'ready') {
      console.log('Recipient:',event.application?.name ?? 'no application');
      if(!event.application){await provider.cancel(session);break;}
      if(event.audio.level || event.audio.spectrum) {
        await provider.audio(session,{level:event.audio.level ? 0.5 : null,spectrum:event.audio.spectrum ? [0.2,0.8,0.1] : null});
      }
      await provider.complete(session,process.argv[2] ?? 'Hello from JavaScript',process.argv.includes('--submit'));
    } else if(event.kind === 'finished' || event.kind === 'cancelled') {console.log(event);break;}
  }
} finally {provider.close();}
