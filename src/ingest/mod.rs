pub mod crawler;
pub mod importer;
pub mod media;
pub mod proxy;
pub mod resolver;
pub mod streamer;
pub mod watcher;

pub use crawler::{ChannelCrawler, CrawlSummary, CrawledVideoItem, NicheOutlierScout, NicheScoutVideoItem};
pub use importer::{BatchImporter, ImportChannelFeed, ImportResult, ImportVideoItem};
pub use media::{
    is_remote_url, MediaEngine, MediaSceneDetector, StreamChunk,
    MAX_CONCURRENT_STREAM_WORKERS, STREAM_CHUNK_DURATION_SEC,
};
pub use proxy::{ProxyNode, ProxyPool};
pub use resolver::{MediaPlatform, MediaResolver, ResolvedMediaStream, YtDlpMetadata};
pub use streamer::StreamIngestor;
pub use watcher::{
    is_candidate_video, is_file_stable_and_unlocked, process_video_draft, WatchFolderDaemon,
    WatcherConfig, WatcherProcessResult,
};



