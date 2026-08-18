const { getDefaultConfig } = require('expo/metro-config');
const { withNativeWind } = require('nativewind/metro');
const path = require('node:path');

// Pin projectRoot explicitly. In this pnpm workspace Expo's monorepo detection
// otherwise resolves projectRoot to the repo root, so `./index` fails to resolve
// (the entry lives in apps/mobile, not the workspace root).
const projectRoot = __dirname;
const config = getDefaultConfig(projectRoot);
config.projectRoot = projectRoot;
config.watchFolders = [path.resolve(projectRoot, '..', '..')];
config.resolver.nodeModulesPaths = [
  path.resolve(projectRoot, 'node_modules'),
  path.resolve(projectRoot, '..', '..', 'node_modules'),
];

module.exports = withNativeWind(config, { input: './global.css' });
