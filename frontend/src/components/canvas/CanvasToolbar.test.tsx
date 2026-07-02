import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { CanvasToolbar } from './CanvasToolbar';
import { describe, it, expect, vi } from 'vitest';

// Mock ModelSelectorCompact to simplify rendering
vi.mock('./ModelSelectorCompact', () => ({
  ModelSelectorCompact: () => <div data-testid="model-selector" />,
}));

const defaultProps = {
  workflowName: 'Test Workflow',
  setWorkflowName: vi.fn(),
  searchQuery: '',
  setSearchQuery: vi.fn(),
  showNodePanel: false,
  onToggleNodePanel: vi.fn(),
  onShowTemplatePanel: vi.fn(),
  onImportClick: vi.fn(),
  onExport: vi.fn(),
  hasNodes: false,
  aiNodesCount: 0,
  onBatchUpdateModel: vi.fn(),
  layoutMode: 'auto' as const,
  onLayout: vi.fn(),
  onClear: vi.fn(),
  showExecutionPanel: false,
  onToggleExecutionPanel: vi.fn(),
  isSaving: false,
  onSave: vi.fn(),
};

describe('CanvasToolbar', () => {
  it('renders workflow name input', () => {
    render(<CanvasToolbar {...defaultProps} />);
    const nameInput = screen.getByDisplayValue('Test Workflow');
    expect(nameInput).toBeInTheDocument();
  });

  it('renders search input', () => {
    render(<CanvasToolbar {...defaultProps} />);
    const searchInput = screen.getByPlaceholderText(/搜索|Search|search/i);
    expect(searchInput).toBeInTheDocument();
  });

  it('calls onToggleNodePanel when node panel button clicked', async () => {
    const onToggleNodePanel = vi.fn();
    render(<CanvasToolbar {...defaultProps} onToggleNodePanel={onToggleNodePanel} />);
    const nodeBtn = screen.getByRole('button', { name: /节点|Node|node/i });
    await userEvent.click(nodeBtn);
    expect(onToggleNodePanel).toHaveBeenCalled();
  });

  it('calls onExport via export button', async () => {
    const onExport = vi.fn();
    render(<CanvasToolbar {...defaultProps} onExport={onExport} hasNodes={true} />);
    const exportBtn = screen.getByTitle('Export Workflow');
    await userEvent.click(exportBtn);
    expect(onExport).toHaveBeenCalled();
  });

  it('renders toolbar with buttons when saving', () => {
    render(<CanvasToolbar {...defaultProps} isSaving={true} />);
    // When saving, buttons should still be present
    const buttons = screen.getAllByRole('button');
    expect(buttons.length).toBeGreaterThan(0);
  });

  it('renders save button', async () => {
    const onSave = vi.fn();
    render(<CanvasToolbar {...defaultProps} onSave={onSave} />);
    const saveBtn = screen.getByRole('button', { name: /Save|保存/i });
    await userEvent.click(saveBtn);
    expect(onSave).toHaveBeenCalled();
  });

  it('calls onClear when clear button clicked', async () => {
    const onClear = vi.fn();
    render(<CanvasToolbar {...defaultProps} onClear={onClear} hasNodes={true} />);
    const clearBtn = screen.getByRole('button', { name: /Clear|清空|Trash/i });
    await userEvent.click(clearBtn);
    expect(onClear).toHaveBeenCalled();
  });

  it('renders all layout mode buttons', () => {
    render(<CanvasToolbar {...defaultProps} />);
    const buttons = screen.getAllByRole('button');
    expect(buttons.length).toBeGreaterThan(3);
  });
});
