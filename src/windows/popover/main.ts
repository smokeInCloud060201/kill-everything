import { mount } from 'svelte';
import '../../styles/shared.css';
import PopoverApp from './PopoverApp.svelte';

mount(PopoverApp, {
  target: document.getElementById('app')!,
});
