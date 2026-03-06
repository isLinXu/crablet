import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import './index.css'
import './i18n/config'
import App from './App.tsx'

console.log('Mounting React App...');

try {
  const root = document.getElementById('root');
  if (!root) throw new Error('Root element not found');
  
  createRoot(root).render(
    <StrictMode>
      <BrowserRouter>
        <App />
      </BrowserRouter>
    </StrictMode>,
  )
  console.log('React App mounted successfully');
} catch (e) {
  console.error('Failed to mount React App:', e);
  document.body.innerHTML = `<div style="color: red; padding: 20px;"><h1>App Crash</h1><pre>${e}</pre></div>`;
}
