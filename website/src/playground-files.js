const storageKey = 'locus-playground-draft-v1';

export function readDraft() {
  try {
    const draft = JSON.parse(localStorage.getItem(storageKey));
    if (typeof draft?.source === 'string' && typeof draft?.name === 'string') return draft;
  } catch { /* Private browsing and storage restrictions still allow editing. */ }
  return null;
}

export function saveDraft(source, name) {
  try { localStorage.setItem(storageKey, JSON.stringify({ source, name })); }
  catch { /* Saving locally is optional. */ }
}

export function fileName(name, extension) {
  return name.replace(/\.[^.]+$/, '').replace(/[^\p{L}\p{N}_-]/gu, '_') + extension;
}

export function downloadSource(source, name) {
  const url = URL.createObjectURL(new Blob([source], { type: 'text/plain;charset=utf-8' }));
  const link = document.createElement('a');
  link.href = url;
  link.download = fileName(name, '.locus');
  link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
