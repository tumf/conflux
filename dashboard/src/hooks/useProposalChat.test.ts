// @vitest-environment jsdom

import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { listProposalSessionMessages } from '../api/restClient';
import { useProposalChat } from './useProposalChat';

vi.mock('../api/restClient', () => ({
  getProposalSessionWsUrl: vi.fn(() => 'ws://localhost/ws'),
  listProposalSessionMessages: vi.fn(async () => ({ messages: [] })),
}));

const listProposalSessionMessagesMock = vi.mocked(listProposalSessionMessages);

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
  listProposalSessionMessagesMock.mockReset();
  listProposalSessionMessagesMock.mockResolvedValue({ messages: [] });
  vi.useFakeTimers();
  vi.stubGlobal('WebSocket', MockWebSocket as unknown as typeof WebSocket);
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe('useProposalChat', () => {
  it('loads history before connecting websocket', async () => {
    let resolveHistory: ((value: { messages: [] }) => void) | null = null;
    listProposalSessionMessagesMock.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveHistory = resolve;
        }),
    );

    renderHook(() => useProposalChat('project-1', 'session-1'));

    expect(MockWebSocket.instances).toHaveLength(0);

    await act(async () => {
      resolveHistory?.({ messages: [] });
      await Promise.resolve();
    });

    expect(MockWebSocket.instances).toHaveLength(1);
  });

  it('queues prompt while disconnected and flushes with client_message_id on reconnect', async () => {
    const { result } = renderHook(() => useProposalChat('project-1', 'session-1'));

    await act(async () => {
      await Promise.resolve();
    });

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

  it('replaces optimistic user message when server echoes matching client_message_id', async () => {
    const { result } = renderHook(() => useProposalChat('project-1', 'session-1'));

    await act(async () => {
      await Promise.resolve();
    });

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

  it('enters recovering and schedules reconnect when disconnected mid-turn', async () => {
    const { result } = renderHook(() => useProposalChat('project-1', 'session-1'));

    await act(async () => {
      await Promise.resolve();
    });

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

    expect(result.current.status).toBe('recovering');

    act(() => {
      vi.advanceTimersByTime(1000);
    });

    expect(MockWebSocket.instances.length).toBeGreaterThan(1);
  });

  it('transitions to streaming when tool_call arrives without message chunk', async () => {
    const { result } = renderHook(() => useProposalChat('project-1', 'session-1'));

    await act(async () => {
      await Promise.resolve();
    });

    const socket = MockWebSocket.instances[0];

    act(() => {
      socket.emitOpen();
      result.current.sendMessage('run tool only');
      socket.emitMessage({
        type: 'tool_call',
        message_id: 'assistant-tool-only',
        tool_call_id: 'tool-1',
        title: 'Read file',
        kind: 'read',
        status: 'pending',
      });
    });

    expect(result.current.status).toBe('streaming');
  });

  it('keeps recovering until replay indicates completed turn', async () => {
    const { result } = renderHook(() => useProposalChat('project-1', 'session-1'));

    await act(async () => {
      await Promise.resolve();
    });

    const socket = MockWebSocket.instances[0];

    act(() => {
      socket.emitOpen();
      result.current.sendMessage('recover me');
      socket.emitMessage({ type: 'agent_message_chunk', text: 'partial', message_id: 'assistant-1' });
    });

    act(() => {
      socket.emitClose();
    });

    expect(['recovering', 'streaming']).toContain(result.current.status);

    act(() => {
      vi.advanceTimersByTime(1000);
    });

    const reconnectSocket = MockWebSocket.instances[1];

    act(() => {
      reconnectSocket.emitOpen();
      reconnectSocket.emitMessage({ type: 'recovery_state', active: false });
      reconnectSocket.emitMessage({ type: 'turn_complete', stop_reason: 'end_turn' });
    });

    expect(result.current.status).toBe('ready');
    expect(result.current.error).toBeNull();
  });

  it('does not flush duplicate prompt once server already acknowledged it', async () => {
    const { result } = renderHook(() => useProposalChat('project-1', 'session-1'));

    await act(async () => {
      await Promise.resolve();
    });

    const firstSocket = MockWebSocket.instances[0];

    act(() => {
      result.current.sendMessage('once only');
      firstSocket.emitOpen();
      const firstSent = JSON.parse(firstSocket.sentMessages[0]);
      firstSocket.emitMessage({
        type: 'user_message',
        id: 'server-user-ack',
        content: 'once only',
        timestamp: '2026-04-01T00:00:00Z',
        client_message_id: firstSent.client_message_id,
      });
      firstSocket.emitClose();
    });

    act(() => {
      vi.advanceTimersByTime(1000);
    });

    const reconnectSocket = MockWebSocket.instances[1];
    act(() => {
      reconnectSocket.emitOpen();
    });

    expect(reconnectSocket.sentMessages).toHaveLength(0);
  });

  it('hydrates history from REST and preserves it after websocket connect', async () => {
    listProposalSessionMessagesMock.mockResolvedValueOnce({
      messages: [
        {
          id: 'history-1',
          role: 'assistant',
          content: 'Persisted response',
          timestamp: '2026-03-30T00:00:00Z',
        },
      ],
    });

    const { result } = renderHook(() => useProposalChat('project-1', 'session-1'));

    await act(async () => {
      await Promise.resolve();
    });

    const socket = MockWebSocket.instances[0];
    expect(result.current.messages).toHaveLength(1);
    expect(result.current.messages[0].id).toBe('history-1');

    act(() => {
      socket.emitOpen();
    });

    expect(result.current.messages).toHaveLength(1);
    expect(result.current.messages[0].id).toBe('history-1');
  });

  it('ignores stale history response after session switch', async () => {
    let resolveSessionA: ((value: { messages: Array<{ id: string; role: "assistant"; content: string; timestamp: string }> }) => void) | null = null;
    listProposalSessionMessagesMock
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveSessionA = resolve;
          }),
      )
      .mockResolvedValueOnce({
        messages: [
          {
            id: 'history-b',
            role: 'assistant',
            content: 'session-b',
            timestamp: '2026-04-01T00:00:00Z',
          },
        ],
      });

    const { result, rerender } = renderHook(
      ({ projectId, sessionId }) => useProposalChat(projectId, sessionId),
      {
        initialProps: { projectId: 'project-1', sessionId: 'session-a' as string | null },
      },
    );

    rerender({ projectId: 'project-1', sessionId: 'session-b' });

    await act(async () => {
      await Promise.resolve();
    });

    await act(async () => {
      resolveSessionA?.({
        messages: [
          {
            id: 'history-a',
            role: 'assistant',
            content: 'session-a',
            timestamp: '2026-03-31T00:00:00Z',
          },
        ],
      });
      await Promise.resolve();
    });

    expect(result.current.messages).toHaveLength(1);
    expect(result.current.messages[0].id).toBe('history-b');
    expect(result.current.messages[0].content).toBe('session-b');
  });

  it('does not duplicate assistant content when replay re-emits same message_id', async () => {
    listProposalSessionMessagesMock.mockResolvedValueOnce({
      messages: [
        {
          id: 'assistant-turn-1',
          role: 'assistant',
          content: 'Persisted response',
          timestamp: '2026-03-30T00:00:00Z',
          turn_id: 'turn-1',
        },
      ],
    });

    const { result } = renderHook(() => useProposalChat('project-1', 'session-1'));

    await act(async () => {
      await Promise.resolve();
    });

    const socket = MockWebSocket.instances[0];
    act(() => {
      socket.emitOpen();
      socket.emitMessage({
        type: 'agent_message_chunk',
        message_id: 'assistant-turn-1',
        turn_id: 'turn-1',
        text: 'Persisted response',
      });
      socket.emitMessage({
        type: 'turn_complete',
        stop_reason: 'end_turn',
        message_id: 'assistant-turn-1',
        turn_id: 'turn-1',
      });
    });

    expect(result.current.messages).toHaveLength(1);
    expect(result.current.messages[0].id).toBe('assistant-turn-1');
    expect(result.current.messages[0].content).toBe('Persisted response');
  });
});
