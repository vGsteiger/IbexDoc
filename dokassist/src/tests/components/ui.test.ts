import { describe, it, expect } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import Badge from '$lib/components/ui/Badge.svelte';
import Button from '$lib/components/ui/Button.svelte';
import Card from '$lib/components/ui/Card.svelte';
import Input from '$lib/components/ui/Input.svelte';
import Spinner from '$lib/components/ui/Spinner.svelte';

/**
 * These guard the contract the rest of the app relies on: primitives must emit
 * design tokens, never raw palette classes, and must not silently drop the
 * accessibility affordances views depend on.
 */
const RAW_PALETTE =
  /\b(?:bg|text|border|ring|divide)-(?:gray|slate|zinc|neutral|blue|red|green|amber|yellow|purple|emerald|orange|indigo|sky)-[0-9]{2,3}\b/;

describe('Button', () => {
  it('renders a button element by default', () => {
    render(Button);
    expect(screen.getByRole('button')).toBeInTheDocument();
  });

  it('renders an anchor when href is given', () => {
    render(Button, { href: '/patients' });
    expect(screen.getByRole('link')).toHaveAttribute('href', '/patients');
  });

  it('applies the accent fill only for the primary variant', () => {
    const { unmount } = render(Button, { variant: 'primary' });
    expect(screen.getByRole('button')).toHaveClass('bg-accent');
    unmount();

    render(Button, { variant: 'secondary' });
    expect(screen.getByRole('button')).not.toHaveClass('bg-accent');
  });

  it('uses the danger token for destructive actions', () => {
    render(Button, { variant: 'danger' });
    expect(screen.getByRole('button')).toHaveClass('bg-danger');
  });

  it('disables itself while loading so the action cannot double-fire', () => {
    render(Button, { loading: true });
    expect(screen.getByRole('button')).toBeDisabled();
  });

  it('blocks pointer events when disabled', () => {
    // jsdom dispatches clicks to disabled elements, so assert the guard itself
    // rather than the event behaviour a real browser provides.
    render(Button, { disabled: true });
    const button = screen.getByRole('button');
    expect(button).toBeDisabled();
    expect(button).toHaveClass('disabled:pointer-events-none');
  });

  it('fires onclick when enabled', async () => {
    let clicks = 0;
    render(Button, { onclick: () => (clicks += 1) });
    await fireEvent.click(screen.getByRole('button'));
    expect(clicks).toBe(1);
  });

  it('keeps both control sizes on the shared 28/32px rhythm', () => {
    const { unmount } = render(Button, { size: 'sm' });
    expect(screen.getByRole('button')).toHaveClass('h-7');
    unmount();

    render(Button, { size: 'md' });
    expect(screen.getByRole('button')).toHaveClass('h-8');
  });

  it('emits no raw palette utilities', () => {
    render(Button, { variant: 'primary' });
    expect(screen.getByRole('button').className).not.toMatch(RAW_PALETTE);
  });
});

describe('Input', () => {
  it('marks itself invalid for assistive tech', () => {
    render(Input, { invalid: true });
    expect(screen.getByRole('textbox')).toHaveAttribute('aria-invalid', 'true');
  });

  it('carries no aria-invalid when valid', () => {
    render(Input);
    expect(screen.getByRole('textbox')).not.toHaveAttribute('aria-invalid');
  });

  it('signals invalidity with the danger border token', () => {
    render(Input, { invalid: true });
    expect(screen.getByRole('textbox')).toHaveClass('border-danger');
  });

  it('emits no raw palette utilities', () => {
    render(Input, { invalid: true });
    expect(screen.getByRole('textbox').className).not.toMatch(RAW_PALETTE);
  });
});

describe('Badge and Card', () => {
  it('maps each badge tone to its own token trio', () => {
    render(Badge, { tone: 'success' });
    const badge = screen.getByText((_, el) => el?.tagName === 'SPAN');
    expect(badge.className).toContain('bg-success-subtle');
    expect(badge.className).toContain('text-success-fg');
    expect(badge.className).not.toMatch(RAW_PALETTE);
  });

  it('separates cards with a hairline rather than a shadow', () => {
    const { container } = render(Card);
    const card = container.querySelector('div');
    expect(card).toHaveClass('border-line');
    expect(card?.className).not.toMatch(/shadow-/);
  });
});

describe('Spinner', () => {
  it('exposes a status role with an accessible name', () => {
    render(Spinner);
    expect(screen.getByRole('status')).toHaveAccessibleName('Loading');
  });

  it('uses the provided label as the accessible name', () => {
    render(Spinner, { label: 'Generating report' });
    expect(screen.getByRole('status')).toHaveTextContent('Generating report');
  });
});
