/**
 * @vitest-environment jsdom
 */

import React from 'react';
import { act, cleanup, render, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { TerminalTab } from './TerminalTab';

const {
  MockFitAddon,
  MockResizeObserver,
  MockTerminal,
  MockWebSocket,
  getTerminalWsUrlMock,
  fitAddonInstances,
  sockets,
  terminalInstances,
  terminalHandlers,
} = vi.hoisted(() => {
  const getTerminalWsUrlMock = vi.fn((sessionId: string) => `ws://localhost/api/v1/terminal/sessions/${sessionId}/ws`);
  const terminalInstances: MockTerminal[] = [];
  const fitAddonInstances: MockFitAddon[] = [];
  const sockets: MockWebSocket[] = [];
  const terminalHandlers: {
    data: ((data: string) => void) | null;
    resize: ((dimensions: { cols: number; rows: number }) => void) | null;
  } = { data: null, resize: null };

  class MockTerminal {
  public writes: unknown[] = [];
  public disposed = false;
  public openedElement: HTMLElement | null = null;
  public focused = false;

  constructor(public options: unknown) {
    terminalInstances.push(this);
  }

  loadAddon(_addon: unknown) {}

  open(element: HTMLElement) {
    this.openedElement = element;
    const helper = document.createElement('textarea');
    helper.className = 'xterm-helper-textarea';
    element.appendChild(helper);
  }

  attachCustomKeyEventHandler(_handler: (event: KeyboardEvent) => boolean) {}

  hasSelection() {
    return false;
  }

  write(data: unknown) {
    this.writes.push(data);
  }

  onData(handler: (data: string) => void) {
    terminalHandlers.data = handler;
    return { dispose: vi.fn() };
  }

  onResize(handler: (dimensions: { cols: number; rows: number }) => void) {
    terminalHandlers.resize = handler;
    return { dispose: vi.fn() };
  }

  focus() {
    this.focused = true;
  }

  dispose() {
    this.disposed = true;
  }
}

class MockFitAddon {
  public fit = vi.fn();

  constructor() {
    fitAddonInstances.push(this);
  }

  proposeDimensions() {
    return { cols: 132, rows: 43 };
  }
}

class MockWebSocket {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;

  public binaryType = '';
  public readyState = MockWebSocket.CONNECTING;
  public sent: unknown[] = [];
  public closed = false;
  public onopen: (() => void) | null = null;
  public onmessage: ((event: MessageEvent) => void) | null = null;
  public onerror: ((event: Event) => void) | null = null;
  public onclose: (() => void) | null = null;

  constructor(public url: string) {
    sockets.push(this);
  }

  send(payload: unknown) {
    this.sent.push(payload);
  }

  close() {
    this.readyState = MockWebSocket.CLOSED;
    this.closed = true;
  }
}

  class MockResizeObserver {
    observe(_element: Element) {}
    disconnect() {}
  }

  return {
    MockFitAddon,
    MockResizeObserver,
    MockTerminal,
    MockWebSocket,
    getTerminalWsUrlMock,
    fitAddonInstances,
    sockets,
    terminalHandlers,
    terminalInstances,
  };
});

vi.mock('../api/restClient', () => ({
  getTerminalWsUrl: (sessionId: string) => getTerminalWsUrlMock(sessionId),
}));

vi.mock('@xterm/xterm', () => ({
  Terminal: MockTerminal,
}));

vi.mock('@xterm/addon-fit', () => ({
  FitAddon: MockFitAddon,
}));

vi.mock('@xterm/xterm/css/xterm.css', () => ({}));

beforeEach(() => {
  vi.stubGlobal('WebSocket', MockWebSocket);
  vi.stubGlobal('ResizeObserver', MockResizeObserver);
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    callback(0);
    return 0;
  });
  terminalInstances.length = 0;
  fitAddonInstances.length = 0;
  sockets.length = 0;
  terminalHandlers.data = null;
  terminalHandlers.resize = null;
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  getTerminalWsUrlMock.mockClear();
});

describe('TerminalTab lifecycle', () => {
  it('opens a WebSocket for the session and preserves resize/input/data payload behavior', async () => {
    render(<TerminalTab sessionId="session-a" isActive />);

    await waitFor(() => {
      expect(sockets).toHaveLength(1);
    });

    const socket = sockets[0];
    const terminal = terminalInstances[0];
    expect(getTerminalWsUrlMock).toHaveBeenCalledWith('session-a');
    expect(socket.url).toBe('ws://localhost/api/v1/terminal/sessions/session-a/ws');
    expect(socket.binaryType).toBe('arraybuffer');

    act(() => {
      socket.readyState = MockWebSocket.OPEN;
      socket.onopen?.();
    });

    expect(socket.sent).toContain(JSON.stringify({ rows: 43, cols: 132 }));

    act(() => {
      terminalHandlers.resize?.({ cols: 100, rows: 30 });
    });
    expect(socket.sent).toContain(JSON.stringify({ rows: 30, cols: 100 }));

    act(() => {
      terminalHandlers.data?.('ls\n');
    });
    const encodedInput = socket.sent.find(
      (payload) => ArrayBuffer.isView(payload) && payload.constructor.name === 'Uint8Array',
    ) as Uint8Array;
    expect(Array.from(encodedInput)).toEqual(Array.from(new TextEncoder().encode('ls\n')));

    const data = new Uint8Array([65, 66]).buffer;
    act(() => {
      socket.onmessage?.({ data } as MessageEvent);
      socket.onmessage?.({ data: 'plain text' } as MessageEvent);
      socket.onerror?.(new Event('error'));
      socket.onclose?.();
    });

    expect(terminal.writes).toContain('plain text');
    expect(terminal.writes).toContain('\r\n\x1b[31m[Terminal connection error]\x1b[0m\r\n');
    expect(terminal.writes).toContain('\r\n\x1b[33m[Terminal session ended]\x1b[0m\r\n');
    expect(terminal.writes.some((value) => ArrayBuffer.isView(value) && value.constructor.name === 'Uint8Array')).toBe(true);
  });

  it('closes the old socket and disposes the old terminal when the session changes or unmounts', async () => {
    const rendered = render(<TerminalTab sessionId="session-a" isActive />);

    await waitFor(() => {
      expect(sockets).toHaveLength(1);
    });

    const firstSocket = sockets[0];
    const firstTerminal = terminalInstances[0];

    rendered.rerender(<TerminalTab sessionId="session-b" isActive />);

    await waitFor(() => {
      expect(sockets).toHaveLength(2);
    });

    expect(firstSocket.closed).toBe(true);
    expect(firstTerminal.disposed).toBe(true);
    expect(getTerminalWsUrlMock).toHaveBeenLastCalledWith('session-b');

    const secondSocket = sockets[1];
    const secondTerminal = terminalInstances[1];

    rendered.unmount();

    expect(secondSocket.closed).toBe(true);
    expect(secondTerminal.disposed).toBe(true);
  });
});
