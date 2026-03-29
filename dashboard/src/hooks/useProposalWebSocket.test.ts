import { describe, expect, it, vi } from 'vitest';

import { ElicitationRequest, ToolCallInfo } from '../api/types';
import { handleServerMessage, UseProposalWebSocketOptions } from './useProposalWebSocket';

function makeCallbacks(overrides: Partial<UseProposalWebSocketOptions> = {}): UseProposalWebSocketOptions {
  return {
    projectId: 'project-1',
    sessionId: 'session-1',
    ...overrides,
  };
}

describe('handleServerMessage', () => {
  it('dispatches streaming chunks', () => {
    const onMessageChunk = vi.fn();

    handleServerMessage(
      { type: 'agent_message_chunk', text: 'partial' },
      makeCallbacks({ onMessageChunk }),
    );

    expect(onMessageChunk).toHaveBeenCalledWith('partial');
  });

  it('dispatches tool call events and updates', () => {
    const onToolCall = vi.fn();
    const onToolCallUpdate = vi.fn();
    const toolCall: ToolCallInfo = {
      id: 'tool-1',
      title: 'Run validation',
      status: 'pending',
    };

    handleServerMessage(
      {
        type: 'tool_call',
        tool_call_id: 'tool-1',
        title: 'Run validation',
        kind: 'task',
        status: 'pending',
      },
      makeCallbacks({ onToolCall }),
    );
    handleServerMessage(
      {
        type: 'tool_call_update',
        tool_call_id: 'tool-1',
        status: 'completed',
        content: [],
      },
      makeCallbacks({ onToolCallUpdate }),
    );

    expect(onToolCall).toHaveBeenCalledWith(toolCall);
    expect(onToolCallUpdate).toHaveBeenCalledWith('tool-1', 'completed');
  });

  it('dispatches elicitation requests and turn completion', () => {
    const onElicitationRequest = vi.fn();
    const onTurnComplete = vi.fn();
    const elicitation: ElicitationRequest = {
      id: 'elicitation-1',
      message: 'Need more details',
      properties: {
        answer: { type: 'string', title: 'Answer' },
      },
      required: ['answer'],
    };

    handleServerMessage(
      {
        type: 'elicitation',
        request_id: 'elicitation-1',
        mode: 'form',
        message: 'Need more details',
        schema: {
          properties: {
            answer: { type: 'string', title: 'Answer' },
          },
          required: ['answer'],
        },
      },
      makeCallbacks({ onElicitationRequest }),
    );
    handleServerMessage(
      { type: 'turn_complete', stop_reason: 'completed' },
      makeCallbacks({ onTurnComplete }),
    );

    expect(onElicitationRequest).toHaveBeenCalledWith(elicitation);
    expect(onTurnComplete).toHaveBeenCalledWith('completed');
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
