//! Deterministic, configuration-driven model capability routing.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Capabilities are intentionally conservative: unspecified boolean capabilities
/// are treated as unsupported, and no vendor/model assumptions are embedded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CapabilityDescriptor {
    pub structured_output: bool,
    pub tool_calling: bool,
    pub multimodal: bool,
    pub context_window: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapabilityRequirements {
    pub structured_output: bool,
    pub tool_calling: bool,
    pub multimodal: bool,
    pub min_context_window: Option<u32>,
}

impl CapabilityDescriptor {
    pub fn satisfies(&self, required: &CapabilityRequirements) -> bool {
        (!required.structured_output || self.structured_output)
            && (!required.tool_calling || self.tool_calling)
            && (!required.multimodal || self.multimodal)
            && required
                .min_context_window
                .map(|minimum| self.context_window.is_some_and(|actual| actual >= minimum))
                .unwrap_or(true)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCandidate {
    /// Stable routing identifier, conventionally `provider/model`.
    pub id: String,
    pub capabilities: CapabilityDescriptor,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FallbackPolicy {
    /// Candidate IDs in exact fallback order. Unlisted candidates retain input order.
    pub order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteError {
    NoMatchingModel,
}

impl std::fmt::Display for RouteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoMatchingModel => write!(f, "no model satisfies the required capabilities"),
        }
    }
}

impl std::error::Error for RouteError {}

/// Returns a deterministic attempt chain. The preferred model remains first when
/// compatible; configured fallbacks follow, then remaining candidates in input order.
pub fn route_models<'a>(
    candidates: &'a [ModelCandidate],
    required: &CapabilityRequirements,
    preferred: Option<&str>,
    policy: &FallbackPolicy,
) -> Result<Vec<&'a ModelCandidate>, RouteError> {
    let compatible: Vec<_> = candidates
        .iter()
        .filter(|candidate| candidate.capabilities.satisfies(required))
        .collect();
    if compatible.is_empty() {
        return Err(RouteError::NoMatchingModel);
    }

    let fallback_rank: HashMap<&str, usize> = policy
        .order
        .iter()
        .enumerate()
        .map(|(rank, id)| (id.as_str(), rank))
        .collect();
    let input_rank: HashMap<&str, usize> = candidates
        .iter()
        .enumerate()
        .map(|(rank, candidate)| (candidate.id.as_str(), rank))
        .collect();

    let mut routed = compatible;
    routed.sort_by_key(|candidate| {
        if preferred == Some(candidate.id.as_str()) {
            (0, 0)
        } else if let Some(rank) = fallback_rank.get(candidate.id.as_str()) {
            (1, *rank)
        } else {
            (2, input_rank[candidate.id.as_str()])
        }
    });
    Ok(routed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, capabilities: CapabilityDescriptor) -> ModelCandidate {
        ModelCandidate {
            id: id.into(),
            capabilities,
        }
    }

    #[test]
    fn matches_all_required_capabilities() {
        let candidates = vec![
            candidate("plain", CapabilityDescriptor::default()),
            candidate(
                "tools",
                CapabilityDescriptor {
                    tool_calling: true,
                    context_window: Some(32_000),
                    ..Default::default()
                },
            ),
        ];
        let routed = route_models(
            &candidates,
            &CapabilityRequirements {
                tool_calling: true,
                min_context_window: Some(16_000),
                ..Default::default()
            },
            None,
            &FallbackPolicy::default(),
        )
        .unwrap();
        assert_eq!(
            routed
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            ["tools"]
        );
    }

    #[test]
    fn reports_no_match_for_unknown_or_insufficient_capability() {
        let candidates = vec![candidate("unknown", CapabilityDescriptor::default())];
        assert_eq!(
            route_models(
                &candidates,
                &CapabilityRequirements {
                    multimodal: true,
                    ..Default::default()
                },
                None,
                &FallbackPolicy::default()
            ),
            Err(RouteError::NoMatchingModel)
        );
    }

    #[test]
    fn preferred_then_configured_fallback_order_is_stable() {
        let capable = CapabilityDescriptor {
            structured_output: true,
            ..Default::default()
        };
        let candidates = vec![
            candidate("a", capable.clone()),
            candidate("b", capable.clone()),
            candidate("c", capable),
        ];
        let routed = route_models(
            &candidates,
            &CapabilityRequirements {
                structured_output: true,
                ..Default::default()
            },
            Some("b"),
            &FallbackPolicy {
                order: vec!["c".into(), "a".into()],
            },
        )
        .unwrap();
        assert_eq!(
            routed
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            ["b", "c", "a"]
        );
    }

    #[test]
    fn legacy_unspecified_requirements_preserve_preferred_and_input_order() {
        let candidates = vec![
            candidate("legacy-a", CapabilityDescriptor::default()),
            candidate("legacy-b", CapabilityDescriptor::default()),
        ];
        let routed = route_models(
            &candidates,
            &CapabilityRequirements::default(),
            Some("legacy-a"),
            &FallbackPolicy::default(),
        )
        .unwrap();
        assert_eq!(
            routed
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            ["legacy-a", "legacy-b"]
        );
    }
}
