import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import process from 'node:process';

const fallbackTargets = [
  'src/api/chat.ts',
  'src/api/client.ts',
  'src/api/types.ts',
  'src/components/chat/ChatWindow.tsx',
  'src/components/chat/ContextWindowViz.tsx',
  'src/components/chat/MessageBubble.tsx',
  'src/components/chat/SessionList.tsx',
  'src/components/chat/StarsList.tsx',
  'src/components/observability/TraceViewer.tsx',
  'src/components/rag/RagConfigPanel.tsx',
  'src/components/sidebar/SkillBrowser.tsx',
  'src/components/skills/SemanticSearch.tsx',
  'src/components/skills/SkillCreator.tsx',
  'src/components/skills/SkillLogs.tsx',
  'src/components/skills/SkillRunner.tsx',
  'src/components/ui/Button.tsx',
  'src/components/ui/Card.tsx',
  'src/components/ui/EmptyState.tsx',
  'src/components/ui/Input.tsx',
  'src/components/ui/Skeleton.tsx',
  'src/components/ui/cn.ts',
  'src/hooks/useAgentThinking.ts',
  'src/hooks/useApi.ts',
  'src/hooks/useStreamingChat.ts',
  'src/hooks/useWebSocket.ts',
  'src/hooks/__tests__/useApi.test.tsx',
  'src/services/api.ts',
  'src/services/chatService.ts',
  'src/services/dashboardService.ts',
  'src/services/knowledgeService.ts',
  'src/services/settingsService.ts',
  'src/store/chatStore.ts',
  'src/store/canvasStore.ts',
  'src/store/canvasVersionStore.ts',
  'src/store/modelStore.ts',
  'src/store/__tests__/chatStore.test.ts',
  'src/utils/canvasLayout.ts',
  'src/utils/chatToCanvas.ts',
  'src/utils/constants.ts',
  'src/utils/fileContentExtractor.ts',
  'src/utils/__tests__/constants.test.ts',
];

const targetsFileUrl = new URL('../.eslint-ci-targets.txt', import.meta.url);
const targets = existsSync(targetsFileUrl)
  ? readFileSync(targetsFileUrl, 'utf8')
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean)
  : fallbackTargets;

const command = process.platform === 'win32' ? 'npx.cmd' : 'npx';
const result = spawnSync(command, ['eslint', '--max-warnings=0', ...targets], {
  stdio: 'inherit',
});

if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}

process.exit(result.status ?? 1);
