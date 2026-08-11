import { registerRootComponent } from 'expo';

import { App } from './App';

// Expo SDK 57 entry. The New Architecture (Bridgeless Mode) is enabled via
// `app.json` (newArchEnabled). `registerRootComponent` wires the React tree.
registerRootComponent(App);
