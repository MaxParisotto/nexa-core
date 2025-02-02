impl Server {
    pub async fn start(&self) -> Result<(), NexaError> {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), self.config.port);
        info!("Server starting on {}", addr);

        // ... rest of the implementation ...
    }
} 