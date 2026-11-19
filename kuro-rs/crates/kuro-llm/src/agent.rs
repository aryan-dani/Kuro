//! Agent loop config (phase 26A). Engine wiring lands in a later slice.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentConfig {
    pub enabled: bool,
    pub max_steps: u32,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self { enabled: false, max_steps: 3 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_off_by_default() {
        let cfg = AgentConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.max_steps, 3);
    }
}
