use seiri_appeal::{evaluate_claim_capability_membrane, IncrementalMembraneState};
use seiri_core::{
    AppealIrError, CapabilityNodeId, CapabilitySemanticSignature, CapabilitySupport, ClaimAtomIR,
    ClaimPolarity, ProfileKind, ReadmeGrammarIR, RepositoryCapabilityIR, ValueDimension,
};
use std::num::NonZeroU32;

#[test]
fn malformed_public_grammar_returns_typed_error_without_panicking() {
    let grammar = ReadmeGrammarIR {
        semantic_revision: "tampered".to_string(),
        ..ReadmeGrammarIR::default()
    };
    let capabilities = RepositoryCapabilityIR::default();

    let call = std::panic::catch_unwind(|| {
        evaluate_claim_capability_membrane(&grammar, &capabilities, ProfileKind::Common)
    });
    assert!(call.is_ok(), "checked membrane boundary must not panic");
    assert_eq!(
        call.expect("no panic"),
        Err(AppealIrError::ReadmeGrammarSemanticRevisionMismatch)
    );
}

#[test]
fn dangling_signature_owner_returns_typed_error_without_panicking() {
    let grammar = ReadmeGrammarIR::default();
    let mut capabilities = RepositoryCapabilityIR::default();
    capabilities
        .semantic_signatures
        .push(CapabilitySemanticSignature {
            capability_node: CapabilityNodeId::new(NonZeroU32::MIN),
            dimension: ValueDimension::Capability,
            subject: Some("reposeiri".to_string()),
            action: Some("audit".to_string()),
            object: Some("repository".to_string()),
            qualifiers: Vec::new(),
            polarity: ClaimPolarity::Positive,
            input_nodes: Vec::new(),
            output_nodes: Vec::new(),
            condition_nodes: Vec::new(),
            semantic_support: CapabilitySupport::Inferred,
        });

    let call = std::panic::catch_unwind(|| {
        evaluate_claim_capability_membrane(&grammar, &capabilities, ProfileKind::Common)
    });
    assert!(call.is_ok(), "checked membrane boundary must not panic");
    assert_eq!(
        call.expect("no panic"),
        Err(AppealIrError::DanglingCapabilitySemanticSignatureOwner(
            CapabilityNodeId::new(NonZeroU32::MIN)
        ))
    );
}

#[test]
fn incremental_entry_point_reuses_the_same_checked_boundary() {
    let grammar = ReadmeGrammarIR {
        claim_atoms: ClaimAtomIR {
            semantic_revision: "tampered".to_string(),
            ..ClaimAtomIR::default()
        },
        ..ReadmeGrammarIR::default()
    };
    let result = IncrementalMembraneState::from_scalar(
        &grammar,
        &RepositoryCapabilityIR::default(),
        ProfileKind::Common,
    );
    assert_eq!(
        result,
        Err(AppealIrError::ClaimAtomSemanticRevisionMismatch)
    );
}
