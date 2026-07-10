import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { mount } from 'svelte';
import './lib/i18n';
import App from './App.svelte';
import { installGlobalErrorReporting } from './lib/utils/crash-report';
import StageApp from './StageApp.svelte';

// 在 mount 之前装上，挂载期的错误也要落盘（两个窗口共用本入口）
installGlobalErrorReporting();

const target = document.getElementById('app');
if (!target) throw new Error('Missing #app mount element');
// The OBS stage window mounts its own minimal mirror tree — never Main: a second
// Main would run a second petStore and double-count every reward. Routed by
// window LABEL, not URL hash — the dev server drops the fragment from
// WebviewUrl::App, so a hash check silently mounts Main in the stage window.
const isStage = getCurrentWebviewWindow().label === 'stage';
const app = mount(isStage ? StageApp : App, { target });

export default app;
