/// Callsign hash cache for resolving non-standard callsigns
/// 
/// Maintains mappings from hash values to full callsigns, allowing
/// subsequent messages to reference callsigns by hash rather than
/// transmitting the full text each time.

use alloc::string::{String, ToString};
use hashbrown::HashMap;
use ahash::AHasher;
use core::hash::BuildHasherDefault;

type AHashMap<K, V> = HashMap<K, V, BuildHasherDefault<AHasher>>;

/// Callsign hash cache for resolving non-standard callsigns
/// 
/// In FT8, non-standard callsigns (those that don't fit pack28) are handled
/// using a two-message protocol:
/// 1. First message includes full callsign text + hash
/// 2. Subsequent messages reference callsign by hash only
/// 
/// This cache stores the mapping from hash values to full callsigns.
#[derive(Debug, Clone)]
pub struct CallsignHashCache {
    /// 10-bit hash cache (for DXpedition mode Type 0.1 messages)
    cache_10bit: AHashMap<u16, String>,
    
    /// 12-bit hash cache (for Type 2 messages)
    cache_12bit: AHashMap<u16, String>,
    
    /// 22-bit hash cache (for Type 1 hash references)
    cache_22bit: AHashMap<u32, String>,
}

impl CallsignHashCache {
    /// Create a new empty hash cache
    pub fn new() -> Self {
        Self {
            cache_10bit: AHashMap::default(),
            cache_12bit: AHashMap::default(),
            cache_22bit: AHashMap::default(),
        }
    }
    
    /// Insert a callsign into all hash caches
    /// 
    /// Computes 10-bit, 12-bit and 22-bit hashes and stores the callsign
    /// under all keys for future lookup.
    /// 
    /// # Arguments
    /// * `callsign` - The callsign to cache
    pub fn insert(&mut self, callsign: &str) {
        use crate::message::callsign::{ihashcall, hash12, hash22};
        let hash10 = ihashcall(callsign, 10) as u16;
        let h12 = hash12(callsign);
        let h22 = hash22(callsign);
        self.cache_10bit.insert(hash10, callsign.to_string());
        self.cache_12bit.insert(h12, callsign.to_string());
        self.cache_22bit.insert(h22, callsign.to_string());
    }
    
    /// Insert a callsign into both hash caches
    /// 
    /// Computes both 12-bit and 22-bit hashes and stores the callsign
    /// under both keys for future lookup.
    /// 
    /// # Arguments
    /// * `callsign` - The callsign to cache
    /// * `hash12` - Precomputed 12-bit hash
    /// * `hash22` - Precomputed 22-bit hash
    pub fn insert_with_hashes(&mut self, callsign: &str, hash12: u16, hash22: u32) {
        self.cache_12bit.insert(hash12, callsign.to_string());
        self.cache_22bit.insert(hash22, callsign.to_string());
    }
    
    /// Look up a callsign by its 10-bit hash
    pub fn lookup_10bit(&self, hash10: u16) -> Option<&String> {
        self.cache_10bit.get(&hash10)
    }
    
    /// Look up a callsign by its 12-bit hash
    pub fn lookup_12bit(&self, hash12: u16) -> Option<&String> {
        self.cache_12bit.get(&hash12)
    }
    
    /// Look up a callsign by its 22-bit hash
    pub fn lookup_22bit(&self, hash22: u32) -> Option<&String> {
        self.cache_22bit.get(&hash22)
    }
    
    /// Clear all cached entries
    pub fn clear(&mut self) {
        self.cache_10bit.clear();
        self.cache_12bit.clear();
        self.cache_22bit.clear();
    }
    
    /// Get the number of cached entries (10-bit count, 12-bit count, 22-bit count)
    pub fn len(&self) -> (usize, usize, usize) {
        (self.cache_10bit.len(), self.cache_12bit.len(), self.cache_22bit.len())
    }
    
    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache_10bit.is_empty() && self.cache_12bit.is_empty() && self.cache_22bit.is_empty()
    }
}

impl Default for CallsignHashCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::string::{String, ToString};
    use super::*;

    #[test]
    fn test_hash_cache() {
        let mut cache = CallsignHashCache::new();
        
        // Insert callsign with known hash values
        cache.insert_with_hashes("PJ4/K1ABC", 1387, 1420834);
        
        // Look up by 12-bit hash
        assert_eq!(cache.lookup_12bit(1387), Some(&"PJ4/K1ABC".to_string()));
        
        // Look up by 22-bit hash
        assert_eq!(cache.lookup_22bit(1420834), Some(&"PJ4/K1ABC".to_string()));
        
        // Verify not found for non-existent hash
        assert_eq!(cache.lookup_12bit(9999), None);
        assert_eq!(cache.lookup_22bit(9999999), None);
    }

    #[test]
    fn test_hash_cache_multiple_entries() {
        let mut cache = CallsignHashCache::new();
        
        let callsigns = vec![
            ("PJ4/K1ABC", 1387u16, 1420834u32),
            ("KH1/KH7Z", 806u16, 825805u32),
            ("W9XYZ/7", 1927u16, 1973674u32),
        ];
        
        for (call, h12, h22) in &callsigns {
            cache.insert_with_hashes(call, *h12, *h22);
        }
        
        // Verify all can be looked up
        for (call, h12, h22) in &callsigns {
            assert_eq!(cache.lookup_12bit(*h12), Some(&call.to_string()));
            assert_eq!(cache.lookup_22bit(*h22), Some(&call.to_string()));
        }
    }

    #[test]
    fn test_hash_cache_clear() {
        let mut cache = CallsignHashCache::new();
        cache.insert_with_hashes("PJ4/K1ABC", 1387, 1420834);
        
        assert!(!cache.is_empty());
        
        cache.clear();
        assert!(cache.is_empty());
        
        assert_eq!(cache.lookup_12bit(1387), None);
        assert_eq!(cache.lookup_22bit(1420834), None);
    }

    #[test]
    fn test_hash_cache_insert_auto() {
        let mut cache = CallsignHashCache::new();
        
        // Use the new insert() method that auto-computes hashes
        cache.insert("KH1/KH7Z");
        
        // Should be able to look up by all hash sizes
        // Expected hashes for KH1/KH7Z: 10-bit=201, 12-bit=806, 22-bit=825805
        assert_eq!(cache.lookup_10bit(201), Some(&"KH1/KH7Z".to_string()));
        assert_eq!(cache.lookup_12bit(806), Some(&"KH1/KH7Z".to_string()));
        assert_eq!(cache.lookup_22bit(825805), Some(&"KH1/KH7Z".to_string()));
    }
}
