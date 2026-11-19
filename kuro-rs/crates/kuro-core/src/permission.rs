//! Permission tiers and confirmation policy (ADR 0016).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionTier {
    ReadOnly,
    Reversible,
    ExternalSend,
    Irreversible,
}

/// Who initiated the action (voice phrase, LLM tool call, or API).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionSource {
    Voice,
    Llm,
    Api,
}

impl PermissionTier {
    pub fn parse(manifest: &str) -> Option<Self> {
        match manifest.trim().to_ascii_lowercase().as_str() {
            "read_only" => Some(Self::ReadOnly),
            "reversible" => Some(Self::Reversible),
            "external_send" => Some(Self::ExternalSend),
            "irreversible" => Some(Self::Irreversible),
            _ => None,
        }
    }

    pub const fn manifest_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::Reversible => "reversible",
            Self::ExternalSend => "external_send",
            Self::Irreversible => "irreversible",
        }
    }
}

/// Whether the turn machine should enter `Confirming` before running the plan.
pub fn needs_confirm(tier: PermissionTier, source: ActionSource) -> bool {
    match (tier, source) {
        (PermissionTier::ReadOnly | PermissionTier::Reversible, _) => false,
        (PermissionTier::ExternalSend, ActionSource::Voice) => false,
        (PermissionTier::ExternalSend, ActionSource::Llm | ActionSource::Api) => true,
        (PermissionTier::Irreversible, _) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_manifest_tiers() {
        assert_eq!(PermissionTier::parse("read_only"), Some(PermissionTier::ReadOnly));
        assert_eq!(PermissionTier::parse("REVERSIBLE"), Some(PermissionTier::Reversible));
        assert_eq!(PermissionTier::parse("external_send"), Some(PermissionTier::ExternalSend));
        assert_eq!(PermissionTier::parse("irreversible"), Some(PermissionTier::Irreversible));
        assert_eq!(PermissionTier::parse("unknown"), None);
    }

    #[test]
    fn tier_matrix() {
        for tier in [
            PermissionTier::ReadOnly,
            PermissionTier::Reversible,
            PermissionTier::ExternalSend,
            PermissionTier::Irreversible,
        ] {
            for source in [ActionSource::Voice, ActionSource::Llm, ActionSource::Api] {
                let confirm = needs_confirm(tier, source);
                let expected = match (tier, source) {
                    (PermissionTier::ReadOnly | PermissionTier::Reversible, _) => false,
                    (PermissionTier::ExternalSend, ActionSource::Voice) => false,
                    (PermissionTier::ExternalSend, ActionSource::Llm | ActionSource::Api) => true,
                    (PermissionTier::Irreversible, _) => true,
                };
                assert_eq!(confirm, expected, "tier {:?} source {:?}", tier, source);
            }
        }
    }
}
