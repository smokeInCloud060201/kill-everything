import { mount } from 'svelte';
import '../../styles/shared.css';
import ManagerApp from './ManagerApp.svelte';

mount(ManagerApp, {
  target: document.getElementById('app')!,
});
