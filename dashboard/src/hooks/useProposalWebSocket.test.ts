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
  it('dispatches user messages', () => {
    const onUserMessage = vi.fn();

    handleServerMessage(
      { type: 'user_message', id: 'user-1', content: 'hello', timestamp: '2026-03-30T00:00:00Z' },
      makeCallbacks({ onUserMessage }),
    );

    expect(onUserMessage).toHaveBeenCalledWith({
      id: 'user-1',
      content: 'hello',
      timestamp: '2026-03-30T00:00:00Z',
    });
  });

  it('dispatches streaming chunks with message/turn ids', () => {
    const onMessageChunk = vi.fn();

    handleServerMessage(
      { type: 'agent_message_chunk', text: 'partial', message_id: 'assistant-1', turn_id: 'turn-1' },
      makeCallbacks({ onMessageChunk }),
    );

    expect(onMessageChunk).toHaveBeenCalledWith('partial', 'assistant-1', 'turn-1');
  });

  it('dispatches thought chunks with message/turn ids', () => {
    const onThoughtChunk = vi.fn();

    handleServerMessage(
      {
        type: 'agent_thought_chunk',
        text: 'thinking...',
        message_id: 'assistant-1',
        turn_id: 'turn-1',
      },
      makeCallbacks({ onThoughtChunk }),
    );

    expect(onThoughtChunk).toHaveBeenCalledWith('thinking...', 'assistant-1', 'turn-1');
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
        message_id: 'assistant-1',
        turn_id: 'turn-1',
      },
      makeCallbacks({ onToolCall }),
    );
    handleServerMessage(
      {
        type: 'tool_call_update',
        tool_call_id: 'tool-1',
        status: 'completed',
        content: [],
        message_id: 'assistant-1',
        turn_id: 'turn-1',
      },
      makeCallbacks({ onToolCallUpdate }),
    );

    expect(onToolCall).toHaveBeenCalledWith(toolCall, 'assistant-1', 'turn-1');
    expect(onToolCallUpdate).toHaveBeenCalledWith('tool-1', 'completed', 'assistant-1', 'turn-1');
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
      { type: 'turn_complete', stop_reason: 'completed', message_id: 'assistant-1', turn_id: 'turn-1' },
      makeCallbacks({ onTurnComplete }),
    );

    expect(onElicitationRequest).toHaveBeenCalledWith(elicitation);
    expect(onTurnComplete).toHaveBeenCalledWith('completed', 'assistant-1', 'turn-1');
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
