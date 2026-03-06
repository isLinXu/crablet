import { render, screen } from '@testing-library/react';
import { Canvas } from './Canvas';
import { describe, it, expect, vi } from 'vitest';

// Mock @xyflow/react
vi.mock('@xyflow/react', () => ({
  ReactFlow: ({ children }: any) => <div data-testid="react-flow">{children}</div>,
  Controls: () => <div>Controls</div>,
  Background: () => <div>Background</div>,
  Panel: ({ children }: any) => <div>{children}</div>,
  MiniMap: () => <div>MiniMap</div>,
  useNodesState: (initial: any) => [initial, vi.fn(), vi.fn()],
  useEdgesState: (initial: any) => [initial, vi.fn(), vi.fn()],
  applyNodeChanges: vi.fn(),
  applyEdgeChanges: vi.fn(),
  Position: { Top: 'top', Bottom: 'bottom' },
  Handle: () => <div />,
}));

// Mock ResizeObserver
global.ResizeObserver = class ResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
};

describe('Canvas', () => {
  it('renders canvas container', () => {
    render(<Canvas />);
    expect(screen.getByTestId('react-flow')).toBeInTheDocument();
  });
});