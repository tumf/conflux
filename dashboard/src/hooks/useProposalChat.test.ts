// @vitest-environment jsdom

import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useProposalChat } from './useProposalChat';

vi.mock('../api/restClient', () => ({
  getProposalSessionWsUrl: vi.fn(() => 'ws://localhost/ws'),
  listProposalSessionMessages: vi.fn(async () => ({ messages: [] })),
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

  emitMessage(payload: unknown): void {
    this.onmessage?.(new MessageEvent('message', { data: JSON.stringify(payload) }));
  }

  emitClose(): void {
    this.readyState = 3;
    this.onclose?.(new CloseEvent('close'));
  }
}

beforeEach(() => {
  MockWebSocket.instances = [];
  vi.useFakeTimers();
  vi.stubGlobal('WebSocket', MockWebSocket as unknown as typeof WebSocket);
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('useProposalChat', () => {
  it('queues prompt while disconnected and flushes with client_message_id on reconnect', async () => {
    const { result } = renderHook(() => useProposalChat('project-1', 'session-1'));

    const firstSocket = MockWebSocket.instances[0];
    expect(firstSocket).toBeDefined();

    act(() => {
      result.current.sendMessage('hello');
    });

    expect(result.current.status).toBe('submitted');
    expect(firstSocket.sentMessages).toHaveLength(0);

    act(() => {
      firstSocket.emitOpen();
    });

    act(() => {
      vi.runOnlyPendingTimers();
    });

    expect(firstSocket.sentMessages).toHaveLength(1);
    const sent = JSON.parse(firstSocket.sentMessages[0]);
    expect(sent.type).toBe('prompt');
    expect(sent.content).toBe('hello');
    expect(typeof sent.client_message_id).toBe('string');
  });

  it('replaces optimistic user message when server echoes matching client_message_id', () => {
    const { result } = renderHook(() => useProposalChat('project-1', 'session-1'));
    const socket = MockWebSocket.instances[0];

    act(() => {
      result.current.sendMessage('replace me');
      socket.emitOpen();
      vi.runOnlyPendingTimers();
    });

    const sent = JSON.parse(socket.sentMessages[0]);

    act(() => {
      socket.emitMessage({
        type: 'user_message',
        id: 'server-user-1',
        content: 'replace me',
        timestamp: '2026-04-01T00:00:00Z',
        client_message_id: sent.client_message_id,
      });
    });

    expect(result.current.messages).toHaveLength(1);
    expect(result.current.messages[0].id).toBe('server-user-1');
    expect(result.current.messages[0].sendStatus).toBe('sent');
  });

  it('marks turn error and schedules reconnect when disconnected mid-turn', () => {
    const { result } = renderHook(() => useProposalChat('project-1', 'session-1'));
    const socket = MockWebSocket.instances[0];

    act(() => {
      socket.emitOpen();
      result.current.sendMessage('hello');
      socket.emitMessage({ type: 'agent_message_chunk', text: 'partial', message_id: 'assistant-1' });
    });

    expect(result.current.status).toBe('streaming');

    act(() => {
      socket.emitClose();
    });

    act(() => {
      vi.runOnlyPendingTimers();
    });

    expect(result.current.status).toBe('error');

    act(() => {
      vi.advanceTimersByTime(1000);
    });

    expect(MockWebSocket.instances.length).toBeGreaterThan(1);
  });
});
