import React from 'react';
import { Wrench, Loader2, CheckCircle2, XCircle } from 'lucide-react';
import { ToolCallInfo, ToolCallStatus } from '../api/types';

interface ToolCallIndicatorProps {
  toolCall: ToolCallInfo;
}

const statusConfig: Record<ToolCallStatus, { icon: React.ReactNode; color: string; bg: string; label: string }> = {
  pending: {
    icon: <Wrench className="size-3" />,
    color: 'text-text-muted',
    bg: 'bg-border',
    label: 'Pending',
  },
  in_progress: {
    icon: <Loader2 className="size-3 animate-spin" />,
    color: 'text-warning',
    bg: 'bg-warning/15',
    label: 'Running',
  },
  completed: {
    icon: <CheckCircle2 className="size-3" />,
    color: 'text-success',
    bg: 'bg-success/15',
    label: 'Done',
  },
  failed: {
    icon: <XCircle className="size-3" />,
    color: 'text-error',
    bg: 'bg-error/15',
    label: 'Failed',
  },
};

export function ToolCallIndicator({ toolCall }: ToolCallIndicatorProps) {
  const cfg = statusConfig[toolCall.status];

  return (
    <div className={`inline-flex items-center gap-1.5 rounded-md px-2 py-1 text-xs ${cfg.color} ${cfg.bg}`}>
      {cfg.icon}
      <span className="font-medium">{toolCall.title}</span>
      <span className="text-[0.65rem] opacity-70">{cfg.label}</span>
    </div>
  );
}
