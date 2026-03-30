// @vitest-environment jsdom

import React from 'react';
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { ElicitationRequest, ToolCallInfo } from '../api/types';
import { handleServerMessage, UseProposalWebSocketOptions, useProposalWebSocket } from './useProposalWebSocket';

vi.mock('../api/restClient', () => ({
  getProposalSessionWsUrl: vi.fn(() => 'ws://localhost/ws'),
}));

class MockWebSocket {
  static instances: MockWebSocket[] = [];
  static OPEN = 1;
  static CONNECTING = 0;

  readyState = MockWebSocket.CONNECTING;
  sentMessages: string[] = [];
  onopen: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;

  constructor(public readonly url: string) {
    MockWebSocket.instances.push(this);
  }

  send(payload: string): void {
    this.sentMessages.push(payload);
  }

  close(): void {
    this.readyState = 3;
  }

  emitOpen(): void {
    this.readyState = MockWebSocket.OPEN;
    this.onopen?.(new Event('open'));
  }
}

function makeCallbacks(overrides: Partial<UseProposalWebSocketOptions> = {}): UseProposalWebSocketOptions {
  return {
    projectId: 'project-1',
    sessionId: 'session-1',
    ...overrides,
  };
}

beforeEach(() => {
  MockWebSocket.instances = [];
  vi.stubGlobal('WebSocket', MockWebSocket as unknown as typeof WebSocket);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

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

describe('useProposalWebSocket prompt queueing', () => {
  it('queues prompt while disconnected and flushes in order after reconnect', () => {
    const onPromptQueued = vi.fn();
    const onPromptSendStarted = vi.fn();

    const { result } = renderHook(() =>
      useProposalWebSocket({
        projectId: 'project-1',
        sessionId: 'session-1',
        onPromptQueued,
        onPromptSendStarted,
      }),
    );

    const ws = MockWebSocket.instances[0];
    expect(ws).toBeDefined();

    act(() => {
      const queued = result.current.sendPrompt('first prompt', 'client-1');
      expect(queued.queued).toBe(true);
      expect(queued.clientMessageId).toBe('client-1');
    });

    expect(onPromptQueued).toHaveBeenCalledWith('client-1');
    expect(ws.sentMessages).toHaveLength(0);

    act(() => {
      const queued = result.current.sendPrompt('second prompt', 'client-2');
      expect(queued.queued).toBe(true);
    });

    act(() => {
      ws.emitOpen();
    });

    expect(onPromptSendStarted).toHaveBeenNthCalledWith(1, 'client-1');
    expect(onPromptSendStarted).toHaveBeenNthCalledWith(2, 'client-2');
    expect(ws.sentMessages).toEqual([
      JSON.stringify({ type: 'prompt', content: 'first prompt' }),
      JSON.stringify({ type: 'prompt', content: 'second prompt' }),
    ]);
  });

  it('marks send failure when websocket send throws', () => {
    const onPromptSendFailed = vi.fn();

    const { result } = renderHook(() =>
      useProposalWebSocket({
        projectId: 'project-1',
        sessionId: 'session-1',
        onPromptSendFailed,
      }),
    );

    const ws = MockWebSocket.instances[0];
    expect(ws).toBeDefined();

    ws.send = () => {
      throw new Error('send failed');
    };

    act(() => {
      ws.emitOpen();
      const sendResult = result.current.sendPrompt('prompt', 'client-fail');
      expect(sendResult.queued).toBe(false);
    });

    expect(onPromptSendFailed).toHaveBeenCalledWith('client-fail', 'send failed');
  });
});
