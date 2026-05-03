/**
 * WebSocket Client for eBPF Blockchain Monitor
 * Handles connection to node WebSocket endpoint and event processing
 */

class WebSocketClient {
    constructor() {
        this.ws = null;
        this.url = '';
        this.apiUrl = '';
        this.connected = false;
        this.reconnectAttempts = 0;
        this.maxReconnectAttempts = 10;
        this.reconnectDelay = 1000;
        this.callbacks = {
            onConnect: [],
            onDisconnect: [],
            onBlockCreated: [],
            onBlockConfirmed: [],
            onBlockRejected: [],
            onSecurityAlert: [],
            onTxProcessed: [],
            onMessage: [],
            onError: [],
        };
    }

    /**
     * Register callback for events
     */
    on(event, callback) {
        if (this.callbacks[event]) {
            this.callbacks[event].push(callback);
        }
    }

    /**
     * Emit event to all registered callbacks
     */
    emit(event, data) {
        if (this.callbacks[event]) {
            this.callbacks[event].forEach(cb => {
                try {
                    cb(data);
                } catch (e) {
                    console.error(`Error in callback for ${event}:`, e);
                }
            });
        }
    }

    /**
     * Connect to WebSocket endpoint
     */
    connect(url, apiUrl) {
        this.url = url;
        this.apiUrl = apiUrl;
        
        if (this.ws && this.ws.readyState === WebSocket.OPEN) {
            console.log('Already connected');
            return;
        }

        try {
            this.ws = new WebSocket(url);
            
            this.ws.onopen = () => {
                console.log('WebSocket connected');
                this.connected = true;
                this.reconnectAttempts = 0;
                this.emit('onConnect', { url });
            };

            this.ws.onmessage = (event) => {
                try {
                    const data = JSON.parse(event.data);
                    this.processMessage(data);
                } catch (e) {
                    console.error('Error parsing message:', e);
                    this.emit('onMessage', { raw: event.data });
                }
            };

            this.ws.onclose = (event) => {
                console.log(`WebSocket closed: code=${event.code}, reason=${event.reason}`);
                this.connected = false;
                this.emit('onDisconnect', { code: event.code, reason: event.reason });
                this.scheduleReconnect();
            };

            this.ws.onerror = (error) => {
                console.error('WebSocket error:', error);
                this.emit('onError', { error });
            };

        } catch (e) {
            console.error('Failed to create WebSocket:', e);
            this.emit('onError', { error: e.message });
        }
    }

    /**
     * Process incoming WebSocket message
     */
    processMessage(data) {
        this.emit('onMessage', data);
        
        switch (data.event) {
            case 'BlockCreated':
                this.emit('onBlockCreated', data);
                break;
            case 'BlockConfirmed':
                this.emit('onBlockConfirmed', data);
                break;
            case 'BlockRejected':
                this.emit('onBlockRejected', data);
                break;
            case 'SecurityAlert':
                this.emit('onSecurityAlert', data);
                break;
            case 'TxProcessed':
                this.emit('onTxProcessed', data);
                break;
            default:
                console.log('Unknown event:', data.event);
        }
    }

    /**
     * Schedule automatic reconnection
     */
    scheduleReconnect() {
        if (this.reconnectAttempts >= this.maxReconnectAttempts) {
            console.log('Max reconnect attempts reached');
            this.emit('onError', { error: 'Max reconnect attempts reached' });
            return;
        }

        this.reconnectAttempts++;
        const delay = this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1);
        console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);
        
        setTimeout(() => {
            if (!this.connected && this.url) {
                this.connect(this.url, this.apiUrl);
            }
        }, delay);
    }

    /**
     * Disconnect from WebSocket
     */
    disconnect() {
        if (this.ws) {
            this.ws.close(1000, 'User disconnected');
            this.ws = null;
        }
        this.connected = false;
        this.reconnectAttempts = 0;
    }

    /**
     * Fetch blocks from REST API
     */
    async fetchBlocks(height = null) {
        if (!this.apiUrl) {
            console.warn('No API URL configured');
            return null;
        }

        try {
            const url = height 
                ? `${this.apiUrl}/api/v1/blocks/${height}`
                : `${this.apiUrl}/api/v1/blocks/latest`;
            
            const response = await fetch(url);
            if (!response.ok) {
                throw new Error(`HTTP ${response.status}: ${response.statusText}`);
            }
            return await response.json();
        } catch (e) {
            console.error('Failed to fetch blocks:', e);
            return null;
        }
    }

    /**
     * Fetch node health status
     */
    async fetchHealth() {
        if (!this.apiUrl) {
            return null;
        }

        try {
            const response = await fetch(`${this.apiUrl}/api/v1/health`);
            if (!response.ok) {
                throw new Error(`HTTP ${response.status}`);
            }
            return await response.json();
        } catch (e) {
            console.error('Failed to fetch health:', e);
            return null;
        }
    }
}

// Export for use in other modules
if (typeof module !== 'undefined' && module.exports) {
    module.exports = WebSocketClient;
}
