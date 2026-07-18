use crate::crypto::identity::PublicIdentity;
use crate::network::peer::PeerRegistry;

pub struct SearchResult {
    pub identity: PublicIdentity,
    pub handle: String,
    pub onion_address: String,
    pub is_online: bool,
}

impl PeerRegistry {
    pub async fn search_users(&self, query: &str) -> Vec<SearchResult> {
        let peers = self.all_peers().await;
        let mut results: Vec<SearchResult> = peers
            .into_iter()
            .filter(|peer| peer.identity.matches_search(query))
            .map(|peer| SearchResult {
                handle: peer.identity.handle(),
                onion_address: peer.onion_address.clone(),
                is_online: true,
                identity: peer.identity,
            })
            .collect();

        results.sort_by(|a, b| {
            let a_exact = a.identity.alias.to_lowercase() == query.to_lowercase();
            let b_exact = b.identity.alias.to_lowercase() == query.to_lowercase();
            b_exact.cmp(&a_exact)
        });

        results
    }
}
