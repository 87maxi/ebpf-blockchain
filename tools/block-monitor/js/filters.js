/**
 * Filter Manager for eBPF Blockchain Monitor
 * Handles search, status filtering, type filtering, and time-based filtering
 */

class FilterManager {
    constructor() {
        this.filters = {
            search: '',
            status: 'all',
            type: 'all',
            timeRange: 'all',
        };
        this.listeners = [];
    }

    /**
     * Set a filter value
     */
    setFilter(key, value) {
        this.filters[key] = value;
        this.notify();
    }

    /**
     * Get current filters
     */
    getFilters() {
        return { ...this.filters };
    }

    /**
     * Reset all filters
     */
    reset() {
        this.filters = {
            search: '',
            status: 'all',
            type: 'all',
            timeRange: 'all',
        };
        this.notify();
    }

    /**
     * Register a listener for filter changes
     */
    onChange(callback) {
        this.listeners.push(callback);
    }

    /**
     * Notify all listeners of filter changes
     */
    notify() {
        this.listeners.forEach(cb => {
            try {
                cb(this.filters);
            } catch (e) {
                console.error('Error in filter listener:', e);
            }
        });
    }

    /**
     * Check if a block matches the current filters
     */
    matches(block) {
        // Search filter
        if (this.filters.search) {
            const search = this.filters.search.toLowerCase();
            const searchable = [
                block.hash,
                block.proposer,
                block.height?.toString(),
                block.flags?.join(''),
            ].filter(Boolean).join(' ').toLowerCase();
            
            if (!searchable.includes(search)) {
                return false;
            }
        }

        // Status filter
        if (this.filters.status !== 'all') {
            if (block.status !== this.filters.status) {
                return false;
            }
        }

        // Type filter (flags)
        if (this.filters.type !== 'all') {
            const flags = block.flags || [];
            if (!flags.includes(this.filters.type)) {
                return false;
            }
        }

        // Time range filter
        if (this.filters.timeRange !== 'all') {
            const seconds = parseInt(this.filters.timeRange);
            const cutoff = Date.now() - (seconds * 1000);
            const blockTime = block.timestamp ? block.timestamp * 1000 : block.timestampMs;
            
            if (!blockTime || blockTime < cutoff) {
                return false;
            }
        }

        return true;
    }
}

// Export for use in other modules
if (typeof module !== 'undefined' && module.exports) {
    module.exports = FilterManager;
}
