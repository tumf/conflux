import React, { useEffect, useRef, useCallback } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { getTerminalWsUrl } from '../api/restClient';
import '@xterm/xterm/css/xterm.css';

interface TerminalTabProps {
  sessionId: string;
  isActive: boolean;
}

const SHELL_CONTROL_KEYS = new Set(['a', 'e', 'k', 'u', 'l', 'r', 'd', 'w']);

export function TerminalTab({ sessionId, isActive }: TerminalTabProps) {
  const terminalRef = useRef<HTMLDivElement>(null);
  const xtermRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const initializedRef = useRef(false);
  const helperTextAreaRef = useRef<HTMLTextAreaElement | null>(null);
  const textEncoderRef = useRef(new TextEncoder());

  // Fit terminal to container
  const fitTerminal = useCallback(() => {
    if (fitAddonRef.current && xtermRef.current) {
      try {
        fitAddonRef.current.fit();
      } catch {
        // Ignore fit errors when terminal is not visible
      }
    }
  }, []);

  const resolveHelperTextArea = useCallback(() => {
    if (helperTextAreaRef.current?.isConnected) {
      return helperTextAreaRef.current;
    }

    const container = terminalRef.current;
    const helperTextArea = container?.querySelector('textarea.xterm-helper-textarea') as HTMLTextAreaElement | null;
    helperTextAreaRef.current = helperTextArea;
    return helperTextArea;
  }, []);

  const clearHelperTextArea = useCallback(() => {
    requestAnimationFrame(() => {
      const helperTextArea = resolveHelperTextArea();
      if (helperTextArea) {
        helperTextArea.value = '';
      }
    });
  }, [resolveHelperTextArea]);

  // Send resize to server
  const sendResize = useCallback(
    (cols: number, rows: number) => {
      if (wsRef.current?.readyState === WebSocket.OPEN) {
        wsRef.current.send(JSON.stringify({ rows, cols }));
      }
    },
    [],
  );

  // Initialize terminal
  useEffect(() => {
    if (initializedRef.current || !terminalRef.current) return;
    initializedRef.current = true;

    const term = new Terminal({
      theme: {
        background: '#0a0a0a',
        foreground: '#d4d4d8',
        cursor: '#a5b4fc',
        selectionBackground: '#1e1b4b',
        black: '#18181b',
        red: '#ef4444',
        green: '#22c55e',
        yellow: '#eab308',
        blue: '#6366f1',
        magenta: '#a855f7',
        cyan: '#06b6d4',
        white: '#d4d4d8',
        brightBlack: '#52525b',
        brightRed: '#f87171',
        brightGreen: '#4ade80',
        brightYellow: '#facc15',
        brightBlue: '#818cf8',
        brightMagenta: '#c084fc',
        brightCyan: '#22d3ee',
        brightWhite: '#fafafa',
      },
      fontSize: 13,
      fontFamily: 'ui-monospace, "SF Mono", Menlo, Monaco, "Cascadia Code", monospace',
      cursorBlink: true,
      scrollback: 5000,
      allowProposedApi: true,
    });

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);

    term.open(terminalRef.current);

    term.attachCustomKeyEventHandler((event) => {
      const isModifierKey = event.ctrlKey && !event.shiftKey && !event.altKey && !event.metaKey;
      const key = event.key.toLowerCase();

      if (isModifierKey && key === 'c' && event.type === 'keydown') {
        return !term.hasSelection();
      }

      if (isModifierKey && SHELL_CONTROL_KEYS.has(key)) {
        return true;
      }

      return true;
    });
    xtermRef.current = term;
    fitAddonRef.current = fitAddon;

    // Initial fit
    requestAnimationFrame(() => {
      fitTerminal();
    });

    // Connect WebSocket
    const wsUrl = getTerminalWsUrl(sessionId);
    const ws = new WebSocket(wsUrl);
    ws.binaryType = 'arraybuffer';
    wsRef.current = ws;

    ws.onopen = () => {
      // Send initial size
      const dims = fitAddon.proposeDimensions();
      if (dims) {
        sendResize(dims.cols, dims.rows);
      }
    };

    ws.onmessage = (event) => {
      if (event.data instanceof ArrayBuffer) {
        term.write(new Uint8Array(event.data));
      } else if (typeof event.data === 'string') {
        term.write(event.data);
      }
    };

    ws.onerror = (event) => {
      console.error('Terminal WebSocket error:', event);
      term.write('\r\n\x1b[31m[Terminal connection error]\x1b[0m\r\n');
    };

    ws.onclose = () => {
      term.write('\r\n\x1b[33m[Terminal session ended]\x1b[0m\r\n');
    };

    // Forward terminal input to WebSocket
    term.onData((data) => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.send(textEncoderRef.current.encode(data));
      }
      clearHelperTextArea();
    });

    // Handle terminal resize
    term.onResize(({ cols, rows }) => {
      sendResize(cols, rows);
    });

    // Cleanup
    return () => {
      helperTextAreaRef.current = null;
      if (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING) {
        ws.close();
      }
      term.dispose();
      initializedRef.current = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId]);

  // Refit when becoming active
  useEffect(() => {
    if (isActive) {
      requestAnimationFrame(() => {
        fitTerminal();
        xtermRef.current?.focus();
      });
    }
  }, [isActive, fitTerminal]);

  // ResizeObserver to refit terminal when container resizes
  useEffect(() => {
    const container = terminalRef.current;
    if (!container) return;

    const observer = new ResizeObserver(() => {
      fitTerminal();
    });
    observer.observe(container);

    return () => {
      observer.disconnect();
    };
  }, [fitTerminal]);

  return (
    <div
      ref={terminalRef}
      className="h-full w-full"
      style={{ padding: '4px' }}
    />
  );
}
