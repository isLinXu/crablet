import { render, screen, waitFor } from '@testing-library/react';
import { Canvas } from './Canvas';
import { describe, it, expect, vi } from 'vitest';

const workflowApiMocks = vi.hoisted(() => ({
  getNodeTypes: vi.fn(async () => []),
}));

vi.mock('../../services/workflowApi', () => ({
  workflowApi: {
    getNodeTypes: workflowApiMocks.getNodeTypes,
  },
  executionApi: {},
}));

// Mock @xyflow/react
vi.mock('@xyflow/react', () => ({
  ReactFlowProvider: ({ children }: any) => <div>{children}</div>,
  ReactFlow: ({ children }: any) => <div data-testid="react-flow">{children}</div>,
  Controls: () => <div>Controls</div>,
  Background: () => <div>Background</div>,
  Panel: ({ children }: any) => <div>{children}</div>,
  MiniMap: () => <div>MiniMap</div>,
  useNodesState: (initial: any) => [initial, vi.fn(), vi.fn()],
  useEdgesState: (initial: any) => [initial, vi.fn(), vi.fn()],
  useReactFlow: () => ({
    fitView: vi.fn(),
    screenToFlowPosition: vi.fn(() => ({ x: 0, y: 0 })),
  }),
  applyNodeChanges: vi.fn(),
  applyEdgeChanges: vi.fn(),
  Position: { Top: 'top', Bottom: 'bottom' },
  Handle: () => <div />,
}));

// Mock ResizeObserver
(globalThis as any).ResizeObserver = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};

describe('Canvas', () => {
  it('renders canvas container', async () => {
    render(<Canvas />);
    await waitFor(() => expect(workflowApiMocks.getNodeTypes).toHaveBeenCalled());
    expect(screen.getByTestId('react-flow')).toBeInTheDocument();
  });
});
