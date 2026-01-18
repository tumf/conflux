// Conflux Web Monitor - Client Application
// Responsive mobile-first implementation with touch support

class WebMonitor {
    constructor() {
        this.ws = null;
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 10;
        this.reconnectDelay = 1000;
        this.pollIntervalId = null;
        this.pollIntervalMs = 5000;
        this.isMobile = window.matchMedia('(max-width: 767px)').matches;
        this.previousConnectionStatus = null;

        this.elements = {
            connectionStatus: document.getElementById('connection-status'),
            statusText: document.querySelector('.status-text'),
            totalChanges: document.getElementById('total-changes'),
            completedChanges: document.getElementById('completed-changes'),
            inProgressChanges: document.getElementById('in-progress-changes'),
            pendingChanges: document.getElementById('pending-changes'),
            loading: document.getElementById('loading'),
            changesList: document.getElementById('changes-list'),
            emptyState: document.getElementById('empty-state'),
            lastUpdated: document.getElementById('last-updated'),
            toastContainer: document.getElementById('toast-container'),
            ptrIndicator: document.getElementById('ptr-indicator'),
            ptrText: document.querySelector('.ptr-text'),
            overallProgressFill: document.getElementById('overall-progress-fill'),
            overallProgressTasks: document.getElementById('overall-progress-tasks'),
            overallProgressPercent: document.getElementById('overall-progress-percent'),
        };

        this.touchState = {
            startX: 0,
            startY: 0,
            currentX: 0,
            currentY: 0,
            isDragging: false,
            element: null,
        };

        this.ptrState = {
            startY: 0,
            isPulling: false,
            isRefreshing: false,
            threshold: 80,
        };

        this.setupMediaQueryListener();
        this.setupTouchHandlers();
        this.setupPullToRefresh();
        this.fetchState();
        this.connect();
    }

    setupMediaQueryListener() {
        const mq = window.matchMedia('(max-width: 767px)');
        mq.addEventListener('change', (e) => {
            this.isMobile = e.matches;
        });
    }

    setupTouchHandlers() {
        this.elements.changesList.addEventListener('touchstart', this.handleTouchStart.bind(this), { passive: true });
        this.elements.changesList.addEventListener('touchmove', this.handleTouchMove.bind(this), { passive: true });
        this.elements.changesList.addEventListener('touchend', this.handleTouchEnd.bind(this));
        this.elements.changesList.addEventListener('click', this.handleCardClick.bind(this));
        this.elements.changesList.addEventListener('click', this.handleApprovalClick.bind(this));
    }

    setupPullToRefresh() {
        document.addEventListener('touchstart', this.handlePtrStart.bind(this), { passive: true });
        document.addEventListener('touchmove', this.handlePtrMove.bind(this), { passive: false });
        document.addEventListener('touchend', this.handlePtrEnd.bind(this));
    }

    handlePtrStart(e) {
        if (!this.isMobile || this.ptrState.isRefreshing) return;
        if (window.scrollY > 0) return;

        this.ptrState.startY = e.touches[0].clientY;
        this.ptrState.isPulling = true;
    }

    handlePtrMove(e) {
        if (!this.ptrState.isPulling || this.ptrState.isRefreshing) return;

        const currentY = e.touches[0].clientY;
        const pullDistance = currentY - this.ptrState.startY;

        if (pullDistance > 0 && window.scrollY === 0) {
            e.preventDefault();

            const indicator = this.elements.ptrIndicator;
            const progress = Math.min(pullDistance / this.ptrState.threshold, 1);

            if (pullDistance > 10) {
                indicator.classList.add('visible');
            }

            if (pullDistance >= this.ptrState.threshold) {
                this.elements.ptrText.textContent = 'Release to refresh';
            } else {
                this.elements.ptrText.textContent = 'Pull to refresh';
            }

            const icon = indicator.querySelector('.ptr-icon');
            icon.style.transform = `rotate(${progress * 180}deg)`;
        }
    }

    handlePtrEnd() {
        if (!this.ptrState.isPulling || this.ptrState.isRefreshing) return;

        const indicator = this.elements.ptrIndicator;
        const currentY = this.ptrState.startY;

        this.ptrState.isPulling = false;

        const touchEndY = event.changedTouches?.[0]?.clientY || currentY;
        const pullDistance = touchEndY - this.ptrState.startY;

        if (pullDistance >= this.ptrState.threshold) {
            this.triggerRefresh();
        } else {
            indicator.classList.remove('visible');
            this.elements.ptrText.textContent = 'Pull to refresh';
            const icon = indicator.querySelector('.ptr-icon');
            icon.style.transform = '';
        }
    }

    triggerRefresh() {
        this.ptrState.isRefreshing = true;
        const indicator = this.elements.ptrIndicator;
        indicator.classList.add('refreshing');
        this.elements.ptrText.textContent = 'Refreshing...';

        const icon = indicator.querySelector('.ptr-icon');
        icon.style.transform = '';

        // Request fresh data from server
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            this.fetchState();
            this.showToast('Refreshing data...', 'info');
        } else {
            this.showToast('Not connected. Reconnecting...', 'warning');
            this.connect();
        }

        // Reset after a delay
        setTimeout(() => {
            indicator.classList.remove('visible', 'refreshing');
            this.elements.ptrText.textContent = 'Pull to refresh';
            this.ptrState.isRefreshing = false;
        }, 1500);
    }

    handleTouchStart(e) {
        const card = e.target.closest('.change-card');
        if (!card) return;

        const touch = e.touches[0];
        this.touchState = {
            startX: touch.clientX,
            startY: touch.clientY,
            currentX: touch.clientX,
            currentY: touch.clientY,
            isDragging: true,
            element: card,
        };
    }

    handleTouchMove(e) {
        if (!this.touchState.isDragging) return;

        const touch = e.touches[0];
        this.touchState.currentX = touch.clientX;
        this.touchState.currentY = touch.clientY;

        const deltaX = this.touchState.currentX - this.touchState.startX;
        const deltaY = this.touchState.currentY - this.touchState.startY;

        // Only trigger horizontal swipe if movement is primarily horizontal
        if (Math.abs(deltaX) > Math.abs(deltaY) && Math.abs(deltaX) > 10) {
            this.touchState.element.classList.add('swiping');
        }
    }

    handleTouchEnd() {
        if (!this.touchState.isDragging) return;

        const deltaX = this.touchState.currentX - this.touchState.startX;
        const deltaY = this.touchState.currentY - this.touchState.startY;
        const card = this.touchState.element;

        card.classList.remove('swiping');

        // Swipe right to expand, left to collapse
        if (Math.abs(deltaX) > 50 && Math.abs(deltaX) > Math.abs(deltaY)) {
            if (deltaX > 0) {
                card.classList.add('expanded');
            } else {
                card.classList.remove('expanded');
            }
        }

        this.touchState = {
            startX: 0,
            startY: 0,
            currentX: 0,
            currentY: 0,
            isDragging: false,
            element: null,
        };
    }

    handleCardClick(e) {
        const card = e.target.closest('.change-card');
        if (!card || !this.isMobile) return;

        // Don't toggle if clicking on an interactive element
        if (e.target.closest('a, button')) return;

        card.classList.toggle('expanded');
    }

    handleApprovalClick(e) {
        const approvalBtn = e.target.closest('.approval-button');
        if (!approvalBtn) return;

        e.preventDefault();
        e.stopPropagation();

        const changeId = approvalBtn.dataset.changeId;
        const isApproved = approvalBtn.dataset.approved === 'true';

        this.toggleApproval(changeId, isApproved);
    }

    async toggleApproval(changeId, isCurrentlyApproved) {
        const endpoint = isCurrentlyApproved ? 'unapprove' : 'approve';
        const url = `/api/changes/${encodeURIComponent(changeId)}/${endpoint}`;

        try {
            const response = await fetch(url, {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                },
            });

            if (!response.ok) {
                const error = await response.json();
                throw new Error(error.error || 'Failed to update approval status');
            }

            const updatedChange = await response.json();

            // Update the UI immediately
            this.updateChangeInUI(updatedChange);

            const action = isCurrentlyApproved ? 'unapproved' : 'approved';
            this.showToast(`Change ${changeId} ${action}`, 'success');
        } catch (error) {
            console.error('Failed to toggle approval:', error);
            this.showToast(`Error: ${error.message}`, 'error');
        }
    }

    updateChangeInUI(change) {
        const card = document.querySelector(`[data-change-id="${this.escapeHtml(change.id)}"]`);
        if (!card) return;

        // Update approval badge
        const badge = card.querySelector('.badge-approval');
        if (badge) {
            badge.className = change.is_approved ? 'badge badge-approved badge-approval' : 'badge badge-unapproved badge-approval';
            badge.textContent = change.is_approved ? 'Approved' : 'Pending Approval';
        }

        // Update approval button
        const button = card.querySelector('.approval-button');
        if (button) {
            button.dataset.approved = change.is_approved;
            button.textContent = change.is_approved ? '✓ Approved' : '○ Approve';
            button.className = change.is_approved ? 'approval-button approved' : 'approval-button';
        }
    }

    async fetchState() {
        try {
            const response = await fetch('/api/state', { cache: 'no-store' });
            if (!response.ok) {
                throw new Error(`Failed to fetch state: ${response.status}`);
            }

            const state = await response.json();
            this.renderFullState(state);
        } catch (error) {
            console.error('Failed to fetch state:', error);
            this.showToast('Failed to refresh state', 'error');
        }
    }

    startPolling() {
        if (this.pollIntervalId) return;

        this.pollIntervalId = setInterval(() => {
            this.fetchState();
        }, this.pollIntervalMs);

        this.fetchState();
    }

    stopPolling() {
        if (!this.pollIntervalId) return;

        clearInterval(this.pollIntervalId);
        this.pollIntervalId = null;
    }

    connect() {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${protocol}//${window.location.host}/ws`;

        this.updateConnectionStatus('connecting');

        try {
            this.ws = new WebSocket(wsUrl);

            this.ws.onopen = () => {
                console.log('WebSocket connected');
                this.reconnectAttempts = 0;
                this.updateConnectionStatus('connected');
            };

            this.ws.onmessage = (event) => {
                try {
                    const data = JSON.parse(event.data);
                    this.handleMessage(data);
                } catch (e) {
                    console.error('Failed to parse message:', e);
                }
            };

            this.ws.onclose = () => {
                console.log('WebSocket disconnected');
                this.updateConnectionStatus('disconnected');
                this.scheduleReconnect();
            };

            this.ws.onerror = (error) => {
                console.error('WebSocket error:', error);
                this.updateConnectionStatus('disconnected');
            };
        } catch (e) {
            console.error('Failed to create WebSocket:', e);
            this.updateConnectionStatus('disconnected');
            this.scheduleReconnect();
        }
    }

    scheduleReconnect() {
        if (this.reconnectAttempts >= this.maxReconnectAttempts) {
            console.log('Max reconnect attempts reached');
            this.elements.statusText.textContent = 'Connection failed';
            this.showToast('Connection failed. Please refresh the page.', 'error');
            return;
        }

        this.reconnectAttempts++;
        const delay = this.reconnectDelay * Math.pow(1.5, this.reconnectAttempts - 1);

        console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);
        this.elements.statusText.textContent = `Reconnecting (${this.reconnectAttempts})...`;

        setTimeout(() => this.connect(), delay);
    }

    updateConnectionStatus(status) {
        const el = this.elements.connectionStatus;
        el.classList.remove('connected', 'disconnected');

        // Show toast on status change
        if (this.previousConnectionStatus !== null && this.previousConnectionStatus !== status) {
            if (status === 'connected') {
                this.showToast('Connected to server', 'success');
            } else if (status === 'disconnected') {
                this.showToast('Disconnected from server', 'warning');
            }
        }
        this.previousConnectionStatus = status;

        switch (status) {
            case 'connected':
                el.classList.add('connected');
                this.elements.statusText.textContent = 'Connected';
                this.stopPolling();
                break;
            case 'disconnected':
                el.classList.add('disconnected');
                this.elements.statusText.textContent = 'Disconnected';
                this.startPolling();
                break;
            default:
                this.elements.statusText.textContent = 'Connecting...';
        }
    }

    showToast(message, type = 'info') {
        const toast = document.createElement('div');
        toast.className = `toast ${type}`;
        toast.textContent = message;
        toast.setAttribute('role', 'alert');

        this.elements.toastContainer.appendChild(toast);

        // Auto-remove after 4 seconds
        setTimeout(() => {
            toast.style.animation = 'slideOut 0.3s ease-out forwards';
            setTimeout(() => toast.remove(), 300);
        }, 4000);
    }

    handleMessage(data) {
        switch (data.type) {
            case 'initial_state':
                this.renderFullState(data.state);
                break;
            case 'state_update':
                this.renderChanges(data.changes);
                this.updateTimestamp(data.timestamp);
                break;
            default:
                console.log('Unknown message type:', data.type);
        }
    }

    renderFullState(state) {
        // Hide loading, show content
        this.elements.loading.style.display = 'none';

        // Update stats
        this.elements.totalChanges.textContent = state.total_changes;
        this.elements.completedChanges.textContent = state.completed_changes;
        this.elements.inProgressChanges.textContent = state.in_progress_changes;
        this.elements.pendingChanges.textContent = state.pending_changes;

        // Update overall progress
        this.updateOverallProgress(state.changes);

        // Render changes
        this.renderChanges(state.changes);

        // Update timestamp
        this.updateTimestamp(state.last_updated);
    }

    renderChanges(changes) {
        if (!changes || changes.length === 0) {
            this.elements.changesList.innerHTML = '';
            this.elements.emptyState.style.display = 'block';
            return;
        }

        this.elements.emptyState.style.display = 'none';

        // Update stats from changes
        const stats = this.calculateStats(changes);
        this.elements.totalChanges.textContent = stats.total;
        this.elements.completedChanges.textContent = stats.completed;
        this.elements.inProgressChanges.textContent = stats.inProgress;
        this.elements.pendingChanges.textContent = stats.pending;

        // Update overall progress
        this.updateOverallProgress(changes);

        // Render change cards
        this.elements.changesList.innerHTML = changes.map(change =>
            this.renderChangeCard(change)
        ).join('');
    }

    updateOverallProgress(changes) {
        if (!changes || changes.length === 0) {
            this.elements.overallProgressFill.style.width = '0%';
            this.elements.overallProgressTasks.textContent = '0 / 0 tasks';
            this.elements.overallProgressPercent.textContent = '0%';
            return;
        }

        // Calculate total tasks across all changes
        const totalCompleted = changes.reduce((sum, c) => sum + c.completed_tasks, 0);
        const totalTasks = changes.reduce((sum, c) => sum + c.total_tasks, 0);
        const overallPercent = totalTasks > 0 ? (totalCompleted / totalTasks) * 100 : 0;

        this.elements.overallProgressFill.style.width = `${overallPercent.toFixed(1)}%`;
        this.elements.overallProgressTasks.textContent = `${totalCompleted} / ${totalTasks} tasks`;
        this.elements.overallProgressPercent.textContent = `${overallPercent.toFixed(1)}%`;

        // Update aria attributes
        const progressBar = this.elements.overallProgressFill.parentElement;
        progressBar.setAttribute('aria-valuenow', overallPercent.toFixed(1));
    }

    calculateStats(changes) {
        return {
            total: changes.length,
            completed: changes.filter(c => c.status === 'complete').length,
            inProgress: changes.filter(c => c.status === 'in_progress').length,
            pending: changes.filter(c => c.status === 'pending').length,
        };
    }

    renderChangeCard(change) {
        const progressPercent = change.progress_percent.toFixed(1);
        const isComplete = change.status === 'complete';

        // Use queue_status if available, otherwise fall back to status
        const displayStatus = change.queue_status || change.status.replace('_', ' ');
        const statusClass = change.queue_status ? change.queue_status.replace(' ', '-') : change.status;

        // Status icons mapping
        const statusIcons = {
            'not queued': '○',
            'queued': '⏳',
            'processing': '⚙️',
            'completed': '✅',
            'archiving': '📦',
            'archived': '📥',
            'merged': '🔀',
            'merge wait': '⏸️',
            'resolving': '🔧',
            'error': '❌',
            'pending': '○',
            'in_progress': '⚙️',
            'complete': '✅',
        };
        const statusIcon = statusIcons[displayStatus] || '•';

        // Show iteration number if > 0
        const iterationHtml = change.iteration_number && change.iteration_number > 0
            ? `<span class="change-iteration">Iteration: ${change.iteration_number}</span>`
            : '';

        const dependenciesHtml = change.dependencies && change.dependencies.length > 0
            ? `<div class="change-dependencies">
                <div class="dependencies-label">Dependencies:</div>
                <div class="dependencies-list">
                    ${change.dependencies.map(dep =>
                        `<span class="dependency-tag">${this.escapeHtml(dep)}</span>`
                    ).join('')}
                </div>
               </div>`
            : '';

        const approvalButtonHtml = `
            <div class="approval-actions">
                <button
                    class="approval-button ${change.is_approved ? 'approved' : ''}"
                    data-change-id="${this.escapeHtml(change.id)}"
                    data-approved="${change.is_approved}"
                    aria-label="${change.is_approved ? 'Unapprove change' : 'Approve change'}">
                    ${change.is_approved ? '✓ Approved' : '○ Approve'}
                </button>
            </div>
        `;

        const expandHintHtml = this.isMobile
            ? `<div class="expand-hint">
                <span class="expand-hint-icon" aria-hidden="true">▼</span>
                <span>Tap or swipe to expand</span>
               </div>`
            : '';

        return `
            <article class="change-card" data-change-id="${this.escapeHtml(change.id)}" role="listitem" tabindex="0">
                <div class="change-header">
                    <span class="change-id">${this.escapeHtml(change.id)}</span>
                    <div class="change-status-row">
                        <span class="badge badge-status ${statusClass}">
                            <span class="status-icon" aria-hidden="true">${statusIcon}</span>
                            ${displayStatus}
                        </span>
                        ${iterationHtml}
                    </div>
                </div>
                <div class="progress-container">
                    <div class="progress-bar" role="progressbar" aria-valuenow="${progressPercent}" aria-valuemin="0" aria-valuemax="100">
                        <div class="progress-fill ${isComplete ? 'complete' : ''}"
                             style="width: ${progressPercent}%"></div>
                    </div>
                    <div class="progress-text">
                        <span>${change.completed_tasks} / ${change.total_tasks} tasks</span>
                        <span>${progressPercent}%</span>
                    </div>
                </div>
                <div class="change-details">
                    <div class="approval-section">
                        <span class="badge ${change.is_approved ? 'badge-approved' : 'badge-unapproved'} badge-approval">
                            ${change.is_approved ? 'Approved' : 'Pending Approval'}
                        </span>
                        ${approvalButtonHtml}
                    </div>
                    ${dependenciesHtml}
                </div>
                ${expandHintHtml}
            </article>
        `;
    }

    updateTimestamp(timestamp) {
        if (!timestamp) return;

        const date = new Date(timestamp);
        const formatted = date.toLocaleString();
        this.elements.lastUpdated.textContent = `Last updated: ${formatted}`;
    }

    escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }
}

// Throttle utility for performance optimization
function throttle(func, limit) {
    let inThrottle;
    return function(...args) {
        if (!inThrottle) {
            func.apply(this, args);
            inThrottle = true;
            setTimeout(() => inThrottle = false, limit);
        }
    };
}

// Debounce utility for performance optimization
function debounce(func, wait) {
    let timeout;
    return function(...args) {
        clearTimeout(timeout);
        timeout = setTimeout(() => func.apply(this, args), wait);
    };
}

// Initialize on page load
document.addEventListener('DOMContentLoaded', () => {
    window.monitor = new WebMonitor();
});
