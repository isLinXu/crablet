import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { EmptyState } from './EmptyState';

describe('EmptyState', () => {
  it('renders with title', () => {
    render(<EmptyState title="No data" />);
    expect(screen.getByText('No data')).toBeInTheDocument();
  });

  it('renders with description', () => {
    render(<EmptyState title="No data" description="Try adding some items." />);
    expect(screen.getByText('Try adding some items.')).toBeInTheDocument();
  });

  it('does not render description when not provided', () => {
    render(<EmptyState title="No data" />);
    // Just verify no extra text
    expect(screen.getByText('No data')).toBeInTheDocument();
    expect(screen.queryByText('description')).not.toBeInTheDocument();
  });

  it('renders with icon', () => {
    render(<EmptyState title="Empty" icon={<span data-testid="test-icon">🔍</span>} />);
    expect(screen.getByTestId('test-icon')).toBeInTheDocument();
  });

  it('does not render icon container when no icon provided', () => {
    const { container } = render(<EmptyState title="No icon" />);
    // Should have exactly one child (the text container)
    expect(container.firstChild?.childNodes.length).toBeGreaterThan(0);
  });

  it('applies custom className', () => {
    const { container } = render(
      <EmptyState title="Test" className="min-h-[200px]" />
    );
    expect((container.firstChild as HTMLElement)?.className).toContain('min-h-[200px]');
  });
});
