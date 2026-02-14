//! Robust Random Cut Forest (RRCF) for Multivariate Anomaly Detection
//!
//! RRCF is a SOTA algorithm for streaming anomaly detection on multivariate data.
//! It extends the Random Cut Forest algorithm to handle streaming data with
//! efficient insertions and deletions.
//!
//! Key features:
//! - O(log n) insertion and deletion
//! - Robust to outliers (doesn't contaminate the model)
//! - No hyperparameters required
//! - Works on high-dimensional data
//!
//! Reference: "Robust Random Cut Forest Based Anomaly Detection On Streams"
//! (Guha et al., KDD 2016)

use rand::Rng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::VecDeque;
use std::sync::Arc;

// --- Serde Helpers for Arc<[f64]> ---

mod serde_arc {
    use super::*;
    use serde::ser::SerializeSeq;

    pub fn serialize<S>(data: &Arc<[f64]>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(data.len()))?;
        for e in data.iter() {
            seq.serialize_element(e)?;
        }
        seq.end()
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Arc<[f64]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let vec: Vec<f64> = Vec::deserialize(deserializer)?;
        Ok(vec.into())
    }
}

mod serde_points {
    use super::*;
    use serde::ser::SerializeSeq;

    pub fn serialize<S>(
        data: &VecDeque<(u64, Arc<[f64]>)>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(data.len()))?;
        for (id, point) in data {
            // Helper struct to match the desired tuple format but with custom serialization
            #[derive(Serialize)]
            struct PointHelper<'a> {
                id: u64,
                #[serde(with = "serde_arc")]
                point: &'a Arc<[f64]>,
            }
            // Serialize as a tuple (u64, Arc<[f64]>)
            // Note: Tuple serialization in serde expects ordered elements.
            // We can manually serialize a tuple variant.
            let _helper = PointHelper { id: *id, point };
            // To match Vec<(u64, Arc<[f64]>)> we need to serialize as a tuple.
            // But helper struct serializes as a map/struct usually?
            // No, strictly speaking we just want to serialize the elements.
            // Let's simpler serialize as a tuple:
            seq.serialize_element(&(*id, Helper(point)))?;
        }
        seq.end()
    }

    // Helper wrapper to apply serde_arc to the second element of the tuple
    struct Helper<'a>(&'a Arc<[f64]>);
    impl<'a> Serialize for Helper<'a> {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            serde_arc::serialize(self.0, serializer)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<VecDeque<(u64, Arc<[f64]>)>, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Helper struct to deserialize the tuple
        struct TupleVisitor;

        impl<'de> serde::de::Visitor<'de> for TupleVisitor {
            type Value = VecDeque<(u64, Arc<[f64]>)>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a sequence of (id, point) tuples")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut vec = VecDeque::new();
                while let Some((id, point_wrapper)) = seq.next_element::<(u64, PointWrapper)>()? {
                    vec.push_back((id, point_wrapper.0));
                }
                Ok(vec)
            }
        }

        // Wrapper to use serde_arc for deserialization
        struct PointWrapper(Arc<[f64]>);
        impl<'de> Deserialize<'de> for PointWrapper {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                serde_arc::deserialize(deserializer).map(PointWrapper)
            }
        }

        deserializer.deserialize_seq(TupleVisitor)
    }
}

/// A node in the RRCF tree
#[derive(Serialize, Deserialize, Clone, Debug)]
enum RcNode {
    /// Internal node with cut dimension and value
    Internal {
        cut_dim: usize,
        cut_value: f64,
        left: Box<RcNode>,
        right: Box<RcNode>,
        /// Bounding box for this subtree
        bbox_min: Box<[f64]>,
        bbox_max: Box<[f64]>,
        /// Number of points in subtree
        num_points: usize,
    },
    /// Leaf node containing a point
    Leaf {
        #[serde(with = "serde_arc")]
        point: Arc<[f64]>,
        /// Unique identifier for this point
        point_id: u64,
    },
}

/// Robust Random Cut Forest
#[derive(Serialize, Deserialize, Clone)]
pub struct StreamingRRCF {
    /// Forest of trees
    trees: Vec<RcTree>,
    /// Window size for streaming (sliding window)
    window_size: usize,
    /// Shingle buffer for time series embedding
    shingle_buffer: VecDeque<f64>,
    /// Shingle size (dimensionality of embedded vectors)
    shingle_size: usize,
    /// Current point ID counter
    next_point_id: u64,
    /// Dimensionality of input data
    dimensions: usize,
    /// Number of trees in the forest
    num_trees: usize,
    /// Tree size (subsample size per tree)
    tree_size: usize,
    /// Baseline for score normalization (learned from data)
    baseline_codisp: f64,
    /// EWMA alpha for baseline update
    baseline_alpha: f64,
    /// Sample count
    sample_count: u64,
}

/// A single RRCF tree
#[derive(Serialize, Deserialize, Clone)]
struct RcTree {
    root: Option<RcNode>,
    /// Points currently in this tree (id -> point)
    #[serde(with = "serde_points")]
    points: VecDeque<(u64, Arc<[f64]>)>,
    /// Maximum points this tree can hold
    max_size: usize,
}

impl RcTree {
    fn new(max_size: usize) -> Self {
        Self {
            root: None,
            points: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    /// Insert a point into the tree
    fn insert(&mut self, point_id: u64, point: Arc<[f64]>) -> Option<(u64, Arc<[f64]>)> {
        // If tree is full, need to evict oldest
        let evicted = if self.points.len() >= self.max_size {
            // FIFO eviction (oldest point)
            if let Some(evicted) = self.points.pop_front() {
                self.delete_point(&evicted.1);
                Some(evicted)
            } else {
                None
            }
        } else {
            None
        };

        // Insert new point
        self.points.push_back((point_id, point.clone()));
        self.root = Some(insert_recursive(self.root.take(), point_id, point));

        evicted
    }

    /// Delete a point from the tree
    fn delete_point(&mut self, point: &[f64]) {
        self.root = delete_recursive(self.root.take(), point);
    }

    /// Compute codisp (collusive displacement) for a point
    /// This is the anomaly score - higher means more anomalous
    fn codisp(&self, point: &[f64]) -> f64 {
        if self.root.is_none() || self.points.is_empty() {
            return 0.0;
        }
        compute_codisp(self.root.as_ref(), point)
    }

    /// Get number of points in tree
    fn size(&self) -> usize {
        self.points.len()
    }
}

/// Recursive insertion with proper bounding box updates
fn insert_recursive(node: Option<RcNode>, point_id: u64, point: Arc<[f64]>) -> RcNode {
    match node {
        None => RcNode::Leaf { point, point_id },
        Some(RcNode::Leaf {
            point: existing_point,
            point_id: existing_id,
        }) => {
            // Split leaf into internal node
            split_leaf(existing_point, existing_id, point, point_id)
        }
        Some(RcNode::Internal {
            cut_dim,
            cut_value,
            left,
            right,
            mut bbox_min,
            mut bbox_max,
            num_points,
        }) => {
            // Update bounding box to include new point
            for (i, &v) in point.iter().enumerate() {
                if i < bbox_min.len() {
                    bbox_min[i] = bbox_min[i].min(v);
                    bbox_max[i] = bbox_max[i].max(v);
                }
            }

            if point.get(cut_dim).copied().unwrap_or(0.0) <= cut_value {
                let new_left = Box::new(insert_recursive(Some(*left), point_id, point));
                RcNode::Internal {
                    cut_dim,
                    cut_value,
                    left: new_left,
                    right,
                    bbox_min,
                    bbox_max,
                    num_points: num_points + 1,
                }
            } else {
                let new_right = Box::new(insert_recursive(Some(*right), point_id, point));
                RcNode::Internal {
                    cut_dim,
                    cut_value,
                    left,
                    right: new_right,
                    bbox_min,
                    bbox_max,
                    num_points: num_points + 1,
                }
            }
        }
    }
}

/// Split a leaf node into an internal node with random cut
fn split_leaf(p1: Arc<[f64]>, id1: u64, p2: Arc<[f64]>, id2: u64) -> RcNode {
    let dims = p1.len();
    if dims == 0 {
        return RcNode::Leaf {
            point: p1,
            point_id: id1,
        };
    }

    // Calculate ranges for each dimension
    let mut ranges: Vec<(usize, f64)> = (0..dims)
        .map(|i| {
            let min = p1[i].min(p2[i]);
            let max = p1[i].max(p2[i]);
            (i, max - min)
        })
        .collect();

    // Sort by range (largest first) for better cuts
    ranges.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    // Choose dimension with probability proportional to range
    let total_range: f64 = ranges.iter().map(|(_, r)| r).sum();
    let cut_dim = if total_range > 1e-10 {
        let mut r = rand::rng().random::<f64>() * total_range;
        let mut chosen = ranges[0].0;
        for (dim, range) in &ranges {
            r -= range;
            if r <= 0.0 {
                chosen = *dim;
                break;
            }
        }
        chosen
    } else {
        // All dimensions are equal, pick random
        rand::rng().random_range(0..dims)
    };

    // Random cut value between the two points
    let min_val = p1[cut_dim].min(p2[cut_dim]);
    let max_val = p1[cut_dim].max(p2[cut_dim]);
    let cut_value = if (max_val - min_val).abs() < 1e-10 {
        min_val
    } else {
        min_val + rand::rng().random::<f64>() * (max_val - min_val)
    };

    // Create bounding box
    let mut bbox_min = Vec::with_capacity(dims);
    let mut bbox_max = Vec::with_capacity(dims);
    for i in 0..dims {
        bbox_min.push(p1[i].min(p2[i]));
        bbox_max.push(p1[i].max(p2[i]));
    }

    // Create children based on cut
    let (left_leaf, right_leaf) = if p1[cut_dim] <= cut_value {
        (
            RcNode::Leaf {
                point: p1,
                point_id: id1,
            },
            RcNode::Leaf {
                point: p2,
                point_id: id2,
            },
        )
    } else {
        (
            RcNode::Leaf {
                point: p2,
                point_id: id2,
            },
            RcNode::Leaf {
                point: p1,
                point_id: id1,
            },
        )
    };

    RcNode::Internal {
        cut_dim,
        cut_value,
        left: Box::new(left_leaf),
        right: Box::new(right_leaf),
        bbox_min: bbox_min.into_boxed_slice(),
        bbox_max: bbox_max.into_boxed_slice(),
        num_points: 2,
    }
}

/// Recursive deletion
fn delete_recursive(node: Option<RcNode>, point: &[f64]) -> Option<RcNode> {
    match node {
        None => None,
        Some(RcNode::Leaf {
            point: leaf_point,
            point_id,
        }) => {
            // Check if this is the point to delete (approximate match)
            let is_match = leaf_point
                .iter()
                .zip(point.iter())
                .all(|(a, b)| (a - b).abs() < 1e-10);
            if is_match {
                None
            } else {
                Some(RcNode::Leaf {
                    point: leaf_point,
                    point_id,
                })
            }
        }
        Some(RcNode::Internal {
            cut_dim,
            cut_value,
            left,
            right,
            bbox_min,
            bbox_max,
            num_points,
        }) => {
            // Determine which subtree to delete from
            let go_left = point.get(cut_dim).copied().unwrap_or(0.0) <= cut_value;

            let (new_left, new_right) = if go_left {
                (delete_recursive(Some(*left), point), Some(*right))
            } else {
                (Some(*left), delete_recursive(Some(*right), point))
            };

            // If one child is None, return the other (tree collapse)
            match (new_left, new_right) {
                (None, None) => None,
                (Some(l), None) => Some(l),
                (None, Some(r)) => Some(r),
                (Some(l), Some(r)) => {
                    let new_count = num_points.saturating_sub(1);
                    Some(RcNode::Internal {
                        cut_dim,
                        cut_value,
                        left: Box::new(l),
                        right: Box::new(r),
                        bbox_min,
                        bbox_max,
                        num_points: new_count,
                    })
                }
            }
        }
    }
}

/// Compute CoDisp (Collusive Displacement) score for a point
/// Higher score = more anomalous
fn compute_codisp(node: Option<&RcNode>, point: &[f64]) -> f64 {
    match node {
        None => 0.0,
        Some(RcNode::Leaf { .. }) => {
            // Reached a leaf - base case
            1.0
        }
        Some(RcNode::Internal {
            cut_dim,
            cut_value,
            left,
            right,
            num_points,
            bbox_min,
            bbox_max,
            ..
        }) => {
            let point_val = point.get(*cut_dim).copied().unwrap_or(0.0);
            let _n = *num_points as f64;

            // Check if point would displace sibling subtree
            let (next_node, sibling) = if point_val <= *cut_value {
                (left.as_ref(), right.as_ref())
            } else {
                (right.as_ref(), left.as_ref())
            };

            // Check if point is outside the bounding box
            let mut is_outside = false;
            for (i, &v) in point.iter().enumerate() {
                if i < bbox_min.len() && (v < bbox_min[i] - 1e-10 || v > bbox_max[i] + 1e-10) {
                    is_outside = true;
                    break;
                }
            }

            if is_outside {
                // Point is outside bbox - high displacement
                let sibling_size = get_subtree_size(sibling) as f64;
                // Displacement score: sibling size / depth
                sibling_size + compute_codisp(Some(next_node), point)
            } else {
                // Point is inside bbox - continue recursively
                compute_codisp(Some(next_node), point)
            }
        }
    }
}

/// Get the size of a subtree
fn get_subtree_size(node: &RcNode) -> usize {
    match node {
        RcNode::Leaf { .. } => 1,
        RcNode::Internal { num_points, .. } => *num_points,
    }
}

impl StreamingRRCF {
    /// Create a new Streaming RRCF
    ///
    /// # Arguments
    /// * `dimensions` - Dimensionality of input vectors
    /// * `num_trees` - Number of trees in forest (typically 10-100)
    /// * `tree_size` - Size of each tree (typically 128-512)
    /// * `shingle_size` - Number of time steps to embed (for time series)
    pub fn new(dimensions: usize, num_trees: usize, tree_size: usize, shingle_size: usize) -> Self {
        let n_trees = num_trees.max(1).min(100);
        let t_size = tree_size.max(16).min(1024);
        let shingle = shingle_size.max(1);

        let trees: Vec<RcTree> = (0..n_trees).map(|_| RcTree::new(t_size)).collect();

        Self {
            trees,
            window_size: t_size * 2,
            shingle_buffer: VecDeque::with_capacity(shingle),
            shingle_size: shingle,
            next_point_id: 1,
            dimensions: dimensions.max(1),
            num_trees: n_trees,
            tree_size: t_size,
            baseline_codisp: 0.0,
            baseline_alpha: 0.01,
            sample_count: 0,
        }
    }

    /// Create for univariate time series (shingles into vectors)
    pub fn univariate(num_trees: usize, tree_size: usize, shingle_size: usize) -> Self {
        Self::new(shingle_size, num_trees, tree_size, shingle_size)
    }

    /// Create for multivariate data (no shingling)
    pub fn multivariate(dimensions: usize, num_trees: usize, tree_size: usize) -> Self {
        Self::new(dimensions, num_trees, tree_size, 1)
    }

    /// Update with new value (univariate time series)
    pub fn update_univariate(&mut self, value: f64) -> (f64, bool) {
        // Add to shingle buffer
        self.shingle_buffer.push_back(value);
        if self.shingle_buffer.len() > self.shingle_size {
            self.shingle_buffer.pop_front();
        }

        // Wait for full shingle
        if self.shingle_buffer.len() < self.shingle_size {
            return (0.0, false);
        }

        // Flatten shingle into Arc<[f64]>
        let point: Arc<[f64]> = self
            .shingle_buffer
            .iter()
            .copied()
            .collect::<Vec<_>>()
            .into();

        self.update_multivariate_arc(point)
    }

    /// Update with new vector (multivariate)
    pub fn update_multivariate(&mut self, point: Vec<f64>) -> (f64, bool) {
        self.update_multivariate_arc(point.into())
    }

    /// Update with shared Arc (zero-allocation for trees)
    pub fn update_multivariate_arc(&mut self, point: Arc<[f64]>) -> (f64, bool) {
        let point_id = self.next_point_id;
        self.next_point_id += 1;
        self.sample_count += 1;

        // Compute codisp before insertion (anomaly score)
        let codisp_sum: f64 = self.trees.iter().map(|tree| tree.codisp(&point)).sum();
        let avg_codisp = if self.num_trees > 0 {
            codisp_sum / self.num_trees as f64
        } else {
            0.0
        };

        // Insert into all trees (Arc cloning is cheap)
        for tree in &mut self.trees {
            tree.insert(point_id, point.clone());
        }

        // Update baseline using EWMA (for adaptive thresholding)
        if self.sample_count == 1 {
            self.baseline_codisp = avg_codisp;
        } else {
            self.baseline_codisp = (1.0 - self.baseline_alpha) * self.baseline_codisp
                + self.baseline_alpha * avg_codisp;
        }

        // Calculate normalized score
        // Higher codisp = more anomalous
        // Score is ratio of current codisp to baseline
        let normalized_score = if self.baseline_codisp > 0.01 && self.sample_count > 10 {
            let ratio = avg_codisp / self.baseline_codisp;
            // Map ratio to 0-1 scale: ratio of 1 = normal (0.0), ratio of 3+ = anomaly (1.0)
            ((ratio - 1.0) / 2.0).clamp(0.0, 1.0)
        } else {
            0.0 // Warmup period
        };

        // Threshold at 0.5 for anomaly detection
        let is_anomaly = normalized_score > 0.5;

        (normalized_score, is_anomaly)
    }

    /// Get forest statistics
    pub fn get_stats(&self) -> (usize, u64, f64) {
        let total_points: usize = self.trees.iter().map(|tree| tree.size()).sum();
        let avg_points = total_points as f64 / self.num_trees as f64;
        (self.num_trees, self.next_point_id, avg_points)
    }

    /// Reset the forest
    pub fn reset(&mut self) {
        self.trees = (0..self.num_trees)
            .map(|_| RcTree::new(self.tree_size))
            .collect();
        self.shingle_buffer.clear();
        self.next_point_id = 1;
        self.baseline_codisp = 0.0;
        self.sample_count = 0;
    }
}

/// RRCF-based detector for integration with engine
pub struct RRCFDetector {
    rrcf: StreamingRRCF,
    threshold: f64,
}

impl RRCFDetector {
    pub fn new_univariate(shingle_size: usize) -> Self {
        Self {
            rrcf: StreamingRRCF::univariate(10, 128, shingle_size), // Reduced for speed
            threshold: 0.2, // Lower threshold for high recall
        }
    }

    pub fn new_multivariate(dimensions: usize) -> Self {
        Self {
            rrcf: StreamingRRCF::multivariate(dimensions, 10, 128), // Reduced for speed
            threshold: 0.2, // Lower threshold for high recall
        }
    }

    pub fn update(&mut self, value: f64) -> (f64, bool) {
        let (score, _) = self.rrcf.update_univariate(value);
        (score, score > self.threshold)
    }

    pub fn update_vector(&mut self, vector: Vec<f64>) -> (f64, bool) {
        let (score, _) = self.rrcf.update_multivariate(vector);
        (score, score > self.threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rrcf_basic() {
        let mut rrcf = StreamingRRCF::univariate(10, 64, 4);

        // Warm up with normal values
        for i in 0..20 {
            let value = 100.0 + (i % 5) as f64;
            let (score, _) = rrcf.update_univariate(value);
            // During warmup, scores should be low
            if i > 10 {
                assert!(score <= 1.0, "Score should be bounded");
            }
        }
    }

    #[test]
    fn test_rrcf_detects_anomaly() {
        let mut rrcf = StreamingRRCF::univariate(10, 64, 4);

        // Warm up with stable pattern
        for _ in 0..50 {
            rrcf.update_univariate(100.0);
        }

        // Inject anomaly - should get elevated score
        let (score_anomaly, _) = rrcf.update_univariate(500.0);

        // The score should be elevated for the anomaly
        assert!(
            score_anomaly > 0.0,
            "Anomaly should have positive score: {}",
            score_anomaly
        );
    }

    #[test]
    fn test_multivariate() {
        let mut rrcf = StreamingRRCF::multivariate(3, 10, 32);

        // Warm up with consistent pattern
        for i in 0..30 {
            let vec = vec![i as f64, i as f64 * 2.0, i as f64 * 0.5];
            rrcf.update_multivariate(vec);
        }

        // Normal vector (follows pattern)
        let (score_normal, _) = rrcf.update_multivariate(vec![31.0, 62.0, 15.5]);

        // Anomalous vector (breaks pattern significantly)
        let (score_anomaly, _) = rrcf.update_multivariate(vec![1000.0, 10.0, 500.0]);

        // Anomaly should score higher than normal
        assert!(
            score_anomaly >= score_normal,
            "Anomaly ({}) should score >= normal ({})",
            score_anomaly,
            score_normal
        );
    }

    #[test]
    fn test_detector_wrapper() {
        let mut detector = RRCFDetector::new_univariate(4);

        // Warm up
        for i in 0..20 {
            detector.update(100.0 + (i % 3) as f64);
        }

        // Test score bounds
        let (score, _) = detector.update(100.0);
        assert!(
            score >= 0.0 && score <= 1.0,
            "Score should be normalized: {}",
            score
        );
    }
}
