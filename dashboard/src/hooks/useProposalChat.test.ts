// @vitest-environment jsdom

import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useProposalChat } from './useProposalChat';

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
  it('connects websocket immediately without REST hydration prerequisite', async () => {
    renderHook(() => useProposalChat('project-1', 'session-1'));

    await act(async () => {
      await Promise.resolve();
    });

    expect(MockWebSocket.instances).toHaveLength(1);
  });

  it('queues prompt while disconnected and flushes on reconnect with client_message_id', async () => {
    const { result } = renderHook(() => useProposalChat('project-1', 'session-1'));

    await act(async () => {
      await Promise.resolve();
    });

    const socket = MockWebSocket.instances[0];

    act(() => {
      result.current.sendMessage('hello');
    });

    expect(result.current.status).toBe('submitted');
    expect(result.current.submissionLock.isLocked).toBe(true);
    expect(socket.sentMessages).toHaveLength(0);
    expect(result.current.messages.at(-1)?.sendStatus).toBe('pending');

    act(() => {
      socket.emitOpen();
      vi.runOnlyPendingTimers();
    });

    expect(socket.sentMessages).toHaveLength(1);
    const payload = JSON.parse(socket.sentMessages[0]);
    expect(payload.type).toBe('prompt');
    expect(payload.content).toBe('hello');
    expect(typeof payload.client_message_id).toBe('string');
  });

  it('clears submission lock after matching user_message ACK and marks sent', async () => {
    const { result } = renderHook(() => useProposalChat('project-1', 'session-1'));

    await act(async () => {
      await Promise.resolve();
    });

    const socket = MockWebSocket.instances[0];

    act(() => {
      socket.emitOpen();
      result.current.sendMessage('replace me');
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

    expect(result.current.submissionLock.isLocked).toBe(false);
    expect(result.current.messages).toHaveLength(1);
    expect(result.current.messages[0].id).toBe('server-user-1');
    expect(result.current.messages[0].sendStatus).toBe('sent');
  });

  it('marks pending user message as failed after reconnect limit is reached', async () => {
    const { result } = renderHook(() => useProposalChat('project-1', 'session-1'));

    await act(async () => {
      await Promise.resolve();
    });

    act(() => {
      result.current.sendMessage('will fail');
    });

    for (let i = 0; i < 11; i += 1) {
      const socket = MockWebSocket.instances[i];
      act(() => {
        socket.emitClose();
      });
      if (i < 10) {
        act(() => {
          vi.runOnlyPendingTimers();
        });
      }
    }

    expect(result.current.status).toBe('error');
    expect(result.current.submissionLock.isLocked).toBe(false);
    const pendingMessage = result.current.messages.find((message) => message.role === 'user');
    expect(pendingMessage?.sendStatus).toBe('failed');
  });

  it('retries failed message with same message id and pending status', async () => {
    const { result } = renderHook(() => useProposalChat('project-1', 'session-1'));

    await act(async () => {
      await Promise.resolve();
    });

    const socket = MockWebSocket.instances[0];
    act(() => {
      socket.emitOpen();
      result.current.sendMessage('retry me');
    });

    const firstPayload = JSON.parse(socket.sentMessages[0]);

    act(() => {
      socket.emitMessage({ type: 'error', message: 'delivery failed' });
    });

    const failedMessage = result.current.messages.find((message) => message.role === 'user');
    expect(failedMessage?.id).toBe(firstPayload.client_message_id);
    expect(failedMessage?.sendStatus).toBe('failed');

    act(() => {
      result.current.retryMessage(failedMessage!.id);
    });

    const retriedMessage = result.current.messages.find((message) => message.id === failedMessage!.id);
    expect(retriedMessage?.sendStatus).toBe('pending');
    expect(socket.sentMessages).toHaveLength(2);

    const retryPayload = JSON.parse(socket.sentMessages[1]);
    expect(retryPayload.client_message_id).toBe(failedMessage!.id);
    expect(retryPayload.content).toBe('retry me');
  });

  it('does not flush duplicate prompt after server already acknowledged it', async () => {
    const { result } = renderHook(() => useProposalChat('project-1', 'session-1'));

    await act(async () => {
      await Promise.resolve();
    });

    const firstSocket = MockWebSocket.instances[0];

    act(() => {
      firstSocket.emitOpen();
      result.current.sendMessage('once only');
    });

    const firstSent = JSON.parse(firstSocket.sentMessages[0]);

    act(() => {
      firstSocket.emitMessage({
        type: 'user_message',
        id: 'server-user-ack',
        content: 'once only',
        timestamp: '2026-04-01T00:00:00Z',
        client_message_id: firstSent.client_message_id,
      });
      firstSocket.emitClose();
      vi.advanceTimersByTime(1000);
    });

    const reconnectSocket = MockWebSocket.instances[1];
    act(() => {
      reconnectSocket.emitOpen();
    });

    expect(reconnectSocket.sentMessages).toHaveLength(0);
  });
});
