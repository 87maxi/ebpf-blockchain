/**
 * Main Application for eBPF Blockchain Monitor
 * Initializes components and wires up event handlers
 */

// Global state
let wsClient = null;
let filterManager = null;
let tableRenderer = null;
let eventLog = [];
const MAX_LOG_ENTRIES = 100;

// Statistics
let stats = {
    totalBlocks: 0,
    suspiciousBlocks: 0,
    rejectedBlocks: 0,
    validatedBlocks: 0,
    totalTransactions: 0,
    latestHeight: 0,
    txTimestamps: [],
};

/**
 * Initialize the application
 */
document.addEventListener('DOMContentLoaded', () => {
    // Initialize components
    filterManager = new FilterManager();
    wsClient = new WebSocketClient();
    tableRenderer = new TableRenderer(filterManager);
    
    // Setup event listeners
    setupUIListeners();
    setupWebSocketListeners();
    setupFilterListeners();
    
    // Initial render
    tableRenderer.render();
    updateDashboard();
    
    console.log('eBPF Blockchain Monitor initialized');
});

/**
 * Setup UI event listeners
 */
function setupUIListeners() {
    // Connect button
    document.getElementById('connectBtn').addEventListener('click', () => {
        const url = document.getElementById('nodeUrl').value.trim();
        const apiUrl = document.getElementById('apiUrl').value.trim();
        
        if (!url) {
            addLogEntry('Please enter a WebSocket URL', 'warning');
            return;
        }
        
        setConnectionStatus('connecting');
        wsClient.connect(url, apiUrl);
    });

    // Disconnect button
    document.getElementById('disconnectBtn').addEventListener('click', () => {
        wsClient.disconnect();
        setConnectionStatus('disconnected');
        addLogEntry('Disconnected from node', 'info');
    });

    // Search
    document.getElementById('searchBtn').addEventListener('click', applySearch);
    document.getElementById('searchInput').addEventListener('keypress', (e) => {
        if (e.key === 'Enter') applySearch();
    });
    document.getElementById('clearSearchBtn').addEventListener('click', clearSearch);

    // Filters
    document.getElementById('applyFiltersBtn').addEventListener('click', applyFilters);

    // Clear log
    document.getElementById('clearLogBtn').addEventListener('click', clearLog);

    // Modal close
    document.getElementById('modalClose').addEventListener('click', () => {
        tableRenderer.hideBlockDetail();
    });
    document.getElementById('blockDetailModal').addEventListener('click', (e) => {
        if (e.target.id === 'blockDetailModal') {
            tableRenderer.hideBlockDetail();
        }
    });

    // Keyboard shortcuts
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape') {
            tableRenderer.hideBlockDetail();
        }
    });
}

/**
 * Setup WebSocket event listeners
 */
function setupWebSocketListeners() {
    wsClient.on('onConnect', (data) => {
        setConnectionStatus('connected');
        document.getElementById('connectBtn').disabled = true;
        document.getElementById('disconnectBtn').disabled = false;
        addLogEntry(`Connected to ${data.url}`, 'success');
    });

    wsClient.on('onDisconnect', (data) => {
        setConnectionStatus('disconnected');
        document.getElementById('connectBtn').disabled = false;
        document.getElementById('disconnectBtn').disabled = true;
        addLogEntry(`Disconnected: ${data.reason || 'Unknown'}`, 'error');
    });

    wsClient.on('onBlockCreated', (data) => {
        const block = {
            height: data.height,
            hash: data.hash,
            proposer: data.proposer,
            timestamp: data.timestamp || Math.floor(Date.now() / 1000),
            tx_count: data.tx_count || 0,
            status: 'pending',
            flags: [],
            parent_hash: data.parent_hash,
            quorum_votes: data.quorum_votes,
            total_validators: data.total_validators,
            transactions: data.transactions || [],
        };
        
        tableRenderer.addBlock(block);
        updateStats(block);
        addLogEntry(`Block #${block.height} created by ${block.proposer} (${block.tx_count} txs)`, 'info');
    });

    wsClient.on('onBlockConfirmed', (data) => {
        tableRenderer.updateBlockStatus(data.height, 'validated');
        stats.validatedBlocks++;
        stats.suspiciousBlocks = Math.max(0, stats.suspiciousBlocks - 
            (tableRenderer.blocks.find(b => b.height === data.height)?.flags?.length > 0 ? 1 : 0));
        updateDashboard();
        addLogEntry(`Block #${data.height} confirmed (${data.voters} voters)`, 'success');
    });

    wsClient.on('onBlockRejected', (data) => {
        tableRenderer.updateBlockStatus(data.height, 'rejected', data.reason);
        stats.rejectedBlocks++;
        updateDashboard();
        addLogEntry(`Block #${data.height} rejected: ${data.reason}`, 'error');
    });

    wsClient.on('onSecurityAlert', (data) => {
        const alertType = data.type?.toLowerCase() || 'unknown';
        const flags = mapAlertTypeToFlag(alertType);
        
        if (data.height) {
            tableRenderer.addFlagsToBlock(data.height, flags);
            stats.suspiciousBlocks++;
            updateDashboard();
        }
        
        addLogEntry(`Security Alert: ${alertType} from ${data.source} - ${data.action}`, 'warning');
    });

    wsClient.on('onTxProcessed', (data) => {
        stats.totalTransactions++;
        stats.txTimestamps.push(Date.now());
        
        // Keep only last 60 seconds of timestamps for rate calculation
        const sixtySecondsAgo = Date.now() - 60000;
        stats.txTimestamps = stats.txTimestamps.filter(t => t > sixtySecondsAgo);
        
        updateDashboard();
    });

    wsClient.on('onError', (data) => {
        addLogEntry(`Error: ${data.error}`, 'error');
    });
}

/**
 * Setup filter change listeners
 */
function setupFilterListeners() {
    filterManager.onChange(() => {
        tableRenderer.applyFilters();
    });
}

/**
 * Apply search from input
 */
function applySearch() {
    const search = document.getElementById('searchInput').value.trim();
    filterManager.setFilter('search', search);
}

/**
 * Clear search
 */
function clearSearch() {
    document.getElementById('searchInput').value = '';
    filterManager.setFilter('search', '');
}

/**
 * Apply all filters
 */
function applyFilters() {
    const status = document.getElementById('statusFilter').value;
    const type = document.getElementById('typeFilter').value;
    const timeRange = document.getElementById('timeFilter').value;
    const search = document.getElementById('searchInput').value.trim();
    
    filterManager.setFilter('status', status);
    filterManager.setFilter('type', type);
    filterManager.setFilter('timeRange', timeRange);
    filterManager.setFilter('search', search);
}

/**
 * Map alert type to flag
 */
function mapAlertTypeToFlag(alertType) {
    const mapping = {
        'replay': ['replay'],
        'replay_attack': ['replay'],
        'doublespend': ['doublespend'],
        'double_spend': ['doublespend'],
        'sybil': ['sybil'],
        'ddos': ['ddos'],
        'ddos_attack': ['ddos'],
        'flood': ['ddos'],
    };
    return mapping[alertType] || [alertType];
}

/**
 * Update statistics with new block
 */
function updateStats(block) {
    stats.totalBlocks++;
    
    if (block.height > stats.latestHeight) {
        stats.latestHeight = block.height;
    }
    
    if (block.tx_count) {
        stats.totalTransactions += block.tx_count;
    }
    
    updateDashboard();
}

/**
 * Update dashboard display
 */
function updateDashboard() {
    // Calculate tx rate
    const now = Date.now();
    const recentTxs = stats.txTimestamps.filter(t => now - t < 60000);
    const txRate = recentTxs.length > 0 
        ? (recentTxs.length / (now - recentTxs[0]) * 1000).toFixed(1)
        : '0';
    
    // Update stat cards with animation
    animateCounter('totalBlocks', stats.totalBlocks);
    animateCounter('suspiciousBlocks', stats.suspiciousBlocks);
    animateCounter('rejectedBlocks', stats.rejectedBlocks);
    animateCounter('validatedBlocks', stats.validatedBlocks);
    animateCounter('txRate', parseFloat(txRate));
    animateCounter('latestHeight', stats.latestHeight);
}

/**
 * Animate counter value change
 */
function animateCounter(elementId, newValue) {
    const element = document.getElementById(elementId);
    if (!element) return;
    
    const currentValue = parseFloat(element.textContent) || 0;
    
    if (currentValue !== newValue) {
        element.textContent = typeof newValue === 'number' && newValue % 1 !== 0 
            ? newValue.toFixed(1) 
            : newValue;
        element.classList.add('counter-animate');
        setTimeout(() => element.classList.remove('counter-animate'), 300);
    }
}

/**
 * Update connection status display
 */
function setConnectionStatus(status) {
    const statusEl = document.getElementById('connectionStatus');
    const dot = statusEl.querySelector('.status-dot');
    const text = statusEl.querySelector('.status-text');
    
    dot.className = 'status-dot ' + status;
    
    const labels = {
        connected: 'Connected',
        disconnected: 'Disconnected',
        connecting: 'Connecting...',
    };
    
    text.textContent = labels[status] || status;
}

/**
 * Add entry to event log
 */
function addLogEntry(message, type = 'info') {
    const logContainer = document.getElementById('eventLog');
    const timestamp = new Date().toLocaleTimeString('en-US', { hour12: false });
    
    const entry = document.createElement('div');
    entry.className = `log-entry ${type}`;
    entry.innerHTML = `
        <span class="log-time">[${timestamp}]</span>
        <span class="log-message">${escapeHtml(message)}</span>
    `;
    
    logContainer.appendChild(entry);
    
    // Auto-scroll to bottom
    logContainer.scrollTop = logContainer.scrollHeight;
    
    // Limit log entries
    while (logContainer.children.length > MAX_LOG_ENTRIES) {
        logContainer.removeChild(logContainer.firstChild);
    }
}

/**
 * Clear event log
 */
function clearLog() {
    document.getElementById('eventLog').innerHTML = '';
}

/**
 * Escape HTML to prevent XSS
 */
function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}
