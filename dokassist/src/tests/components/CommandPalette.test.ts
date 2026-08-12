import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/svelte';
import CommandPalette from '$lib/components/CommandPalette.svelte';

vi.mock('$app/navigation', () => ({
  goto: vi.fn(),
}));

beforeEach(() => {
  vi.clearAllMocks();
});

function renderPalette(props: Record<string, unknown> = {}) {
  return render(CommandPalette, {
    props: { isOpen: true, onClose: vi.fn(), ...props },
  });
}

describe('CommandPalette — dialog semantics', () => {
  it('exposes a focusable modal dialog with an accessible name', () => {
    renderPalette();
    const dialog = screen.getByRole('dialog', { name: /command palette/i });
    expect(dialog).toHaveAttribute('aria-modal', 'true');
    expect(dialog).toHaveAttribute('tabindex', '-1');
  });

  it('renders nothing while closed', () => {
    renderPalette({ isOpen: false });
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('moves focus to the search input when opened', async () => {
    vi.useFakeTimers();
    try {
      renderPalette();
      const input = screen.getByPlaceholderText(/search actions, patients/i);
      await vi.advanceTimersByTimeAsync(100);
      expect(document.activeElement).toBe(input);
    } finally {
      vi.useRealTimers();
    }
  });
});

describe('CommandPalette — keyboard handling', () => {
  it('closes on Escape from the search input', async () => {
    const onClose = vi.fn();
    renderPalette({ onClose });
    await fireEvent.keyDown(screen.getByPlaceholderText(/search actions, patients/i), {
      key: 'Escape',
    });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('closes on Escape when focus is on the dialog itself', async () => {
    const onClose = vi.fn();
    renderPalette({ onClose });
    await fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('advances the selection by one item per ArrowDown', async () => {
    renderPalette();
    const input = screen.getByPlaceholderText(/search actions, patients/i);
    const dashboard = screen.getByRole('button', { name: /go to dashboard/i });
    const patients = screen.getByRole('button', { name: /go to patients/i });

    expect(dashboard).toHaveClass('bg-surface-selected');

    await fireEvent.keyDown(input, { key: 'ArrowDown' });
    await waitFor(() => expect(patients).toHaveClass('bg-surface-selected'));
    expect(dashboard).not.toHaveClass('bg-surface-selected');

    await fireEvent.keyDown(input, { key: 'ArrowUp' });
    await waitFor(() => expect(dashboard).toHaveClass('bg-surface-selected'));
  });

  it('runs the selected item on Enter', async () => {
    const onClose = vi.fn();
    renderPalette({ onClose });
    const input = screen.getByPlaceholderText(/search actions, patients/i);
    await fireEvent.keyDown(input, { key: 'Enter' });
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });
});

describe('CommandPalette — backdrop', () => {
  it('closes when the backdrop is clicked', async () => {
    const onClose = vi.fn();
    const { container } = renderPalette({ onClose });
    const backdrop = container.querySelector('[role="presentation"]')!;
    await fireEvent.click(backdrop);
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('stays open when a click lands inside the dialog', async () => {
    const onClose = vi.fn();
    renderPalette({ onClose });
    await fireEvent.click(screen.getByRole('dialog'));
    expect(onClose).not.toHaveBeenCalled();
  });
});
