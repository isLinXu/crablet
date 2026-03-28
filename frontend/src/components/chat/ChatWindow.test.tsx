import { render, screen } from '@testing-library/react';
import { ChatWindow } from './ChatWindow';
import { describe, it, expect, vi } from 'vitest';
import { MemoryRouter } from 'react-router-dom';

// Mock useWebSocket
vi.mock('../../hooks/useWebSocket', () => ({
  useWebSocket: () => ({
    sendMessage: vi.fn(),
    lastMessage: null,
    readyState: 1,
  }),
}));

// Mock react-virtuoso
vi.mock('react-virtuoso', () => ({
  Virtuoso: ({ data, itemContent }: any) => (
    <div>
      {data.map((item: any, index: number) => (
        <div key={index}>{itemContent(index, item)}</div>
      ))}
    </div>
  ),
}));

describe('ChatWindow', () => {
  it('renders chat header', () => {
    render(
      <MemoryRouter>
        <ChatWindow />
      </MemoryRouter>
    );
    expect(screen.getByRole('heading', { name: /Crablet/i })).toBeInTheDocument();
  });
});
