import { mount } from 'svelte';
import './lib/i18n';
import App from './App.svelte';

const target = document.getElementById('app');
if (!target) throw new Error('Missing #app mount element');
const app = mount(App, { target });

export default app;
