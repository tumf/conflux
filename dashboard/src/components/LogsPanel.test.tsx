/**
 * @vitest-environment jsdom
 */

import React, { act } from 'react';
import { createRoot, Root } from 'react-dom/client';
import { afterEach, beforeAll, describe, expect, it } from 'vitest';
import { LogsPanel } from './LogsPanel';
import { RemoteLogEntry } from '../api/types';

const logs: RemoteLogEntry[] = [
  {
    message: 'First project log',
    level: 'info',
    change_id: null,
    timestamp: '2026-03-29T00:00:00.000Z',
    project_id: 'project-a',
    operation: null,
    iteration: null,
  },
  {
    message: 'Second project log',
    level: 'warn',
    change_id: null,
    timestamp: '2026-03-29T00:00:01.000Z',
    project_id: 'project-b',
    operation: null,
    iteration: null,
  },
];

let container: HTMLDivElement | null = null;
let root: Root | null = null;

beforeAll(() => {
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
});

function renderLogs(selectedProjectId: string | null, renderedLogs: RemoteLogEntry[]) {
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);

  act(() => {
    root!.render(<LogsPanel logs={renderedLogs} selectedProjectId={selectedProjectId} />);
  });

  return container;
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

describe('LogsPanel', () => {
  it('shows orchestration logs with project labels when no project is selected', () => {
    const rendered = renderLogs(null, logs);

    expect(rendered.textContent).toContain('First project log');
    expect(rendered.textContent).toContain('Second project log');
    expect(rendered.textContent).toContain('project-a');
    expect(rendered.textContent).toContain('project-b');
    expect(rendered.querySelector('[aria-label="Orchestration logs"]')).not.toBeNull();
  });

  it('shows project-scoped logs without project labels when a project is selected', () => {
    const rendered = renderLogs('project-a', [logs[0]]);

    expect(rendered.textContent).toContain('First project log');
    expect(rendered.textContent).not.toContain('Second project log');
    expect(rendered.textContent).not.toContain('project-a');
    expect(rendered.querySelector('[aria-label="Project logs"]')).not.toBeNull();
  });

  it('shows empty state when the selected scope has no logs', () => {
    const rendered = renderLogs('project-a', []);

    expect(rendered.textContent).toContain('No logs yet');
  });
});
