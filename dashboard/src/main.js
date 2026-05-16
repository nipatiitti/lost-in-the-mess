import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import RaptorCanvas from './RaptorCanvas.svelte'

const path = window.location.pathname;

const app = mount(path === '/raptorq' ? RaptorCanvas : App, {
  target: document.getElementById('app'),
})

export default app
