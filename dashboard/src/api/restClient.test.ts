// @vitest-environment jsdom
import { describe, expect, it } from 'vitest';

import { getProposalSessionWsUrl } from './restClient';

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
