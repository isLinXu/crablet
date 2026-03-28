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
  document.body.replaceChildren();
  const container = document.createElement('div');
  container.style.color = 'red';
  container.style.padding = '20px';

  const title = document.createElement('h1');
  title.textContent = 'App Crash';

  const details = document.createElement('pre');
  details.textContent = e instanceof Error ? `${e.name}: ${e.message}` : String(e);

  container.append(title, details);
  document.body.appendChild(container);
}
