/// Callsign hash cache for resolving non-standard callsigns
///
/// Maintains mappings from hash values to full callsigns, allowing
/// subsequent messages to reference callsigns by hash rather than
/// transmitting the full text each time.
///
/// Implementation follows WSJT-X behavior with bounded caches and FIFO eviction.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use hashbrown::HashMap;
use ahash::AHasher;
use core::hash::BuildHasherDefault;

type AHashMap<K, V> = HashMap<K, V, BuildHasherDefault<AHasher>>;

/// Maximum capacity for 22-bit hash cache (from WSJT-X MAXHASH constant)
pub const MAX_22BIT_CAPACITY: usize = 1000;

/// Maximum capacity for 10-bit hash cache (implicitly bounded by 10 bits = 1024 entries)
pub const MAX_10BIT_CAPACITY: usize = 1024;

/// Maximum capacity for 12-bit hash cache (implicitly bounded by 12 bits = 4096 entries)
pub const MAX_12BIT_CAPACITY: usize = 4096;

/// Callsign hash cache for resolving non-standard callsigns
///
/// In FT8, non-standard callsigns (those that don't fit pack28) are handled
/// using a two-message protocol:
/// 1. First message includes full callsign text + hash
/// 2. Subsequent messages reference callsign by hash only
///
/// This cache stores the mapping from hash values to full callsigns.
///
/// The 22-bit cache uses FIFO eviction matching WSJT-X behavior:
/// - New entries are inserted at the front (index 0)
/// - When capacity is exceeded, oldest entries (at the end) are removed
/// - Duplicate hashes are updated in-place without reordering
#[derive(Debug, Clone)]
pub struct CallsignHashCache {
    /// 10-bit hash cache (for DXpedition mode Type 0.1 messages)
    cache_10bit: AHashMap<u16, String>,

    /// 12-bit hash cache (for Type 2 messages)
    cache_12bit: AHashMap<u16, String>,

    /// 22-bit hash cache with FIFO ordering (for Type 1 hash references)
    /// Stores (hash, callsign) pairs in insertion order (newest at index 0)
    cache_22bit: Vec<(u32, String)>,

    /// Quick lookup index for 22-bit hashes: maps hash to position in Vec
    cache_22bit_index: AHashMap<u32, usize>,

    /// Maximum capacity for 22-bit cache
    max_22bit_capacity: usize,
}

impl CallsignHashCache {
    /// Create a new empty hash cache with default WSJTX capacity limits
    pub fn new() -> Self {
        Self {
            cache_10bit: AHashMap::default(),
            cache_12bit: AHashMap::default(),
            cache_22bit: Vec::with_capacity(MAX_22BIT_CAPACITY),
            cache_22bit_index: AHashMap::default(),
            max_22bit_capacity: MAX_22BIT_CAPACITY,
        }
    }

    /// Create a new hash cache with a specified 22-bit capacity
    ///
    /// This is only available for tests to allow testing FIFO behavior
    /// with smaller cache sizes without inserting 1000+ entries.
    ///
    /// # Arguments
    /// * `max_22bit_capacity` - Maximum number of entries in the 22-bit cache
    #[cfg(test)]
    pub(crate) fn with_capacity(max_22bit_capacity: usize) -> Self {
        Self {
            cache_10bit: AHashMap::default(),
            cache_12bit: AHashMap::default(),
            cache_22bit: Vec::with_capacity(max_22bit_capacity),
            cache_22bit_index: AHashMap::default(),
            max_22bit_capacity,
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
        use crate::message::callsign::{hash10, hash12, hash22};
        let hash10 = hash10(callsign) as u16;
        let h12 = hash12(callsign);
        let h22 = hash22(callsign);
        self.cache_10bit.insert(hash10, callsign.to_string());
        self.cache_12bit.insert(h12, callsign.to_string());
        self.insert_22bit(h22, callsign);
    }

    /// Insert a callsign into the 22-bit cache with FIFO eviction
    ///
    /// Implements WSJT-X behavior:
    /// - If hash exists: update callsign in-place (no reordering)
    /// - If hash is new: insert at front, evict oldest if at capacity
    ///
    /// # Arguments
    /// * `hash22` - The 22-bit hash value
    /// * `callsign` - The callsign to cache
    fn insert_22bit(&mut self, hash22: u32, callsign: &str) {
        // Check if hash already exists (update-on-hit)
        if let Some(&pos) = self.cache_22bit_index.get(&hash22) {
            // Update in-place without reordering
            if pos < self.cache_22bit.len() {
                self.cache_22bit[pos].1 = callsign.to_string();
            }
            return;
        }

        // New entry: insert at front (index 0)
        self.cache_22bit.insert(0, (hash22, callsign.to_string()));

        // Rebuild index since all positions shifted
        self.cache_22bit_index.clear();
        for (idx, (hash, _)) in self.cache_22bit.iter().enumerate() {
            self.cache_22bit_index.insert(*hash, idx);
        }

        // Evict oldest entry if over capacity
        if self.cache_22bit.len() > self.max_22bit_capacity {
            if let Some((old_hash, _)) = self.cache_22bit.pop() {
                self.cache_22bit_index.remove(&old_hash);
            }
        }
    }
    
    /// Look up a callsign by its 10-bit hash
    pub fn lookup_10bit(&self, hash10: u16) -> Option<&str> {
        self.cache_10bit.get(&hash10).map(|s| s.as_str())
    }

    /// Look up a callsign by its 12-bit hash
    pub fn lookup_12bit(&self, hash12: u16) -> Option<&str> {
        self.cache_12bit.get(&hash12).map(|s| s.as_str())
    }

    /// Look up a callsign by its 22-bit hash
    pub fn lookup_22bit(&self, hash22: u32) -> Option<&str> {
        self.cache_22bit_index.get(&hash22).and_then(|&pos| {
            self.cache_22bit.get(pos).map(|(_, callsign)| callsign.as_str())
        })
    }
    
    /// Clear all cached entries
    pub fn clear(&mut self) {
        self.cache_10bit.clear();
        self.cache_12bit.clear();
        self.cache_22bit.clear();
        self.cache_22bit_index.clear();
    }

    /// Get the number of cached entries (10-bit count, 12-bit count, 22-bit count)
    pub fn len(&self) -> (usize, usize, usize) {
        (
            self.cache_10bit.len(),
            self.cache_12bit.len(),
            self.cache_22bit.len(),
        )
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache_10bit.is_empty()
            && self.cache_12bit.is_empty()
            && self.cache_22bit.is_empty()
    }

    /// Get the maximum capacity for the 22-bit cache
    pub fn max_22bit_capacity(&self) -> usize {
        self.max_22bit_capacity
    }
}

impl Default for CallsignHashCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::string::String;
    use super::*;

    #[test]
    fn test_hash_cache() {
        use crate::message::callsign::{hash12, hash22};
        let mut cache = CallsignHashCache::new();

        // Insert callsign - hashes are computed automatically
        cache.insert("PJ4/K1ABC");

        // Compute expected hashes
        let h12 = hash12("PJ4/K1ABC");
        let h22 = hash22("PJ4/K1ABC");

        // Look up by 12-bit hash
        assert_eq!(cache.lookup_12bit(h12), Some("PJ4/K1ABC"));

        // Look up by 22-bit hash
        assert_eq!(cache.lookup_22bit(h22), Some("PJ4/K1ABC"));

        // Verify not found for non-existent hash
        assert_eq!(cache.lookup_12bit(9999), None);
        assert_eq!(cache.lookup_22bit(9999999), None);
    }

    #[test]
    fn test_hash_cache_multiple_entries() {
        use crate::message::callsign::{hash12, hash22};
        let mut cache = CallsignHashCache::new();

        let callsigns = vec!["PJ4/K1ABC", "KH1/KH7Z", "W9XYZ/7"];

        // Insert all callsigns
        for call in &callsigns {
            cache.insert(call);
        }

        // Verify all can be looked up by their computed hashes
        for call in &callsigns {
            let h12 = hash12(call);
            let h22 = hash22(call);
            assert_eq!(cache.lookup_12bit(h12), Some(*call));
            assert_eq!(cache.lookup_22bit(h22), Some(*call));
        }
    }

    #[test]
    fn test_hash_cache_clear() {
        use crate::message::callsign::{hash12, hash22};
        let mut cache = CallsignHashCache::new();
        cache.insert("PJ4/K1ABC");

        assert!(!cache.is_empty());

        let h12 = hash12("PJ4/K1ABC");
        let h22 = hash22("PJ4/K1ABC");

        cache.clear();
        assert!(cache.is_empty());

        assert_eq!(cache.lookup_12bit(h12), None);
        assert_eq!(cache.lookup_22bit(h22), None);
    }

    #[test]
    fn test_hash_cache_insert_auto() {
        let mut cache = CallsignHashCache::new();

        // Use the new insert() method that auto-computes hashes
        cache.insert("KH1/KH7Z");

        // Should be able to look up by all hash sizes
        // Expected hashes for KH1/KH7Z: 10-bit=201, 12-bit=806, 22-bit=825805
        assert_eq!(cache.lookup_10bit(201), Some("KH1/KH7Z"));
        assert_eq!(cache.lookup_12bit(806), Some("KH1/KH7Z"));
        assert_eq!(cache.lookup_22bit(825805), Some("KH1/KH7Z"));
    }

    #[test]
    fn test_22bit_capacity_enforcement() {
        use crate::message::callsign::hash22;
        // Create cache with small capacity for testing
        let mut cache = CallsignHashCache::with_capacity(3);

        // Use real callsigns that are unlikely to collide
        let calls = vec!["AA1A", "AB1B", "AC1C", "AD1D"];

        // Insert 3 entries (at capacity)
        for call in &calls[0..3] {
            cache.insert(call);
        }

        let (_, _, count_22bit) = cache.len();
        assert_eq!(count_22bit, 3);

        // Compute hashes for verification
        let hashes: Vec<u32> = calls.iter().map(|c| hash22(c)).collect();

        // Verify all entries are present
        assert_eq!(cache.lookup_22bit(hashes[0]), Some(calls[0]));
        assert_eq!(cache.lookup_22bit(hashes[1]), Some(calls[1]));
        assert_eq!(cache.lookup_22bit(hashes[2]), Some(calls[2]));

        // Insert 4th entry - should evict oldest (first callsign)
        cache.insert(calls[3]);

        let (_, _, count_22bit) = cache.len();
        assert_eq!(count_22bit, 3, "Cache should remain at capacity");

        // First callsign should be evicted (oldest)
        assert_eq!(
            cache.lookup_22bit(hashes[0]),
            None,
            "Oldest entry should be evicted"
        );

        // Other entries should still be present
        assert_eq!(cache.lookup_22bit(hashes[1]), Some(calls[1]));
        assert_eq!(cache.lookup_22bit(hashes[2]), Some(calls[2]));
        assert_eq!(cache.lookup_22bit(hashes[3]), Some(calls[3]));
    }

    #[test]
    fn test_22bit_fifo_eviction_order() {
        use crate::message::callsign::hash22;
        // Create cache with capacity of 5
        let mut cache = CallsignHashCache::with_capacity(5);

        // Generate 8 unique callsigns
        let calls: Vec<String> = (1..=8).map(|i| format!("W{}ABC", i)).collect();

        // Insert first 5 entries
        for call in &calls[0..5] {
            cache.insert(call);
        }

        // Compute all hashes
        let hashes: Vec<u32> = calls.iter().map(|c| hash22(c)).collect();

        // All first 5 should be present
        for i in 0..5 {
            assert_eq!(cache.lookup_22bit(hashes[i]), Some(calls[i].as_str()));
        }

        // Insert 3 more entries
        for call in &calls[5..8] {
            cache.insert(call);
        }

        // First 3 entries should be evicted (FIFO)
        assert_eq!(cache.lookup_22bit(hashes[0]), None);
        assert_eq!(cache.lookup_22bit(hashes[1]), None);
        assert_eq!(cache.lookup_22bit(hashes[2]), None);

        // Last 5 entries should remain
        for i in 3..8 {
            assert_eq!(cache.lookup_22bit(hashes[i]), Some(calls[i].as_str()));
        }
    }

    #[test]
    fn test_22bit_update_on_hit_no_reorder() {
        use crate::message::callsign::hash22;
        // Create cache with capacity of 3
        let mut cache = CallsignHashCache::with_capacity(3);

        // Use real callsigns
        let calls = vec!["K1AA", "K2BB", "K3CC", "K4DD"];

        // Insert 3 entries
        for call in &calls[0..3] {
            cache.insert(call);
        }

        let hashes: Vec<u32> = calls.iter().map(|c| hash22(c)).collect();

        // Re-insert first callsign (update-on-hit)
        cache.insert(calls[0]);

        // Verify it's still present
        assert_eq!(cache.lookup_22bit(hashes[0]), Some(calls[0]));

        // All entries should still be present (no eviction on update)
        assert_eq!(cache.lookup_22bit(hashes[1]), Some(calls[1]));
        assert_eq!(cache.lookup_22bit(hashes[2]), Some(calls[2]));

        let (_, _, count_22bit) = cache.len();
        assert_eq!(count_22bit, 3);

        // Insert a new entry - first callsign should still be evicted first
        // because update-on-hit doesn't change position (WSJT-X behavior)
        cache.insert(calls[3]);

        // First callsign should be gone (it was still oldest despite update)
        assert_eq!(
            cache.lookup_22bit(hashes[0]),
            None,
            "Updated entry should still be evicted by age, not reordered"
        );

        // Other entries should remain
        assert_eq!(cache.lookup_22bit(hashes[1]), Some(calls[1]));
        assert_eq!(cache.lookup_22bit(hashes[2]), Some(calls[2]));
        assert_eq!(cache.lookup_22bit(hashes[3]), Some(calls[3]));
    }

    #[test]
    fn test_22bit_default_capacity() {
        let cache = CallsignHashCache::new();
        assert_eq!(
            cache.max_22bit_capacity(),
            MAX_22BIT_CAPACITY,
            "Default capacity should match WSJT-X MAXHASH"
        );
        assert_eq!(MAX_22BIT_CAPACITY, 1000, "MAXHASH should be 1000");
    }

    #[test]
    fn test_22bit_custom_capacity() {
        let cache = CallsignHashCache::with_capacity(500);
        assert_eq!(cache.max_22bit_capacity(), 500);
    }

    #[test]
    fn test_wsjtx_compatible_capacity_limits() {
        // Verify capacity constants match WSJT-X
        assert_eq!(MAX_22BIT_CAPACITY, 1000, "MAXHASH = 1000");
        assert_eq!(MAX_10BIT_CAPACITY, 1024, "10-bit = 2^10");
        assert_eq!(MAX_12BIT_CAPACITY, 4096, "12-bit = 2^12");
    }

    #[test]
    fn test_22bit_many_insertions() {
        use crate::message::callsign::hash22;
        // Test with many entries to ensure FIFO works correctly
        let mut cache = CallsignHashCache::with_capacity(10);

        // Generate 20 unique callsigns
        let calls: Vec<String> = (0..20).map(|i| format!("N{}XY", i)).collect();

        // Insert 20 entries
        for call in &calls {
            cache.insert(call);
        }

        // Only last 10 should remain
        let (_, _, count_22bit) = cache.len();
        assert_eq!(count_22bit, 10);

        // Compute hashes
        let hashes: Vec<u32> = calls.iter().map(|c| hash22(c)).collect();

        // First 10 should be evicted
        for i in 0..10 {
            assert_eq!(cache.lookup_22bit(hashes[i]), None);
        }

        // Last 10 should be present
        for i in 10..20 {
            assert_eq!(cache.lookup_22bit(hashes[i]), Some(calls[i].as_str()));
        }
    }

    #[test]
    fn test_mixed_cache_operations() {
        let mut cache = CallsignHashCache::new();

        // Insert into all cache types
        cache.insert("KH1/KH7Z");

        // Verify all cache types work independently
        assert_eq!(cache.lookup_10bit(201), Some("KH1/KH7Z"));
        assert_eq!(cache.lookup_12bit(806), Some("KH1/KH7Z"));
        assert_eq!(cache.lookup_22bit(825805), Some("KH1/KH7Z"));

        let (count_10, count_12, count_22) = cache.len();
        assert_eq!(count_10, 1);
        assert_eq!(count_12, 1);
        assert_eq!(count_22, 1);

        // 12-bit and 10-bit caches are unbounded HashMaps
        // Only 22-bit has capacity enforcement
    }

    #[test]
    fn test_22bit_index_consistency() {
        use crate::message::callsign::hash22;
        let mut cache = CallsignHashCache::with_capacity(3);

        // Use real callsigns
        let calls = vec!["VE1AA", "VE2BB", "VE3CC", "VE4DD"];

        // Insert 3 entries
        for call in &calls[0..3] {
            cache.insert(call);
        }

        let hashes: Vec<u32> = calls.iter().map(|c| hash22(c)).collect();

        // Verify index is consistent with Vec
        assert_eq!(cache.lookup_22bit(hashes[0]), Some(calls[0]));
        assert_eq!(cache.lookup_22bit(hashes[1]), Some(calls[1]));
        assert_eq!(cache.lookup_22bit(hashes[2]), Some(calls[2]));

        // Trigger eviction
        cache.insert(calls[3]);

        // Index should still be consistent
        assert_eq!(cache.lookup_22bit(hashes[0]), None);
        assert_eq!(cache.lookup_22bit(hashes[1]), Some(calls[1]));
        assert_eq!(cache.lookup_22bit(hashes[2]), Some(calls[2]));
        assert_eq!(cache.lookup_22bit(hashes[3]), Some(calls[3]));

        // Re-insert an existing entry (update)
        cache.insert(calls[1]);
        assert_eq!(cache.lookup_22bit(hashes[1]), Some(calls[1]));
    }
}
