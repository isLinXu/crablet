import { spawnSync } from 'node:child_process';
import process from 'node:process';

const reportsDirectory = process.env.FRONTEND_COVERAGE_DIR || 'coverage';
const command = process.platform === 'win32' ? 'npx.cmd' : 'npx';

const result = spawnSync(
  command,
  [
    'vitest',
    '--run',
    '--coverage',
    '--configLoader',
    'runner',
    `--coverage.reportsDirectory=${reportsDirectory}`,
  ],
  { stdio: 'inherit' },
);

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);
