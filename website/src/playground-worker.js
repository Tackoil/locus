import init, { render } from './wasm/locus.js';

const ready = init();

self.onmessage = async ({ data: { id, source } }) => {
  try {
    await ready;
    self.postMessage({ id, type: 'rendering' });
    const result = JSON.parse(render(source));
    self.postMessage({ id, type: 'result', result });
  } catch (error) {
    // A trapped WASM instance must be discarded before another render.
    self.postMessage({ id, type: 'fatal', error: error.message });
  }
};
