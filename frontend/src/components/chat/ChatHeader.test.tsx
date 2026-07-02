import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ChatHeader } from './ChatHeader';
import { describe, it, expect, vi } from 'vitest';
import { MemoryRouter } from 'react-router-dom';

// Mock react-router-dom navigate
vi.mock('react-router-dom', async () => {
  const actual = await vi.importActual('react-router-dom');
  return { ...actual, useNavigate: () => vi.fn() };
});

// Mock react-hot-toast
vi.mock('react-hot-toast', () => ({
  default: { error: vi.fn(), success: vi.fn() },
}));

// Mock chatToCanvas utilities
vi.mock('@/utils/chatToCanvas', () => ({
  convertChatToCanvas: vi.fn(() => ({ nodes: [], edges: [] })),
  downloadWorkflow: vi.fn(),
  readWorkflowFromFile: vi.fn(),
}));

const defaultProps = {
  isConnected: true,
  currentLayer: 'system1' as const,
  vendor: 'kimi',
  messages: [],
  sessionId: null,
  onNewChat: vi.fn(),
  onShowMobileHistory: vi.fn(),
};

describe('ChatHeader', () => {
  it('renders the Crablet brand title', () => {
    render(
      <MemoryRouter>
        <ChatHeader {...defaultProps} />
      </MemoryRouter>
    );
    expect(screen.getByRole('heading', { name: /Crablet/i })).toBeInTheDocument();
  });

  it('renders header container with buttons', () => {
    const { container } = render(
      <MemoryRouter>
        <ChatHeader {...defaultProps} />
      </MemoryRouter>
    );
    const buttons = container.querySelectorAll('button');
    expect(buttons.length).toBeGreaterThan(0);
  });

  it('calls onNewChat when New Chat button is clicked', async () => {
    const onNewChat = vi.fn();
    render(
      <MemoryRouter>
        <ChatHeader {...defaultProps} onNewChat={onNewChat} />
      </MemoryRouter>
    );
    const newChatBtn = screen.getByRole('button', { name: /New Chat/i });
    await userEvent.click(newChatBtn);
    expect(onNewChat).toHaveBeenCalled();
  });

  it('renders multiple action buttons in header', () => {
    render(
      <MemoryRouter>
        <ChatHeader {...defaultProps} />
      </MemoryRouter>
    );
    const buttons = screen.getAllByRole('button');
    // Header should have at least New Chat + History + upload/convert buttons
    expect(buttons.length).toBeGreaterThanOrEqual(2);
  });

  it('renders without crashing when disconnected', () => {
    render(
      <MemoryRouter>
        <ChatHeader {...defaultProps} isConnected={false} />
      </MemoryRouter>
    );
    expect(screen.getByRole('heading', { name: /Crablet/i })).toBeInTheDocument();
  });

  it('renders without crashing with messages and sessionId', () => {
    render(
      <MemoryRouter>
        <ChatHeader
          {...defaultProps}
          messages={[{ id: '1', role: 'user', content: 'hi' } as any]}
          sessionId="test-session-123"
        />
      </MemoryRouter>
    );
    expect(screen.getByRole('heading', { name: /Crablet/i })).toBeInTheDocument();
  });
});
