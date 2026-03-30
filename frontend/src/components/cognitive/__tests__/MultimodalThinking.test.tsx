import { render, screen, fireEvent } from '@testing-library/react';
import { MultimodalThinking } from '../MultimodalThinking';
import { describe, it, expect, vi } from 'vitest';

// Mock DOMPurify via security.ts — verify it's called for markdown blocks
vi.mock('@/utils/security', () => ({
  sanitizeHtml: vi.fn((html: string) => html.replace(/<script[\s\S]*?<\/script>/gi, '')),
}));

import { sanitizeHtml } from '@/utils/security';

const makeBlock = (id: string, type: any, content: string, language?: string) => ({
  id,
  type,
  content,
  ...(language ? { language } : {}),
});

const makeStep = (id: string, blocks: any[], status: any = 'completed') => ({
  id,
  title: `Step ${id}`,
  description: `Description ${id}`,
  blocks,
  timestamp: Date.now(),
  status,
});

describe('MultimodalThinking', () => {
  it('renders empty state when no steps', () => {
    render(<MultimodalThinking steps={[]} />);
    expect(screen.getByText('暂无思考内容')).toBeInTheDocument();
  });

  it('renders step navigator when enabled', () => {
    const steps = [makeStep('s1', [makeBlock('b1', 'text', 'Hello')])];
    render(<MultimodalThinking steps={steps} showStepNavigator />);
    expect(screen.getByText('📋 思考步骤')).toBeInTheDocument();
    expect(screen.getByText('1 步')).toBeInTheDocument();
  });

  it('hides step navigator when disabled', () => {
    const steps = [makeStep('s1', [makeBlock('b1', 'text', 'Hello')])];
    render(<MultimodalThinking steps={steps} showStepNavigator={false} />);
    expect(screen.queryByText('📋 思考步骤')).not.toBeInTheDocument();
  });

  it('renders step titles and descriptions', () => {
    const steps = [
      makeStep('s1', [makeBlock('b1', 'text', 'Content 1')]),
      makeStep('s2', [makeBlock('b2', 'text', 'Content 2')], 'processing'),
    ];
    render(<MultimodalThinking steps={steps} />);
    expect(screen.getByText('Step s1')).toBeInTheDocument();
    expect(screen.getByText('Description s1')).toBeInTheDocument();
    expect(screen.getByText('Step s2')).toBeInTheDocument();
  });

  it('renders text blocks', () => {
    const steps = [makeStep('s1', [makeBlock('b1', 'text', 'Plain text content')])];
    render(<MultimodalThinking steps={steps} />);
    expect(screen.getByText('Plain text content')).toBeInTheDocument();
  });

  it('renders code blocks with language', () => {
    const code = 'def hello():\n    print("world")';
    const steps = [makeStep('s1', [makeBlock('b1', 'code', code, 'python')])];
    render(<MultimodalThinking steps={steps} enableCodeExecution />);
    expect(screen.getByText('python')).toBeInTheDocument();
    expect(screen.getByText('def hello():')).toBeInTheDocument();
  });

  it('renders shell blocks', () => {
    const steps = [makeStep('s1', [makeBlock('b1', 'shell', 'ls -la')])];
    render(<MultimodalThinking steps={steps} />);
    expect(screen.getByText('💻 Terminal')).toBeInTheDocument();
    expect(screen.getByText('ls -la')).toBeInTheDocument();
  });

  it('renders json blocks', () => {
    const steps = [makeStep('s1', [makeBlock('b1', 'json', '{"key": "value"}')])];
    render(<MultimodalThinking steps={steps} />);
    expect(screen.getByText('"key": "value"')).toBeInTheDocument();
  });

  it('renders diff blocks', () => {
    const diff = '+added line\n-removed line\n context line';
    const steps = [makeStep('s1', [makeBlock('b1', 'diff', diff)])];
    render(<MultimodalThinking steps={steps} />);
    expect(screen.getByText('代码变更')).toBeInTheDocument();
    expect(screen.getByText('added line')).toBeInTheDocument();
  });

  it('renders mermaid blocks with preview tab', () => {
    const mermaid = 'graph LR\n    A --> B';
    const steps = [makeStep('s1', [makeBlock('b1', 'mermaid', mermaid)])];
    render(<MultimodalThinking steps={steps} />);
    expect(screen.getByText('📊 流程图')).toBeInTheDocument();
    expect(screen.getByText('预览')).toBeInTheDocument();
    expect(screen.getByText('源码')).toBeInTheDocument();
  });

  it('renders table blocks from JSON', () => {
    const tableData = JSON.stringify([{ name: 'Alice', age: 30 }, { name: 'Bob', age: 25 }]);
    const steps = [makeStep('s1', [makeBlock('b1', 'table', tableData)])];
    render(<MultimodalThinking steps={steps} />);
    expect(screen.getByText('数据表格')).toBeInTheDocument();
    expect(screen.getByText('Alice')).toBeInTheDocument();
    expect(screen.getByText('Bob')).toBeInTheDocument();
  });

  it('renders invalid table as error', () => {
    const steps = [makeStep('s1', [makeBlock('b1', 'table', 'not-valid-json')])];
    render(<MultimodalThinking steps={steps} />);
    expect(screen.getByText('❌ 无效的表格数据')).toBeInTheDocument();
  });

  it('calls sanitizeHtml for markdown blocks', () => {
    const steps = [makeStep('s1', [makeBlock('b1', 'markdown', '# Hello **World**')])];
    render(<MultimodalThinking steps={steps} />);
    expect(sanitizeHtml).toHaveBeenCalled();
  });

  it('calls onStepClick when step is clicked', () => {
    const onStepClick = vi.fn();
    const steps = [makeStep('s1', [makeBlock('b1', 'text', 'test')])];
    render(<MultimodalThinking steps={steps} onStepClick={onStepClick} />);

    fireEvent.click(screen.getByText('Step s1'));
    expect(onStepClick).toHaveBeenCalledWith(
      expect.objectContaining({ id: 's1' })
    );
  });

  it('shows correct status icons', () => {
    const steps = [
      makeStep('s1', [], 'pending'),
      makeStep('s2', [], 'processing'),
      makeStep('s3', [], 'completed'),
      makeStep('s4', [], 'error'),
    ];
    render(<MultimodalThinking steps={steps} showStepNavigator />);
    expect(screen.getByText('⏳')).toBeInTheDocument();
    expect(screen.getByText('⚡')).toBeInTheDocument();
    expect(screen.getAllByText('✅').length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText('❌')).toBeInTheDocument();
  });

  it('shows run button for python code blocks when enabled', () => {
    const steps = [makeStep('s1', [makeBlock('b1', 'code', 'print(1)', 'python')])];
    const onBlockAction = vi.fn();
    render(<MultimodalThinking steps={steps} enableCodeExecution onBlockAction={onBlockAction} />);
    
    const runBtn = screen.getByTitle('运行代码');
    expect(runBtn).toBeInTheDocument();
    fireEvent.click(runBtn);
    expect(onBlockAction).toHaveBeenCalledWith('execute', expect.objectContaining({ id: 'b1' }));
  });

  it('hides run button for non-python code blocks', () => {
    const steps = [makeStep('s1', [makeBlock('b1', 'code', 'const x = 1;', 'javascript')])];
    render(<MultimodalThinking steps={steps} enableCodeExecution />);
    expect(screen.queryByTitle('运行代码')).not.toBeInTheDocument();
  });

  it('hides run button when code execution is disabled', () => {
    const steps = [makeStep('s1', [makeBlock('b1', 'code', 'print(1)', 'python')])];
    render(<MultimodalThinking steps={steps} enableCodeExecution={false} />);
    expect(screen.queryByTitle('运行代码')).not.toBeInTheDocument();
  });

  it('highlights active step', () => {
    const steps = [
      makeStep('s1', [makeBlock('b1', 'text', 'A')]),
      makeStep('s2', [makeBlock('b2', 'text', 'B')]),
    ];
    render(<MultimodalThinking steps={steps} activeStepId="s2" />);
    
    const stepButtons = screen.getAllByRole('button');
    const activeBtn = stepButtons.find(btn => btn.classList.contains('active'));
    expect(activeBtn).toBeTruthy();
  });

  it('shows export button for table blocks', () => {
    const tableData = JSON.stringify([{ a: 1 }]);
    const steps = [makeStep('s1', [makeBlock('b1', 'table', tableData)])];
    const onBlockAction = vi.fn();
    render(<MultimodalThinking steps={steps} onBlockAction={onBlockAction} />);
    
    const exportBtn = screen.getByText('📥 导出 CSV');
    fireEvent.click(exportBtn);
    expect(onBlockAction).toHaveBeenCalledWith('export_csv', expect.objectContaining({ id: 'b1' }));
  });

  it('auto-expands completed steps on mount', () => {
    const steps = [
      makeStep('s1', [makeBlock('b1', 'code', 'line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11')], 'completed'),
    ];
    render(<MultimodalThinking steps={steps} />);
    // Code block should be expanded (shows all lines including "line11")
    expect(screen.getByText('line11')).toBeInTheDocument();
  });
});
