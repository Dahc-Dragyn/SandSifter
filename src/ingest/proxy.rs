use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Individual proxy node state with failure tracking.
#[derive(Debug, Clone)]
pub struct ProxyNode {
    pub url: String,
    pub failure_count: u32,
    pub last_used: Instant,
    pub is_healthy: bool,
}

/// Residential and Datacenter proxy rotation pool.
#[derive(Clone)]
pub struct ProxyPool {
    nodes: Arc<Mutex<Vec<ProxyNode>>>,
    current_idx: Arc<Mutex<usize>>,
    user_agents: Vec<&'static str>,
}

impl Default for ProxyPool {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl ProxyPool {
    /// Curated pool of high-reputation, realistic browser User-Agents
    pub const USER_AGENTS: [&'static str; 4] = [
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36",
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15",
        "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/127.0.0.0 Safari/537.36 Edg/127.0.0.0",
    ];

    pub fn new(proxy_urls: Vec<String>) -> Self {
        let now = Instant::now();
        let nodes = proxy_urls
            .into_iter()
            .map(|url| ProxyNode {
                url,
                failure_count: 0,
                last_used: now,
                is_healthy: true,
            })
            .collect();

        Self {
            nodes: Arc::new(Mutex::new(nodes)),
            current_idx: Arc::new(Mutex::new(0)),
            user_agents: Self::USER_AGENTS.to_vec(),
        }
    }

    /// Selects the next available proxy URL in round-robin sequence.
    /// Returns None if no proxies are configured or all nodes are quarantined.
    pub fn next_proxy(&self) -> Option<String> {
        let mut nodes = self.nodes.lock().unwrap();
        if nodes.is_empty() {
            return None;
        }

        let mut idx = self.current_idx.lock().unwrap();
        let start_idx = *idx;
        let total = nodes.len();

        for i in 0..total {
            let candidate_idx = (start_idx + i) % total;
            if nodes[candidate_idx].is_healthy {
                nodes[candidate_idx].last_used = Instant::now();
                *idx = (candidate_idx + 1) % total;
                return Some(nodes[candidate_idx].url.clone());
            }
        }

        None
    }

    /// Rotates and returns a realistic browser User-Agent header.
    pub fn next_user_agent(&self) -> &'static str {
        let mut idx = self.current_idx.lock().unwrap();
        let ua = self.user_agents[*idx % self.user_agents.len()];
        *idx = (*idx + 1) % self.user_agents.len();
        ua
    }

    /// Reports an error for a proxy, incrementing failure counter and quarantining if threshold met.
    pub fn report_failure(&self, proxy_url: &str) {
        let mut nodes = self.nodes.lock().unwrap();
        if let Some(node) = nodes.iter_mut().find(|n| n.url == proxy_url) {
            node.failure_count += 1;
            if node.failure_count >= 3 {
                node.is_healthy = false;
            }
        }
    }

    /// Reports a successful request, resetting failure counter.
    pub fn report_success(&self, proxy_url: &str) {
        let mut nodes = self.nodes.lock().unwrap();
        if let Some(node) = nodes.iter_mut().find(|n| n.url == proxy_url) {
            node.failure_count = 0;
            node.is_healthy = true;
        }
    }

    /// Total count of configured proxies
    pub fn total_proxies(&self) -> usize {
        self.nodes.lock().unwrap().len()
    }
}
