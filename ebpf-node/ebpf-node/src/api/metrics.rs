use prometheus::{Encoder, TextEncoder};

/// GET /metrics - Prometheus metrics endpoint
pub async fn metrics_handler() -> String {
    let encoder = TextEncoder::new();
    let mut buffer = vec![];
    let metric_families = prometheus::gather();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
