/**
 * Table Renderer for eBPF Blockchain Monitor
 * Handles rendering of blocks table and block details
 */

class TableRenderer {
    constructor(filterManager) {
        this.filterManager = filterManager;
        this.blocks = [];
        this.filteredBlocks = [];
        this.tbody = document.getElementById('blocksTableBody');
    }

    /**
     * Add a new block to the data store
     */
    addBlock(block) {
        // Check if block already exists (by height or hash)
        const exists = this.blocks.find(b => 
            b.height === block.height || b.hash === block.hash
        );
        
        if (exists) {
            // Update existing block
            Object.assign(exists, block, { 
                updatedAt: Date.now(),
                timestampMs: block.timestampMs || Date.now() 
            });
        } else {
            // Add new block
            this.blocks.push({
                ...block,
                timestampMs: block.timestampMs || Date.now(),
                addedAt: Date.now()
            });
        }
        
        this.applyFilters();
    }

    /**
     * Apply current filters and re-render
     */
    applyFilters() {
        this.filteredBlocks = this.blocks.filter(block => 
            this.filterManager.matches(block)
        );
        
        // Sort by height descending (newest first)
        this.filteredBlocks.sort((a, b) => (b.height || 0) - (a.height || 0));
        
        this.render();
    }

    /**
     * Render the blocks table
     */
    render() {
        if (this.filteredBlocks.length === 0) {
            this.renderEmptyState();
            return;
        }

        this.tbody.innerHTML = this.filteredBlocks
            .map(block => this.renderRow(block))
            .join('');

        // Attach click handlers for detail buttons
        this.tbody.querySelectorAll('.action-btn').forEach(btn => {
            btn.addEventListener('click', (e) => {
                const height = parseInt(e.target.dataset.height);
                this.showBlockDetail(height);
            });
        });

        // Attach click handlers for hash copy
        this.tbody.querySelectorAll('.hash-value').forEach(el => {
            el.addEventListener('click', (e) => {
                navigator.clipboard.writeText(e.target.dataset.hash);
            });
        });
    }

    /**
     * Render a single table row
     */
    renderRow(block) {
        const statusClass = block.status || 'pending';
        const statusIcon = this.getStatusIcon(statusClass);
        const statusLabel = this.getStatusLabel(statusClass);
        
        const timestamp = this.formatTimestamp(block.timestamp, block.timestampMs);
        const hashShort = this.shortHash(block.hash);
        const proposerShort = this.shortProposer(block.proposer);
        const txCount = block.tx_count || block.transactions?.length || 0;
        
        const flags = this.renderFlags(block.flags || []);
        
        return `
            <tr class="block-row ${statusClass}" data-height="${block.height}">
                <td>
                    <span class="status-badge ${statusClass}">
                        ${statusIcon} ${statusLabel}
                    </span>
                </td>
                <td class="col-time">${timestamp}</td>
                <td><span class="height-badge">#${block.height}</span></td>
                <td>
                    <span class="hash-value" data-hash="${block.hash}" title="${block.hash}">
                        ${hashShort}
                    </span>
                </td>
                <td>
                    <span class="proposer-value" title="${block.proposer}">
                        ${proposerShort}
                    </span>
                </td>
                <td><span class="tx-count">${txCount}</span></td>
                <td>${flags || '<span style="color: var(--color-text-muted)">None</span>'}</td>
                <td>
                    <button class="action-btn" data-height="${block.height}" title="View Details">
                        🔍
                    </button>
                </td>
            </tr>
        `;
    }

    /**
     * Render flags for a block
     */
    renderFlags(flags) {
        if (!flags || flags.length === 0) return '';
        
        return flags.map(flag => {
            const label = this.getFlagLabel(flag);
            return `<span class="flag ${flag}">${label}</span>`;
        }).join('');
    }

    /**
     * Get flag display label
     */
    getFlagLabel(flag) {
        const labels = {
            replay: '🔄 Replay',
            doublespend: '💰 DoubleSpend',
            sybil: '👥 Sybil',
            ddos: '💣 DDoS',
        };
        return labels[flag] || flag;
    }

    /**
     * Get status icon
     */
    getStatusIcon(status) {
        const icons = {
            validated: '✅',
            pending: '⏳',
            rejected: '❌',
            suspicious: '🚨',
        };
        return icons[status] || '❓';
    }

    /**
     * Get status label
     */
    getStatusLabel(status) {
        const labels = {
            validated: 'Validated',
            pending: 'Pending',
            rejected: 'Rejected',
            suspicious: 'Suspicious',
        };
        return labels[status] || status;
    }

    /**
     * Format timestamp for display
     */
    formatTimestamp(timestamp, timestampMs) {
        const ms = timestampMs || (timestamp ? timestamp * 1000 : Date.now());
        const date = new Date(ms);
        return date.toLocaleTimeString('en-US', { 
            hour12: false,
            hour: '2-digit',
            minute: '2-digit',
            second: '2-digit'
        });
    }

    /**
     * Truncate hash for display
     */
    shortHash(hash) {
        if (!hash) return '';
        if (hash.length <= 14) return hash;
        return hash.substring(0, 8) + '...' + hash.substring(hash.length - 4);
    }

    /**
     * Truncate proposer ID for display
     */
    shortProposer(proposer) {
        if (!proposer) return '';
        if (proposer.length <= 16) return proposer;
        return proposer.substring(0, 8) + '...' + proposer.substring(proposer.length - 4);
    }

    /**
     * Render empty state
     */
    renderEmptyState() {
        this.tbody.innerHTML = `
            <tr class="empty-row">
                <td colspan="8">
                    <div class="empty-state">
                        <span class="empty-icon">📭</span>
                        <p>No blocks match the current filters.</p>
                    </div>
                </td>
            </tr>
        `;
    }

    /**
     * Show block detail modal
     */
    showBlockDetail(height) {
        const block = this.blocks.find(b => b.height === height);
        if (!block) return;

        const modal = document.getElementById('blockDetailModal');
        const title = document.getElementById('modalTitle');
        const body = document.getElementById('modalBody');

        title.textContent = `Block #${block.height}`;
        
        body.innerHTML = `
            <div class="detail-grid">
                <div class="detail-item">
                    <span class="detail-label">Height</span>
                    <span class="detail-value">${block.height}</span>
                </div>
                <div class="detail-item">
                    <span class="detail-label">Status</span>
                    <span class="detail-value">
                        <span class="status-badge ${block.status}">
                            ${this.getStatusIcon(block.status)} ${this.getStatusLabel(block.status)}
                        </span>
                    </span>
                </div>
                <div class="detail-item">
                    <span class="detail-label">Hash</span>
                    <span class="detail-value hash-value" data-hash="${block.hash}">${block.hash}</span>
                </div>
                <div class="detail-item">
                    <span class="detail-label">Parent Hash</span>
                    <span class="detail-value">${block.parent_hash || 'N/A'}</span>
                </div>
                <div class="detail-item">
                    <span class="detail-label">Proposer</span>
                    <span class="detail-value proposer-value">${block.proposer}</span>
                </div>
                <div class="detail-item">
                    <span class="detail-label">Timestamp</span>
                    <span class="detail-value">${new Date(block.timestampMs || block.timestamp * 1000).toISOString()}</span>
                </div>
                <div class="detail-item">
                    <span class="detail-label">Transaction Count</span>
                    <span class="detail-value">${block.tx_count || block.transactions?.length || 0}</span>
                </div>
                <div class="detail-item">
                    <span class="detail-label">Quorum Votes</span>
                    <span class="detail-value">${block.quorum_votes || 'N/A'}</span>
                </div>
                <div class="detail-item">
                    <span class="detail-label">Total Validators</span>
                    <span class="detail-value">${block.total_validators || 'N/A'}</span>
                </div>
                <div class="detail-item">
                    <span class="detail-label">Flags</span>
                    <span class="detail-value">${this.renderFlags(block.flags) || 'None'}</span>
                </div>
                ${block.reason ? `
                <div class="detail-item" style="grid-column: span 2">
                    <span class="detail-label">Rejection Reason</span>
                    <span class="detail-value" style="color: var(--color-danger)">${block.reason}</span>
                </div>
                ` : ''}
                ${block.transactions && block.transactions.length > 0 ? `
                <div class="detail-item" style="grid-column: span 2">
                    <span class="detail-label">Transactions</span>
                    <span class="detail-value" style="max-height: 200px; overflow-y: auto;">
                        ${block.transactions.map(tx => `<div style="margin-top: 4px;">${this.shortHash(tx)}</div>`).join('')}
                    </span>
                </div>
                ` : ''}
            </div>
        `;

        modal.classList.add('active');
    }

    /**
     * Hide block detail modal
     */
    hideBlockDetail() {
        const modal = document.getElementById('blockDetailModal');
        modal.classList.remove('active');
    }

    /**
     * Update block status
     */
    updateBlockStatus(height, newStatus, reason = null) {
        const block = this.blocks.find(b => b.height === height);
        if (block) {
            block.status = newStatus;
            block.updatedAt = Date.now();
            if (reason) {
                block.reason = reason;
            }
            this.applyFilters();
        }
    }

    /**
     * Add flags to a block
     */
    addFlagsToBlock(height, flags) {
        const block = this.blocks.find(b => b.height === height);
        if (block) {
            if (!block.flags) {
                block.flags = [];
            }
            flags.forEach(flag => {
                if (!block.flags.includes(flag)) {
                    block.flags.push(flag);
                }
            });
            // If flags added, mark as suspicious
            if (block.flags.length > 0 && block.status === 'pending') {
                block.status = 'suspicious';
            }
            block.updatedAt = Date.now();
            this.applyFilters();
        }
    }
}

// Export for use in other modules
if (typeof module !== 'undefined' && module.exports) {
    module.exports = TableRenderer;
}
