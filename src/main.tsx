import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'
import ReferenceWindow from './ReferenceWindow.tsx'

const isReferenceWindow = new URLSearchParams(window.location.search).get("window") === "reference";

createRoot(document.getElementById('root')!).render(isReferenceWindow ? <ReferenceWindow /> : <App />)
