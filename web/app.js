// OpenSpec Orchestrator Web Monitor - Client Application

class WebMonitor {
    constructor() {
        this.ws = null;
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 10;
        this.reconnectDelay = 1000;

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
        };

        this.connect();
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

        switch (status) {
            case 'connected':
                el.classList.add('connected');
                this.elements.statusText.textContent = 'Connected';
                break;
            case 'disconnected':
                el.classList.add('disconnected');
                this.elements.statusText.textContent = 'Disconnected';
                break;
            default:
                this.elements.statusText.textContent = 'Connecting...';
        }
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

        // Render change cards
        this.elements.changesList.innerHTML = changes.map(change =>
            this.renderChangeCard(change)
        ).join('');
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

        return `
            <div class="change-card" data-change-id="${this.escapeHtml(change.id)}">
                <div class="change-header">
                    <span class="change-id">${this.escapeHtml(change.id)}</span>
                    <div class="change-badges">
                        <span class="badge badge-status ${change.status}">${change.status.replace('_', ' ')}</span>
                        <span class="badge ${change.is_approved ? 'badge-approved' : 'badge-unapproved'}">
                            ${change.is_approved ? 'Approved' : 'Pending Approval'}
                        </span>
                    </div>
                </div>
                <div class="progress-container">
                    <div class="progress-bar">
                        <div class="progress-fill ${isComplete ? 'complete' : ''}"
                             style="width: ${progressPercent}%"></div>
                    </div>
                    <div class="progress-text">
                        <span>${change.completed_tasks} / ${change.total_tasks} tasks</span>
                        <span>${progressPercent}%</span>
                    </div>
                </div>
                ${dependenciesHtml}
            </div>
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

// Initialize on page load
document.addEventListener('DOMContentLoaded', () => {
    window.monitor = new WebMonitor();
});
