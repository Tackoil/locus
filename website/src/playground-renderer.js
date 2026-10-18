export function createRenderer({ onPhase, onResult, onError }) {
  let worker;
  let request = 0;
  let timer;

  function cancel() {
    clearTimeout(timer);
    worker?.terminate();
    worker = null;
    request += 1;
  }

  function fail(message) {
    cancel();
    onError(message);
  }

  function receive({ data }) {
    if (data.id !== request) return;
    if (data.type === 'rendering') return onPhase('Rendering figure…');
    clearTimeout(timer);
    if (data.type === 'fatal') return fail(`The renderer stopped: ${data.error}. Try rendering again.`);
    onResult(data.result);
  }

  function start(source) {
    try {
      const loading = !worker;
      if (!worker) {
        worker = new Worker(new URL('./playground-worker.js', import.meta.url), { type: 'module' });
        worker.onmessage = receive;
        const current = worker;
        worker.onerror = () => {
          if (worker === current) fail('Could not load or run the renderer. Check your connection and try rendering again.');
        };
      }
      request += 1;
      onPhase(loading ? 'Loading the browser renderer…' : 'Rendering figure…');
      timer = setTimeout(() => fail('Rendering stopped after 30 seconds. Simplify the source or try rendering again.'), 30000);
      worker.postMessage({ id: request, source });
    } catch (error) {
      fail(`Could not start the renderer: ${error.message}`);
    }
  }

  return { start, cancel };
}
