import { describe, expect, it, vi } from 'vitest';

import { ElicitationRequest, ProposalChatMessage, ProposalSession, ToolCallInfo } from '../api/types';
import { handleServerMessage, UseProposalWebSocketOptions } from './useProposalWebSocket';

function makeCallbacks(overrides: Partial<UseProposalWebSocketOptions> = {}): UseProposalWebSocketOptions {
  return {
    projectId: 'project-1',
    sessionId: 'session-1',
    ...overrides,
  };
}

describe('handleServerMessage', () => {
  it('dispatches assistant messages', () => {
    const onMessage = vi.fn();
    const message: ProposalChatMessage = {
      id: 'msg-1',
      role: 'assistant',
      content: 'hello',
      timestamp: '2026-03-29T00:00:00Z',
    };

    handleServerMessage(
      { type: 'assistant_message', message },
      makeCallbacks({ onMessage }),
    );

    expect(onMessage).toHaveBeenCalledWith(message);
  });

  it('dispatches streaming chunks', () => {
    const onMessageChunk = vi.fn();

    handleServerMessage(
      { type: 'assistant_chunk', message_id: 'msg-2', content: 'partial' },
      makeCallbacks({ onMessageChunk }),
    );

    expect(onMessageChunk).toHaveBeenCalledWith('msg-2', 'partial');
  });

  it('dispatches tool call start and updates', () => {
    const onToolCallStart = vi.fn();
    const onToolCallUpdate = vi.fn();
    const toolCall: ToolCallInfo = {
      id: 'tool-1',
      title: 'Run validation',
      status: 'pending',
    };

    handleServerMessage(
      { type: 'tool_call_start', message_id: 'msg-3', tool_call: toolCall },
      makeCallbacks({ onToolCallStart }),
    );
    handleServerMessage(
      {
        type: 'tool_call_update',
        message_id: 'msg-3',
        tool_call_id: 'tool-1',
        status: 'completed',
      },
      makeCallbacks({ onToolCallUpdate }),
    );

    expect(onToolCallStart).toHaveBeenCalledWith('msg-3', toolCall);
    expect(onToolCallUpdate).toHaveBeenCalledWith('msg-3', 'tool-1', 'completed');
  });

  it('dispatches elicitation requests and session updates', () => {
    const onElicitationRequest = vi.fn();
    const onSessionUpdate = vi.fn();
    const elicitation: ElicitationRequest = {
      id: 'elicitation-1',
      message: 'Need more details',
      properties: {
        answer: { type: 'string', title: 'Answer' },
      },
      required: ['answer'],
    };
    const session: ProposalSession = {
      id: 'session-1',
      project_id: 'project-1',
      status: 'active',
      worktree_branch: 'proposal/session-1',
      is_dirty: false,
      uncommitted_files: [],
      created_at: '2026-03-29T00:00:00Z',
      updated_at: '2026-03-29T00:00:00Z',
    };

    handleServerMessage(
      { type: 'elicitation_request', elicitation },
      makeCallbacks({ onElicitationRequest }),
    );
    handleServerMessage(
      { type: 'session_update', session },
      makeCallbacks({ onSessionUpdate }),
    );

    expect(onElicitationRequest).toHaveBeenCalledWith(elicitation);
    expect(onSessionUpdate).toHaveBeenCalledWith(session);
  });

  it('dispatches errors', () => {
    const onError = vi.fn();

    handleServerMessage(
      { type: 'error', message: 'boom' },
      makeCallbacks({ onError }),
    );

    expect(onError).toHaveBeenCalledWith('boom');
  });
});
