import './styles/tokens.css';
import './styles/app.css';

/** The static fallback in index.html stays until the app has mounted; if that fails it says why and offers a reload. */
function showBootFailure(error: unknown) {
  console.error('Launchpad Drummer failed to start:', error);
  const boot = document.getElementById('boot');
  if (!boot) return;
  boot.classList.add('failed');
  boot.setAttribute('role', 'alert');
  const text = document.getElementById('boot-text');
  if (text) text.textContent = `Launchpad Drummer could not start — ${error instanceof Error ? error.message : String(error)}`;
  if (!boot.querySelector('button')) {
    const reload = document.createElement('button');
    reload.textContent = 'Reload';
    reload.addEventListener('click', () => location.reload());
    boot.appendChild(reload);
  }
}

async function start() {
  try {
    // Browser preview only: lets the failure screen be checked without breaking the build.
    if (!window.__TAURI__ && new URLSearchParams(location.search).get('startup') === 'import-fail') throw new Error('Simulated import failure (preview)');
    const [{ mount }, { default: App }] = await Promise.all([import('svelte'), import('./App.svelte')]);
    mount(App, { target: document.getElementById('app')! });
    performance.mark('lpd:mounted'); // mount is not paint; paint is measured in a trace
    document.getElementById('boot')?.remove();
  } catch (error) {
    showBootFailure(error);
  }
}

void start();
