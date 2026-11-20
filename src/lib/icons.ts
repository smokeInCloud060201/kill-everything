export interface IconPreset {
  id: string;
  label: string;
  path: string;
}

export const ICON_PRESETS: IconPreset[] = [
  {
    id: 'bolt',
    label: 'Bolt',
    path: 'M13 2L4 14h7l-1 8 9-12h-7l1-8z',
  },
  {
    id: 'flame',
    label: 'Flame',
    path: 'M12 2c1.5 3.5 5 5.5 5 10a5 5 0 1 1-10 0c0-2.5 1.5-4.8 3-6.5.2 1.8 1.2 2.8 2 2.8-.3-2.3-.5-4.3 0-6.3z',
  },
  {
    id: 'terminal',
    label: 'Terminal',
    path: 'M4 5h16a1 1 0 0 1 1 1v12a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1zm3 4l4 3-4 3m5 0h5',
  },
  {
    id: 'cpu',
    label: 'Process',
    path: 'M7 7h10v10H7V7zm2-4v2m6-2v2m-6 14v2m6-2v2M3 9h2m-2 6h2m14-6h2m-2 6h2',
  },
  {
    id: 'globe',
    label: 'Browser',
    path: 'M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20zm-8.5 10h17M12 2c2.8 3 4 6.5 4 10s-1.2 7-4 10c-2.8-3-4-6.5-4-10s1.2-7 4-10z',
  },
  {
    id: 'box',
    label: 'Container',
    path: 'M12 2l8 4.5v11L12 22l-8-4.5v-11L12 2zm0 20v-9.5M4 6.5l8 4.5 8-4.5',
  },
  {
    id: 'skull',
    label: 'Force Kill',
    path: 'M12 3a7 7 0 0 0-7 7c0 2.4 1.2 4.5 3 5.7V19h8v-3.3c1.8-1.2 3-3.3 3-5.7a7 7 0 0 0-7-7zm-2.5 8a1 1 0 1 1 0-2 1 1 0 0 1 0 2zm5 0a1 1 0 1 1 0-2 1 1 0 0 1 0 2z',
  },
];

const ICON_MAP = new Map(ICON_PRESETS.map((p) => [p.id, p.path]));

export function getIconPath(id: string): string {
  return ICON_MAP.get(id) ?? ICON_PRESETS[0].path;
}
