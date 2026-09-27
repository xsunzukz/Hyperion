import React from 'react';
import ReactDOM from 'react-dom/client';
import { App } from './App';
import './index.css';

// Disable default browser right-click context menu (Back, Refresh, Save as, Print)
window.addEventListener('contextmenu', (e) => {
  e.preventDefault();
});

// Disable browser shortcuts (F5, Ctrl+R reload, Ctrl+P print)
window.addEventListener('keydown', (e) => {
  if (
    e.key === 'F5' ||
    (e.ctrlKey && (e.key === 'r' || e.key === 'R' || e.key === 'p' || e.key === 'P'))
  ) {
    e.preventDefault();
  }
});

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
