import { fireEvent, render, screen } from '@testing-library/react';
import { ErrorBoundary } from '../ErrorBoundary';

function Broken() { throw new Error('boom'); }

describe('ErrorBoundary', () => {
  it('isolates a route failure and retries without reloading the app', () => {
    const { rerender } = render(<ErrorBoundary variant="route" resetKey="/one"><Broken /></ErrorBoundary>);
    expect(screen.getByRole('alert')).toHaveTextContent('此页面暂时无法显示');
    fireEvent.click(screen.getByRole('button', { name: '重试' }));
    rerender(<ErrorBoundary variant="route" resetKey="/two"><div>recovered</div></ErrorBoundary>);
    expect(screen.getByText('recovered')).toBeInTheDocument();
  });
});
