import React from 'react';
import ReactDOM from 'react-dom/client';

import { App } from '@/app/App';

import '@/styles/globals.css';

const container = document.getElementById('root');
if (!container) {
  throw new Error('未找到 #root 容器');
}

ReactDOM.createRoot(container).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
