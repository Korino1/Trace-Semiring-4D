//! First-class physical class layer for sections 20-22 of the concept.

use crate::algorithms::{odot_kappa, otimes_kappa, parallel_kappa};
use crate::trace::Trace;
use crate::types::Block;

/// Returns `true` when every block of `trace` satisfies the physical limit `|B_i|_1 <= kappa`.
#[inline]
pub fn is_kappa_admissible(trace: &Trace, kappa: u32) -> bool {
    assert!(kappa >= 1, "is_kappa_admissible requires kappa >= 1");
    trace.all_blocks_l1_le(kappa)
}

/// Returns `true` when every block of `trace` satisfies the tight core bound `|B_i|_1 <= kappa/2`.
#[inline]
pub fn is_tight_core(trace: &Trace, kappa: u32) -> bool {
    assert!(kappa >= 1, "is_tight_core requires kappa >= 1");
    let tight_bound = kappa / 2;
    trace.all_blocks_l1_le(tight_bound)
}

/// First-class representative of the synchronous physical class `C`.
///
/// The wrapped trace is always `kappa`-admissible (`|B_i|_1 <= kappa` for every layer).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncTrace {
    trace: Trace,
    kappa: u32,
}

impl SyncTrace {
    #[inline]
    fn single_step(block: Block, kappa: u32) -> Self {
        Self::new(Trace::new(vec![block]), kappa)
            .expect("SyncTrace::single_step must be kappa-admissible")
    }

    #[inline]
    fn temporal_tick(kappa: u32) -> Self {
        Self::new(Trace::tau(1), kappa).expect("SyncTrace::temporal_tick must be kappa-admissible")
    }

    /// Constructs a synchronous physical trace if the given trace is `kappa`-admissible.
    #[inline]
    pub fn new(trace: Trace, kappa: u32) -> Option<Self> {
        assert!(kappa >= 1, "SyncTrace::new requires kappa >= 1");
        if is_kappa_admissible(&trace, kappa) {
            Some(Self { trace, kappa })
        } else {
            None
        }
    }

    /// Canonical zero object `0_L` for the synchronous class with `layers` time layers.
    #[inline]
    pub fn zero(layers: usize, kappa: u32) -> Self {
        assert!(kappa >= 1, "SyncTrace::zero requires kappa >= 1");
        assert!(layers >= 1, "SyncTrace::zero requires layers >= 1");
        Self {
            trace: Trace::tau(layers - 1),
            kappa,
        }
    }

    /// Underlying physical trace.
    #[inline]
    pub fn trace(&self) -> &Trace {
        &self.trace
    }

    /// Consume and return the underlying trace.
    #[inline]
    pub fn into_trace(self) -> Trace {
        self.trace
    }

    /// Fixed physical limit `kappa`.
    #[inline]
    pub fn kappa(&self) -> u32 {
        self.kappa
    }

    /// Number of layers in the current synchronous trace.
    #[inline]
    pub fn layers(&self) -> usize {
        self.trace.len_blocks()
    }

    /// Returns `true` when the trace belongs to the tight core `C_core`.
    #[inline]
    pub fn is_tight_core(&self) -> bool {
        is_tight_core(&self.trace, self.kappa)
    }

    /// Parallel composition on the synchronous class without padding.
    ///
    /// Inputs must live on the same time grid (`layers()` must match).
    #[inline]
    pub fn boxplus(&self, other: &Self) -> Self {
        assert_eq!(
            self.kappa, other.kappa,
            "SyncTrace::boxplus requires matching kappa"
        );
        assert_eq!(
            self.layers(),
            other.layers(),
            "SyncTrace::boxplus requires matching layers"
        );
        let trace = parallel_kappa(&self.trace, &other.trace, self.kappa);
        Self::new(trace, self.kappa).expect("SyncTrace::boxplus must stay kappa-admissible")
    }

    /// Tight-core parallel composition with direct layer-wise addition and no split.
    #[inline]
    pub fn boxplus_tight(&self, other: &Self) -> Self {
        assert_eq!(
            self.kappa, other.kappa,
            "SyncTrace::boxplus_tight requires matching kappa"
        );
        assert_eq!(
            self.layers(),
            other.layers(),
            "SyncTrace::boxplus_tight requires matching layers"
        );
        assert!(
            self.is_tight_core(),
            "SyncTrace::boxplus_tight requires tight-core lhs"
        );
        assert!(
            other.is_tight_core(),
            "SyncTrace::boxplus_tight requires tight-core rhs"
        );

        let trace = self
            .trace
            .try_parallel_tight(&other.trace, self.kappa)
            .expect("SyncTrace::boxplus_tight requires split-free layer sums");

        Self::new(trace, self.kappa)
            .expect("SyncTrace::boxplus_tight must stay kappa-admissible")
    }

    /// Sequential physical composition inside the synchronous class.
    #[inline]
    pub fn sequential(&self, other: &Self) -> Self {
        assert_eq!(
            self.kappa, other.kappa,
            "SyncTrace::sequential requires matching kappa"
        );
        let trace = odot_kappa(&self.trace, &other.trace, self.kappa);
        Self::new(trace, self.kappa).expect("SyncTrace::sequential must stay kappa-admissible")
    }

    /// Time refinement (`otimes_kappa`) inside the synchronous class.
    #[inline]
    pub fn time_refine(&self, other: &Self) -> Self {
        assert_eq!(
            self.kappa, other.kappa,
            "SyncTrace::time_refine requires matching kappa"
        );
        let trace = otimes_kappa(&self.trace, &other.trace, self.kappa);
        Self::new(trace, self.kappa).expect("SyncTrace::time_refine must stay kappa-admissible")
    }

    /// Physical temporal successor `S_T^(kappa)`.
    #[inline]
    pub fn successor_t(&self) -> Self {
        self.sequential(&Self::temporal_tick(self.kappa))
    }

    /// Physical spatial successor `S_X^(kappa)`.
    #[inline]
    pub fn successor_x(&self) -> Self {
        self.sequential(&Self::single_step(Block::new(1, 0, 0), self.kappa))
    }

    /// Physical spatial successor `S_Y^(kappa)`.
    #[inline]
    pub fn successor_y(&self) -> Self {
        self.sequential(&Self::single_step(Block::new(0, 1, 0), self.kappa))
    }

    /// Physical spatial successor `S_Z^(kappa)`.
    #[inline]
    pub fn successor_z(&self) -> Self {
        self.sequential(&Self::single_step(Block::new(0, 0, 1), self.kappa))
    }
}
