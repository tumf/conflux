import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { Activity, CheckCircle2, Clock3, RefreshCw, XCircle } from 'lucide-react';

import { fetchStatsOverview } from '../api/restClient';
import { ChangeEventSummary, ProjectStats, StatsOverview } from '../api/types';

function formatDurationMs(durationMs: number | null | undefined): string {
  if (durationMs == null || Number.isNaN(durationMs)) {
    return '-';
  }
  if (durationMs < 1000) {
    return `${Math.round(durationMs)}ms`;
  }
  const seconds = durationMs / 1000;
  if (seconds < 60) {
    return `${seconds.toFixed(1)}s`;
  }
  const minutes = Math.floor(seconds / 60);
  const remainSeconds = Math.round(seconds % 60);
  return `${minutes}m ${remainSeconds}s`;
}

function formatTimestamp(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString();
}

function formatPercent(rate: number): string {
  const normalized = Number.isFinite(rate) ? rate : 0;
  const base = normalized > 1 ? normalized : normalized * 100;
  return `${Math.max(0, Math.min(100, base)).toFixed(1)}%`;
}

function getProjectLabel(project: Pick<ProjectStats, 'project_name' | 'project_id'>): string {
  return project.project_name || project.project_id;
}

function getEventProjectLabel(event: Pick<ChangeEventSummary, 'project_name' | 'project_id'>): string {
  return event.project_name || event.project_id;
}

export function OverviewDashboard() {
  const [overview, setOverview] = useState<StatsOverview | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadOverview = useCallback(async () => {
    setIsLoading(true);
    setError(null);

    try {
      const data = await fetchStatsOverview();
      setOverview(data);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.error('[OverviewDashboard] failed to fetch stats overview', { message, err });
      setError(message);
      setOverview(null);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadOverview();
  }, [loadOverview]);

  const summaryCards = useMemo(() => {
    const summary = overview?.summary;
    return [
      {
        label: 'Success',
        value: summary?.success_count ?? 0,
        icon: CheckCircle2,
        textClass: 'text-[#22c55e]',
        bgClass: 'bg-[#052e16]/40',
      },
      {
        label: 'Failure',
        value: summary?.failure_count ?? 0,
        icon: XCircle,
        textClass: 'text-[#ef4444]',
        bgClass: 'bg-[#450a0a]/40',
      },
      {
        label: 'In Progress',
        value: summary?.in_progress_count ?? 0,
        icon: Activity,
        textClass: 'text-[#f59e0b]',
        bgClass: 'bg-[#451a03]/40',
      },
      {
        label: 'Avg Duration',
        value: formatDurationMs(summary?.average_duration_ms),
        icon: Clock3,
        textClass: 'text-[#a1a1aa]',
        bgClass: 'bg-[#18181b]',
      },
    ];
  }, [overview]);

  return (
    <div className="flex h-full flex-col overflow-hidden bg-[#09090b] p-4 md:p-6">
      <div className="mb-4 flex items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold text-[#fafafa]">Orchestration Overview</h2>
          <p className="text-xs text-[#71717a]">Global stats across all projects</p>
        </div>
        <button
          onClick={() => void loadOverview()}
          disabled={isLoading}
          className="inline-flex items-center gap-1.5 rounded-md border border-[#27272a] bg-[#111113] px-3 py-1.5 text-xs text-[#d4d4d8] transition-colors hover:bg-[#18181b] disabled:cursor-not-allowed disabled:opacity-50"
        >
          <RefreshCw className={`size-3.5 ${isLoading ? 'animate-spin' : ''}`} />
          Refresh
        </button>
      </div>

      {error && (
        <div className="mb-4 rounded-md border border-[#7f1d1d] bg-[#450a0a]/30 px-3 py-2 text-xs text-[#fca5a5]">
          Failed to load overview: {error}
        </div>
      )}

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 xl:grid-cols-4">
        {summaryCards.map((card) => {
          const Icon = card.icon;
          return (
            <div
              key={card.label}
              className="rounded-lg border border-[#27272a] bg-[#111113] p-3"
            >
              <div className="mb-2 flex items-center justify-between">
                <span className="text-xs text-[#71717a]">{card.label}</span>
                <span className={`rounded p-1 ${card.bgClass}`}>
                  <Icon className={`size-3.5 ${card.textClass}`} />
                </span>
              </div>
              <p className="text-lg font-semibold text-[#fafafa]">{card.value}</p>
            </div>
          );
        })}
      </div>

      <div className="mt-4 grid min-h-0 flex-1 grid-cols-1 gap-4 xl:grid-cols-2">
        <section className="flex min-h-0 flex-col rounded-lg border border-[#27272a] bg-[#111113]">
          <header className="border-b border-[#27272a] px-3 py-2">
            <h3 className="text-sm font-medium text-[#fafafa]">Recent Activity</h3>
          </header>
          <div className="min-h-0 flex-1 overflow-y-auto">
            {!overview || overview.recent_events.length === 0 ? (
              <div className="flex h-full items-center justify-center p-4 text-sm text-[#52525b]">
                {isLoading ? 'Loading events...' : 'No recent events'}
              </div>
            ) : (
              <ul className="divide-y divide-[#27272a]">
                {overview.recent_events.map((event, idx) => (
                  <li key={`${event.change_id}-${event.timestamp}-${idx}`} className="px-3 py-2.5">
                    <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs">
                      <span className="font-medium text-[#e4e4e7]">{getEventProjectLabel(event)}</span>
                      <span className="text-[#52525b]">/</span>
                      <span className="text-[#a1a1aa]">{event.change_id}</span>
                      <span className="rounded bg-[#18181b] px-1.5 py-0.5 text-[#c4b5fd]">
                        {event.operation}
                      </span>
                      <span
                        className={`rounded px-1.5 py-0.5 ${
                          event.result === 'success'
                            ? 'bg-[#052e16]/40 text-[#4ade80]'
                            : event.result === 'failure'
                              ? 'bg-[#450a0a]/40 text-[#f87171]'
                              : 'bg-[#18181b] text-[#a1a1aa]'
                        }`}
                      >
                        {event.result}
                      </span>
                    </div>
                    <p className="mt-1 text-[11px] text-[#71717a]">{formatTimestamp(event.timestamp)}</p>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </section>

        <section className="flex min-h-0 flex-col rounded-lg border border-[#27272a] bg-[#111113]">
          <header className="border-b border-[#27272a] px-3 py-2">
            <h3 className="text-sm font-medium text-[#fafafa]">Project Stats</h3>
          </header>
          <div className="min-h-0 flex-1 overflow-y-auto p-3">
            {!overview || overview.project_stats.length === 0 ? (
              <div className="flex h-full items-center justify-center text-sm text-[#52525b]">
                {isLoading ? 'Loading project stats...' : 'No project stats'}
              </div>
            ) : (
              <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                {overview.project_stats.map((stats) => (
                  <article
                    key={stats.project_id}
                    className="rounded-md border border-[#27272a] bg-[#09090b] p-3"
                  >
                    <h4 className="truncate text-sm font-medium text-[#e4e4e7]">
                      {getProjectLabel(stats)}
                    </h4>
                    <dl className="mt-2 space-y-1.5 text-xs">
                      <div className="flex items-center justify-between gap-2">
                        <dt className="text-[#71717a]">Apply Success</dt>
                        <dd className="font-medium text-[#22c55e]">{formatPercent(stats.apply_success_rate)}</dd>
                      </div>
                      <div className="flex items-center justify-between gap-2">
                        <dt className="text-[#71717a]">Avg Duration</dt>
                        <dd className="text-[#d4d4d8]">{formatDurationMs(stats.average_duration_ms)}</dd>
                      </div>
                      <div className="flex items-center justify-between gap-2">
                        <dt className="text-[#71717a]">Success / Failure / In Progress</dt>
                        <dd className="text-[#d4d4d8]">
                          {stats.success_count} / {stats.failure_count} / {stats.in_progress_count}
                        </dd>
                      </div>
                    </dl>
                  </article>
                ))}
              </div>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}
