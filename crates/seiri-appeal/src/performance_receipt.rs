use crate::{IncrementalEquivalence, IncrementalMembraneUpdate, IncrementalUpdateMode};
use seiri_core::ValueDimension;
use seiri_digest::{Digest32, StableHasher};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

pub const PERFORMANCE_RECEIPT_REVISION: &str = "seiri.appeal-performance-receipt.v1";
const RECEIPT_BOUNDARY: &str = "This receipt binds a local scalar/incremental equivalence check and observed elapsed samples. Timing values are observations only: no timing threshold is applied, and the receipt establishes neither general performance nor an improvement claim.";
const MAX_SAMPLES: usize = 10_000;
const MAX_COMMAND_PARTS: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformanceReceiptContext {
    target: String,
    environment: String,
    corpus: String,
    command: Vec<String>,
    sample_count: usize,
}

impl PerformanceReceiptContext {
    pub fn try_new(
        target: impl Into<String>,
        environment: impl Into<String>,
        corpus: impl Into<String>,
        command: Vec<String>,
        sample_count: usize,
    ) -> Result<Self, PerformanceReceiptError> {
        let value = Self {
            target: target.into(),
            environment: environment.into(),
            corpus: corpus.into(),
            command,
            sample_count,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<(), PerformanceReceiptError> {
        if !portable_label(&self.target, 128)
            || !portable_label(&self.environment, 128)
            || !portable_label(&self.corpus, 256)
        {
            return Err(PerformanceReceiptError::InvalidContextLabel);
        }
        if self.sample_count == 0 || self.sample_count > MAX_SAMPLES {
            return Err(PerformanceReceiptError::InvalidSampleCount);
        }
        if self.command.is_empty()
            || self.command.len() > MAX_COMMAND_PARTS
            || self.command.iter().any(|part| {
                part.is_empty()
                    || part.len() > 512
                    || part.trim() != part
                    || part.chars().any(|character| {
                        character == '\0' || character == '\r' || character == '\n'
                    })
            })
        {
            return Err(PerformanceReceiptError::InvalidCommand);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerformanceUpdateMode {
    Reused,
    Sparse,
    ScalarRebuild,
}

impl From<IncrementalUpdateMode> for PerformanceUpdateMode {
    fn from(value: IncrementalUpdateMode) -> Self {
        match value {
            IncrementalUpdateMode::Reused => Self::Reused,
            IncrementalUpdateMode::Sparse => Self::Sparse,
            IncrementalUpdateMode::ScalarRebuild => Self::ScalarRebuild,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "PerformanceReceiptWire")]
pub struct DeterministicPerformanceReceipt {
    pub semantic_revision: String,
    pub target: String,
    pub environment: String,
    pub corpus: String,
    pub command: Vec<String>,
    pub sample_count: usize,
    pub observed_elapsed_micros: Vec<u64>,
    pub update_mode: PerformanceUpdateMode,
    pub frontier: Vec<ValueDimension>,
    pub reused_dimensions: usize,
    pub incremental_digest: Digest32,
    pub scalar_digest: Digest32,
    pub equivalent: bool,
    pub timing_gate_applied: bool,
    pub semantic_receipt_digest: Digest32,
    pub observation_digest: Digest32,
    pub boundary: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PerformanceReceiptWire {
    semantic_revision: String,
    target: String,
    environment: String,
    corpus: String,
    command: Vec<String>,
    sample_count: usize,
    observed_elapsed_micros: Vec<u64>,
    update_mode: PerformanceUpdateMode,
    frontier: Vec<ValueDimension>,
    reused_dimensions: usize,
    incremental_digest: Digest32,
    scalar_digest: Digest32,
    equivalent: bool,
    timing_gate_applied: bool,
    semantic_receipt_digest: Digest32,
    observation_digest: Digest32,
    boundary: String,
}

impl TryFrom<PerformanceReceiptWire> for DeterministicPerformanceReceipt {
    type Error = PerformanceReceiptError;

    fn try_from(value: PerformanceReceiptWire) -> Result<Self, Self::Error> {
        let receipt = Self {
            semantic_revision: value.semantic_revision,
            target: value.target,
            environment: value.environment,
            corpus: value.corpus,
            command: value.command,
            sample_count: value.sample_count,
            observed_elapsed_micros: value.observed_elapsed_micros,
            update_mode: value.update_mode,
            frontier: value.frontier,
            reused_dimensions: value.reused_dimensions,
            incremental_digest: value.incremental_digest,
            scalar_digest: value.scalar_digest,
            equivalent: value.equivalent,
            timing_gate_applied: value.timing_gate_applied,
            semantic_receipt_digest: value.semantic_receipt_digest,
            observation_digest: value.observation_digest,
            boundary: value.boundary,
        };
        receipt.validate()?;
        Ok(receipt)
    }
}

impl DeterministicPerformanceReceipt {
    pub fn try_new(
        context: PerformanceReceiptContext,
        update: &IncrementalMembraneUpdate,
        equivalence: IncrementalEquivalence,
        observed_elapsed_micros: Vec<u64>,
    ) -> Result<Self, PerformanceReceiptError> {
        context.validate()?;
        if !equivalence.equivalent
            || equivalence.incremental_digest != equivalence.scalar_digest
            || equivalence.incremental_digest != update.state.semantic_digest()
        {
            return Err(PerformanceReceiptError::ScalarIncrementalDivergence);
        }
        if observed_elapsed_micros.len() != context.sample_count {
            return Err(PerformanceReceiptError::ObservationCountMismatch);
        }
        let update_mode = PerformanceUpdateMode::from(update.mode);
        let semantic_receipt_digest = semantic_receipt_digest(
            &context,
            update_mode,
            &update.frontier,
            update.reused_dimensions,
            equivalence,
        );
        let observation_digest =
            observation_digest(semantic_receipt_digest, &observed_elapsed_micros);
        let receipt = Self {
            semantic_revision: PERFORMANCE_RECEIPT_REVISION.to_string(),
            target: context.target,
            environment: context.environment,
            corpus: context.corpus,
            command: context.command,
            sample_count: context.sample_count,
            observed_elapsed_micros,
            update_mode,
            frontier: update.frontier.clone(),
            reused_dimensions: update.reused_dimensions,
            incremental_digest: equivalence.incremental_digest,
            scalar_digest: equivalence.scalar_digest,
            equivalent: true,
            timing_gate_applied: false,
            semantic_receipt_digest,
            observation_digest,
            boundary: RECEIPT_BOUNDARY.to_string(),
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn validate(&self) -> Result<(), PerformanceReceiptError> {
        if self.semantic_revision != PERFORMANCE_RECEIPT_REVISION {
            return Err(PerformanceReceiptError::SemanticRevisionMismatch);
        }
        let context = PerformanceReceiptContext::try_new(
            self.target.clone(),
            self.environment.clone(),
            self.corpus.clone(),
            self.command.clone(),
            self.sample_count,
        )?;
        if self.observed_elapsed_micros.len() != self.sample_count {
            return Err(PerformanceReceiptError::ObservationCountMismatch);
        }
        if self.frontier.windows(2).any(|pair| pair[0] >= pair[1])
            || self.frontier.len().saturating_add(self.reused_dimensions)
                != ValueDimension::ALL.len()
        {
            return Err(PerformanceReceiptError::NonCanonicalFrontier);
        }
        if !self.equivalent || self.incremental_digest != self.scalar_digest {
            return Err(PerformanceReceiptError::ScalarIncrementalDivergence);
        }
        if self.timing_gate_applied {
            return Err(PerformanceReceiptError::TimingGateForbidden);
        }
        if self.boundary != RECEIPT_BOUNDARY {
            return Err(PerformanceReceiptError::BoundaryMismatch);
        }
        let equivalence = IncrementalEquivalence {
            incremental_digest: self.incremental_digest,
            scalar_digest: self.scalar_digest,
            equivalent: self.equivalent,
        };
        let semantic = semantic_receipt_digest(
            &context,
            self.update_mode,
            &self.frontier,
            self.reused_dimensions,
            equivalence,
        );
        if semantic != self.semantic_receipt_digest {
            return Err(PerformanceReceiptError::DigestMismatch);
        }
        if observation_digest(semantic, &self.observed_elapsed_micros) != self.observation_digest {
            return Err(PerformanceReceiptError::DigestMismatch);
        }
        Ok(())
    }
}

fn semantic_receipt_digest(
    context: &PerformanceReceiptContext,
    update_mode: PerformanceUpdateMode,
    frontier: &[ValueDimension],
    reused_dimensions: usize,
    equivalence: IncrementalEquivalence,
) -> Digest32 {
    let mut hasher = StableHasher::new(b"seiri.appeal-performance-receipt.semantic.v1", 11);
    hasher
        .str(1, PERFORMANCE_RECEIPT_REVISION)
        .str(2, &context.target)
        .str(3, &context.environment)
        .str(4, &context.corpus)
        .usize(5, context.sample_count);
    for part in &context.command {
        hasher.str(6, part);
    }
    hasher
        .u8(7, update_mode_rank(update_mode))
        .usize(8, reused_dimensions)
        .digest(9, equivalence.incremental_digest)
        .digest(10, equivalence.scalar_digest);
    for dimension in frontier {
        hasher.u8(11, dimension_rank(*dimension));
    }
    hasher.finish()
}

fn observation_digest(semantic: Digest32, observed_elapsed_micros: &[u64]) -> Digest32 {
    let mut hasher = StableHasher::new(b"seiri.appeal-performance-receipt.observation.v1", 2);
    hasher.digest(1, semantic);
    for elapsed in observed_elapsed_micros {
        hasher.u64(2, *elapsed);
    }
    hasher.finish()
}

fn portable_label(value: &str, max_len: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_len
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':' | b'/')
        })
        && !value.starts_with('/')
        && !value.contains("..")
}

const fn update_mode_rank(mode: PerformanceUpdateMode) -> u8 {
    match mode {
        PerformanceUpdateMode::Reused => 0,
        PerformanceUpdateMode::Sparse => 1,
        PerformanceUpdateMode::ScalarRebuild => 2,
    }
}

const fn dimension_rank(dimension: ValueDimension) -> u8 {
    match dimension {
        ValueDimension::Identity => 0,
        ValueDimension::Audience => 1,
        ValueDimension::Problem => 2,
        ValueDimension::Capability => 3,
        ValueDimension::Outcome => 4,
        ValueDimension::FirstAction => 5,
        ValueDimension::FirstResult => 6,
        ValueDimension::Evidence => 7,
        ValueDimension::Constraint => 8,
        ValueDimension::Differentiation => 9,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceReceiptError {
    InvalidContextLabel,
    InvalidCommand,
    InvalidSampleCount,
    ObservationCountMismatch,
    NonCanonicalFrontier,
    ScalarIncrementalDivergence,
    TimingGateForbidden,
    SemanticRevisionMismatch,
    BoundaryMismatch,
    DigestMismatch,
}

impl Display for PerformanceReceiptError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::InvalidContextLabel => "performance receipt context label is not portable",
            Self::InvalidCommand => "performance receipt command is invalid or unbounded",
            Self::InvalidSampleCount => "performance receipt sample count is invalid or unbounded",
            Self::ObservationCountMismatch => {
                "performance receipt observation count does not match sample count"
            }
            Self::NonCanonicalFrontier => "performance receipt frontier is not canonical",
            Self::ScalarIncrementalDivergence => {
                "performance receipt scalar and incremental results diverge"
            }
            Self::TimingGateForbidden => "performance receipt must not apply a timing gate",
            Self::SemanticRevisionMismatch => {
                "performance receipt semantic revision is unsupported"
            }
            Self::BoundaryMismatch => "performance receipt claim boundary is invalid",
            Self::DigestMismatch => "performance receipt digest does not match its fields",
        })
    }
}

impl std::error::Error for PerformanceReceiptError {}
