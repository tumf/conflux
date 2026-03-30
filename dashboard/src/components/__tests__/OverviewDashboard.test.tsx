// @vitest-environment jsdom

import React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';

import { OverviewDashboard } from '../OverviewDashboard';

const fetchStatsOverviewMock = vi.fn();

vi.mock('../../api/restClient', () => ({
  fetchStatsOverview: (...args: unknown[]) => fetchStatsOverviewMock(...args),
}));

afterEach(() => {
  cleanup();
  fetchStatsOverviewMock.mockReset();
});

describe('OverviewDashboard', () => {
  it('renders summary cards and recent events from stats overview response', async () => {
    fetchStatsOverviewMock.mockResolvedValue({
      summary: {
        success_count: 7,
        failure_count: 2,
        in_progress_count: 1,
        average_duration_ms: 1234,
      },
      recent_events: [
        {
          project_id: 'project-1',
          project_name: 'Demo Project',
          change_id: 'change-1',
          operation: 'apply',
          result: 'success',
          timestamp: '2026-03-30T00:00:00.000Z',
        },
      ],
      project_stats: [
        {
          project_id: 'project-1',
          project_name: 'Demo Project',
          apply_success_rate: 0.9,
          average_duration_ms: 2000,
          success_count: 9,
          failure_count: 1,
          in_progress_count: 0,
        },
      ],
    });

    render(<OverviewDashboard />);

    await waitFor(() => {
      expect(fetchStatsOverviewMock).toHaveBeenCalledTimes(1);
    });

    expect(screen.getByText('Success')).toBeTruthy();
    expect(screen.getByText('Failure')).toBeTruthy();
    expect(screen.getByText('In Progress')).toBeTruthy();
    expect(screen.getAllByText('Avg Duration').length).toBeGreaterThan(0);

    expect(screen.getByText('7')).toBeTruthy();
    expect(screen.getByText('2')).toBeTruthy();
    expect(screen.getByText('1')).toBeTruthy();
    expect(screen.getByText('1.2s')).toBeTruthy();

    expect(screen.getAllByText('Demo Project').length).toBeGreaterThan(0);
    expect(screen.getByText('change-1')).toBeTruthy();
    expect(screen.getByText('apply')).toBeTruthy();
    expect(screen.getAllByText('success').length).toBeGreaterThan(0);
  });

  it('renders fallback UI when recent_events or project_stats are missing', async () => {
    fetchStatsOverviewMock.mockResolvedValue({
      summary: {
        success_count: 1,
        failure_count: 0,
        in_progress_count: 0,
        average_duration_ms: null,
      },
      recent_events: undefined,
      project_stats: undefined,
    });

    render(<OverviewDashboard />);

    await waitFor(() => {
      expect(fetchStatsOverviewMock).toHaveBeenCalledTimes(1);
    });

    expect(screen.getByText('No recent events')).toBeTruthy();
    expect(screen.getByText('No project stats')).toBeTruthy();
  });
});
