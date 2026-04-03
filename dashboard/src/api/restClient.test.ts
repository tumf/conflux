// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';

import { getProposalSessionWsUrl, stopAndDequeueChange } from './restClient';

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('getProposalSessionWsUrl', () => {
  it('returns ws:// URL targeting /api/v1/proposal-sessions/{sessionId}/ws', () => {
    const url = getProposalSessionWsUrl('proj-1', 'sess-42');

    // Must match the backend route registered at /api/v1/proposal-sessions/{session_id}/ws
    expect(url).toContain('/api/v1/proposal-sessions/sess-42/ws');
    expect(url).toMatch(/^wss?:\/\//);
  });

  it('does NOT include project ID in the WebSocket path', () => {
    const url = getProposalSessionWsUrl('proj-1', 'sess-42');

    // The old (broken) path included /projects/{projectId}/ — ensure it's gone
    expect(url).not.toContain('/projects/');
    expect(url).not.toContain('proj-1');
  });
});

describe('stopAndDequeueChange', () => {
  it('calls stop-and-dequeue endpoint and returns parsed payload', async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      json: async () => ({
        change_id: 'change-a',
        selected: false,
        status: 'not queued',
      }),
    });

    vi.stubGlobal('fetch', fetchMock as unknown as typeof fetch);

    await expect(stopAndDequeueChange('project-1', 'change-a')).resolves.toEqual({
      change_id: 'change-a',
      selected: false,
      status: 'not queued',
    });
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/v1/projects/project-1/changes/change-a/stop-and-dequeue',
      expect.objectContaining({ method: 'POST' }),
    );
  });
});
