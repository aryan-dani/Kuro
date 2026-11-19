//! Typed feature failures (ADR 0006). Speech is built elsewhere from reply keys.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeatureError {
    NotConfigured { detail: String },
    Unreachable { detail: String },
    Timeout { detail: String },
    Unauthorized { detail: String },
    Rejected { detail: String },
    InvalidInput { detail: String },
    Busy { detail: String },
    DeviceUnavailable { detail: String },
    Disabled { detail: String },
    Internal { detail: String },
}

impl FeatureError {
    pub fn invalid_input(detail: impl Into<String>) -> Self {
        Self::InvalidInput { detail: detail.into() }
    }

    pub fn detail(&self) -> &str {
        match self {
            Self::NotConfigured { detail }
            | Self::Unreachable { detail }
            | Self::Timeout { detail }
            | Self::Unauthorized { detail }
            | Self::Rejected { detail }
            | Self::InvalidInput { detail }
            | Self::Busy { detail }
            | Self::DeviceUnavailable { detail }
            | Self::Disabled { detail }
            | Self::Internal { detail } => detail,
        }
    }
}

impl fmt::Display for FeatureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind_name(), self.detail())
    }
}

impl std::error::Error for FeatureError {}

impl FeatureError {
    fn kind_name(&self) -> &'static str {
        match self {
            Self::NotConfigured { .. } => "NotConfigured",
            Self::Unreachable { .. } => "Unreachable",
            Self::Timeout { .. } => "Timeout",
            Self::Unauthorized { .. } => "Unauthorized",
            Self::Rejected { .. } => "Rejected",
            Self::InvalidInput { .. } => "InvalidInput",
            Self::Busy { .. } => "Busy",
            Self::DeviceUnavailable { .. } => "DeviceUnavailable",
            Self::Disabled { .. } => "Disabled",
            Self::Internal { .. } => "Internal",
        }
    }
}
