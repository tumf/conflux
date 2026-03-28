import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.jsx'

try {
  const root = document.getElementById('root');
  if (!root) {
    console.error('Root element not found');
    document.body.innerHTML = '<h1>Error: Root element not found</h1>';
  } else {
    console.log('Creating React root');
    createRoot(root).render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
    console.log('React app rendered');
  }
} catch (err) {
  console.error('Failed to render app:', err);
  document.body.innerHTML = `<h1>Error: ${err.message}</h1><pre>${err.stack}</pre>`;
}
