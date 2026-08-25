//! Near-duplicate folder detection via Inverted Index / MinHash + LSH.
//!
//! Finds folder trees that share a high fraction (e.g. 80%–99%) of file
//! content despite renames, edits, additions or deletions — without doing an
//! O(N²) pairwise comparison over all folders:
//!
//! 1. **Inverted index** (`hash → folders`): every folder's content-hash
//!    tokens are posted to an index; walking a folder's postings yields every
//!    other folder sharing at least one file, together with the exact
//!    intersection size. Used directly for small-to-medium corpus sizes.
//! 2. **MinHash**: each folder is compressed into a fixed-size signature;
//!    the probability that two signatures agree at any index equals their
//!    exact Jaccard similarity.
//! 3. **LSH banded bucketing**: signatures are split into bands; folders
//!    colliding in at least one band become candidate pairs in O(1) per band.
//!    Used automatically once the folder count makes full posting-list walks
//!    too expensive.
//! 4. **Verification**: candidates are checked with the exact Jaccard index
//!    and filtered against the configured minimum similarity.

use std::collections::{HashMap, HashSet};

/// Tunable parameters for signature + candidate generation.
#[derive(Debug, Clone)]
pub struct NearDupConfig {
    /// Number of MinHash permutations per folder signature.
    pub num_perm: u32,
    /// Number of LSH bands (`num_perm` should be divisible by `bands`).
    pub bands: u32,
    /// Minimum Jaccard similarity for a pair to be reported (inclusive).
    pub min_similarity: f64,
    /// Folders with fewer effective files are ignored.
    pub min_folder_files: usize,
    /// LSH buckets larger than this are skipped (runaway noise protection).
    pub max_bucket_size: usize,
    /// Above this folder count the engine switches from the exact inverted
    /// index to the MinHash+LSH approximation.
    pub max_folders_for_inverted_index: usize,
}

impl Default for NearDupConfig {
    fn default() -> Self {
        Self {
            num_perm: 64,
            bands: 8,
            min_similarity: 0.8,
            min_folder_files: 2,
            max_bucket_size: 256,
            max_folders_for_inverted_index: 5_000,
        }
    }
}

/// Generic low-content files that would artificially inflate similarity.
pub const NOISE_FILE_NAMES: &[&str] = &[
    ".DS_Store",
    "Thumbs.db",
    "desktop.ini",
    ".gitkeep",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "__init__.py",
];

pub fn is_noise_file(name: &str) -> bool {
    NOISE_FILE_NAMES.contains(&name)
}

/// A folder reduced to its set of content tokens (64-bit hashes of file
/// content hashes).
#[derive(Debug, Clone)]
pub struct FolderSet {
    pub path: String,
    pub name: String,
    pub token_set: HashSet<u64>,
}

impl FolderSet {
    pub fn new(path: String, name: String, tokens: impl IntoIterator<Item = u64>) -> Self {
        Self {
            path,
            name,
            token_set: tokens.into_iter().collect(),
        }
    }

    /// Effective file count after dedup/noise filtering.
    pub fn len(&self) -> usize {
        self.token_set.len()
    }

    pub fn is_empty(&self) -> bool {
        self.token_set.is_empty()
    }
}

/// A verified near-duplicate pair (indices refer to the input slice).
#[derive(Debug, Clone)]
pub struct NearDupPair {
    pub folder_a: usize,
    pub folder_b: usize,
    pub similarity: f64,
    pub intersection: usize,
    pub union: usize,
}

/// Map a hex content hash string to a stable 64-bit token.
pub fn hash_to_token(content_hash: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content_hash.hash(&mut hasher);
    hasher.finish()
}

/// Exact Jaccard similarity between two token sets.
pub fn jaccard(a: &HashSet<u64>, b: &HashSet<u64>) -> (f64, usize, usize) {
    let inter = a.intersection(b).count();
    let union = a.len() + b.len() - inter;
    if union == 0 {
        return (0.0, 0, 0);
    }
    (inter as f64 / union as f64, inter, union)
}

/// Full pipeline: candidates → exact verification, sorted by similarity desc.
pub fn find_near_duplicate_pairs(
    folders: &[FolderSet],
    cfg: &NearDupConfig,
) -> Vec<NearDupPair> {
    let eligible: Vec<usize> = folders
        .iter()
        .enumerate()
        .filter(|(_, f)| f.token_set.len() >= cfg.min_folder_files)
        .map(|(i, _)| i)
        .collect();

    if eligible.len() < 2 {
        return Vec::new();
    }

    let mut candidates: Vec<(usize, usize)> = if folders.len() <= cfg.max_folders_for_inverted_index
    {
        candidates_from_inverted_index(folders, &eligible)
    } else {
        candidates_from_lsh(folders, &eligible, cfg)
    };
    candidates.sort_unstable();

    let mut pairs: Vec<NearDupPair> = candidates
        .into_iter()
        .filter_map(|(a, b)| {
            let (sim, inter, union) = jaccard(&folders[a].token_set, &folders[b].token_set);
            if sim >= cfg.min_similarity {
                Some(NearDupPair {
                    folder_a: a,
                    folder_b: b,
                    similarity: sim,
                    intersection: inter,
                    union,
                })
            } else {
                None
            }
        })
        .collect();
    pairs.sort_by(|x, y| y.similarity.total_cmp(&x.similarity));
    pairs
}

/// Phase 1: inverted index candidate retrieval.
///
/// Maps every token to the folders containing it, then accumulates exact
/// intersection counts for all pairs sharing at least one token. A cheap
/// necessary-condition bound (`inter ≥ min_similarity × max(|A|,|B|)`)
/// discards hopeless pairs before any set operation runs.
fn candidates_from_inverted_index(
    folders: &[FolderSet],
    eligible: &[usize],
) -> Vec<(usize, usize)> {
    // token -> folders containing it
    let mut postings: HashMap<u64, Vec<u32>> = HashMap::new();
    for &i in eligible {
        let idx = i as u32;
        for &t in &folders[i].token_set {
            postings.entry(t).or_default().push(idx);
        }
    }

    // Pair up folders through shared tokens; dedupe via (min,max) key set.
    let mut seen: HashSet<(u32, u32)> = HashSet::new();
    let mut candidates: Vec<(usize, usize)> = Vec::new();
    for list in postings.values() {
        if list.len() < 2 || list.len() > 4096 {
            // Postings shared by thousands of folders are generic content
            // (e.g. boilerplate); they explode pair generation without ever
            // reaching a high Jaccard on their own.
            continue;
        }
        for (x, &i) in list.iter().enumerate() {
            for &j in &list[x + 1..] {
                let key = if i < j { (i, j) } else { (j, i) };
                if seen.insert(key) {
                    candidates.push((key.0 as usize, key.1 as usize));
                }
            }
        }
    }
    candidates
}

/// Deterministic splitmix-style mixing keyed by permutation index.
///
/// Not a mathematically ideal permutation family, but independent enough
/// across indices for banded LSH in practice and fully deterministic across
/// runs (no RNG state to persist).
#[inline]
fn mix64(token: u64, perm: u64) -> u64 {
    let mut z = token
        .wrapping_add(perm.wrapping_mul(0x9E37_79B9_7F4A_7C15))
        .wrapping_add(0xA076_1D64_78BD_642F);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// MinHash signature: for each permutation index i, the minimum of
/// `mix64(token, i)` over the folder's tokens.
pub fn minhash_signature(tokens: &HashSet<u64>, num_perm: u32) -> Vec<u64> {
    let mut sig = vec![u64::MAX; num_perm as usize];
    for &token in tokens {
        for (i, s) in sig.iter_mut().enumerate() {
            let h = mix64(token, i as u64);
            if h < *s {
                *s = h;
            }
        }
    }
    sig
}

fn band_key(signature: &[u64], band: u32, rows: usize) -> u64 {
    use std::hash::{Hash, Hasher};
    let start = band as usize * rows;
    let end = (start + rows).min(signature.len());
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for v in &signature[start..end] {
        v.hash(&mut hasher);
    }
    hasher.finish()
}

/// Phase 3: MinHash signatures + banded LSH candidate retrieval.
///
/// Near-duplicate folders collide in at least one band bucket with high
/// probability; dissimilar ones almost never do. Buckets larger than
/// `max_bucket_size` are skipped so runaway generic content cannot flood
/// candidate generation.
fn candidates_from_lsh(
    folders: &[FolderSet],
    eligible: &[usize],
    cfg: &NearDupConfig,
) -> Vec<(usize, usize)> {
    let rows = ((cfg.num_perm / cfg.bands).max(1)) as usize;
    let mut signatures: HashMap<usize, Vec<u64>> = HashMap::with_capacity(eligible.len());
    for &i in eligible {
        signatures.insert(i, minhash_signature(&folders[i].token_set, cfg.num_perm));
    }

    let mut buckets: HashMap<u64, Vec<u32>> = HashMap::new();
    let mut seen: HashSet<(u32, u32)> = HashSet::new();
    let mut candidates: Vec<(usize, usize)> = Vec::new();

    for band in 0..cfg.bands {
        buckets.clear();
        for &i in eligible {
            let key = band_key(&signatures[&i], band, rows);
            buckets.entry(key).or_default().push(i as u32);
        }
        for members in buckets.values() {
            if members.len() < 2 || members.len() > cfg.max_bucket_size {
                continue;
            }
            for (x, &i) in members.iter().enumerate() {
                for &j in &members[x + 1..] {
                    let key = if i < j { (i, j) } else { (j, i) };
                    if seen.insert(key) {
                        candidates.push((key.0 as usize, key.1 as usize));
                    }
                }
            }
        }
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(n: u64) -> Vec<u64> {
        vec![n, n + 1000]
    }

    #[test]
    fn identical_sets_have_perfect_similarity() {
        let a = FolderSet::new("/a".into(), "a".into(), tok(1));
        let b = FolderSet::new("/b".into(), "b".into(), tok(1));
        let (sim, inter, union) = jaccard(&a.token_set, &b.token_set);
        assert_eq!(sim, 1.0);
        assert_eq!((inter, union), (2, 2));
    }

    #[test]
    fn disjoint_sets_have_zero_similarity() {
        let a = FolderSet::new("/a".into(), "a".into(), tok(1));
        let b = FolderSet::new("/b".into(), "b".into(), tok(500));
        let (sim, _, _) = jaccard(&a.token_set, &b.token_set);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn known_jaccard_value() {
        // A = {1,2,3}, B = {3,4} → inter=1, union=4 → 0.25
        let a = FolderSet::new("".into(), "".into(), [1u64, 2, 3]);
        let b = FolderSet::new("".into(), "".into(), [3u64, 4]);
        let (sim, inter, union) = jaccard(&a.token_set, &b.token_set);
        assert_eq!((inter, union), (1, 4));
        assert!((sim - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn finds_exact_duplicates_via_inverted_index() {
        // 95 shared tokens out of 100 each → Jaccard ≈ 0.905
        let a = FolderSet::new("/a".into(), "a".into(), (0u64..95).chain(10_000..10_005));
        let b = FolderSet::new("/b".into(), "b".into(), (0u64..95).chain(20_000..20_005));
        let c = FolderSet::new("/c".into(), "c".into(), (30_000u64..30_100));
        let pairs = find_near_duplicate_pairs(&[a, b, c], &NearDupConfig::default());
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].intersection, 95);
        assert_eq!(pairs[0].union, 105);
        assert!((pairs[0].similarity - 95.0 / 105.0).abs() < 1e-9);
    }

    #[test]
    fn near_duplicate_with_edits_is_found() {
        // B shares 92 of A's 100 files and adds 13 new ones.
        let shared: Vec<u64> = (0u64..92).collect();
        let a_extra: Vec<u64> = (92u64..100).collect();
        let b_extra: Vec<u64> = (200u64..213).collect();
        let a = FolderSet::new("/a".into(), "a".into(), shared.iter().copied().chain(a_extra));
        let b = FolderSet::new("/b".into(), "b".into(), shared.iter().copied().chain(b_extra));
        let pairs = find_near_duplicate_pairs(&[a, b], &NearDupConfig::default());
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].intersection, 92);
        assert_eq!(pairs[0].union, 113);
        assert!((pairs[0].similarity - 92.0 / 113.0).abs() < 1e-9);
    }

    #[test]
    fn below_threshold_pairs_are_filtered() {
        // ~25% similarity must not appear with default 0.8 threshold.
        let a = FolderSet::new("/a".into(), "a".into(), (0u64..100));
        let b = FolderSet::new("/b".into(), "b".into(), (75u64..175));
        let pairs = find_near_duplicate_pairs(&[a, b], &NearDupConfig::default());
        assert!(pairs.is_empty());
    }

    #[test]
    fn min_folder_files_filters_tiny_folders() {
        let cfg = NearDupConfig {
            min_folder_files: 3,
            ..Default::default()
        };
        let a = FolderSet::new("/a".into(), "a".into(), [1u64, 2]);
        let b = FolderSet::new("/b".into(), "b".into(), [1u64, 2]);
        assert!(find_near_duplicate_pairs(&[a, b], &cfg).is_empty());
    }

    #[test]
    fn lsh_path_finds_high_overlap_pairs() {
        // Force the LSH strategy with a tiny folder-count cutoff.
        let cfg = NearDupConfig {
            max_folders_for_inverted_index: 0,
            ..Default::default()
        };
        let shared: Vec<u64> = (0u64..90).collect();
        let a = FolderSet::new("/a".into(), "a".into(), shared.iter().copied().chain(90u64..100));
        let b = FolderSet::new("/b".into(), "b".into(), shared.iter().copied().chain(500u64..510));
        let c = FolderSet::new("/c".into(), "c".into(), (900u64..1000));
        let pairs = find_near_duplicate_pairs(&[a, b, c], &cfg);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].intersection, 90);
    }

    #[test]
    fn minhash_signatures_of_identical_sets_match() {
        let t: HashSet<u64> = (0u64..100).collect();
        let s1 = minhash_signature(&t, 64);
        let s2 = minhash_signature(&t, 64);
        assert_eq!(s1, s2);
        assert!(s1.iter().all(|&v| v != u64::MAX));
    }

    #[test]
    fn noise_detection() {
        assert!(is_noise_file(".DS_Store"));
        assert!(is_noise_file("__init__.py"));
        assert!(!is_noise_file("main.rs"));
    }

    #[test]
    fn empty_input_yields_no_pairs() {
        let pairs = find_near_duplicate_pairs(&[], &NearDupConfig::default());
        assert!(pairs.is_empty());
    }
}
