// Debug helper for WebUI terminal helper textarea inspection.
// Load this script in a page that already contains xterm (e.g. browser devtools or via console):
//   const s = document.createElement('script');
//   s.src = '/dashboard/debug-ws.js';
//   document.body.appendChild(s);

(() => {
  const formatValue = (value) => {
    if (typeof value !== 'string') {
      return '(not-found)';
    }
    return JSON.stringify(value);
  };

  const logHelperState = (eventName) => {
    const el = document.querySelector('textarea.xterm-helper-textarea');
    if (!el) {
      console.info(`[xterm-helper-debug] ${eventName}: textarea not found`);
      return;
    }

    console.info(
      `[xterm-helper-debug] ${eventName}:`,
      `length=${el.value.length}`,
      `value=${formatValue(el.value)}`,
    );
  };

  const onInput = () => {
    logHelperState('input');
  };

  const onKeydown = () => {
    logHelperState('keydown');
  };

  document.addEventListener('input', onInput, true);
  document.addEventListener('keydown', onKeydown, true);

  window.__cflxTerminalDebug = {
    report: () => logHelperState('manual'),
    detach: () => {
      document.removeEventListener('input', onInput, true);
      document.removeEventListener('keydown', onKeydown, true);
      delete window.__cflxTerminalDebug;
      console.info('[xterm-helper-debug] detached');
    },
  };

  console.info('[xterm-helper-debug] attached. Run window.__cflxTerminalDebug.report() or call detach() to stop.');
})();
