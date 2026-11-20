import { mount } from 'svelte';
import TickApp from './TickApp.svelte';

mount(TickApp, {
  target: document.getElementById('app')!,
});
