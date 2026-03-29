/**
 * @vitest-environment jsdom
 */

import React, { act } from 'react';
import { createRoot, Root } from 'react-dom/client';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import { ProjectCard } from './ProjectCard';
import { RemoteProject } from '../api/types';

const project: RemoteProject = {
  id: 'project-1',
  name: 'repo@main',
  repo: 'repo',
  branch: 'main',
  status: 'idle',
  is_busy: false,
  error: null,
  changes: [],
};

let container: HTMLDivElement | null = null;
let root: Root | null = null;

beforeAll(() => {
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
});

function renderCard(isSelected: boolean, onSelect = vi.fn()) {
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);

  act(() => {
    root!.render(
      <ProjectCard
        project={project}
        isSelected={isSelected}
        onSelect={onSelect}
        onGitSync={vi.fn()}
        onDelete={vi.fn()}
        isLoading={false}
        syncAvailable
        activeCommands={[]}
      />,
    );
  });

  const card = container.querySelector('[role="button"]');
  if (!(card instanceof HTMLDivElement)) {
    throw new Error('Project card button was not rendered');
  }

  return { card, onSelect };
}

afterEach(() => {
  if (root) {
    act(() => {
      root!.unmount();
    });
  }

  if (container) {
    container.remove();
  }

  root = null;
  container = null;
});

describe('ProjectCard', () => {
  it('selects an unselected project on click', () => {
    const onSelect = vi.fn();
    const { card } = renderCard(false, onSelect);

    act(() => {
      card.click();
    });

    expect(onSelect).toHaveBeenCalledWith('project-1');
  });

  it('clears selection when the selected project is clicked again', () => {
    const onSelect = vi.fn();
    const { card } = renderCard(true, onSelect);

    act(() => {
      card.click();
    });

    expect(onSelect).toHaveBeenCalledWith(null);
  });

  it('toggles selection with Enter and Space keyboard activation', () => {
    const onSelect = vi.fn();
    const { card } = renderCard(true, onSelect);

    act(() => {
      card.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
      card.dispatchEvent(new KeyboardEvent('keydown', { key: ' ', bubbles: true }));
    });

    expect(onSelect).toHaveBeenNthCalledWith(1, null);
    expect(onSelect).toHaveBeenNthCalledWith(2, null);
  });
});
