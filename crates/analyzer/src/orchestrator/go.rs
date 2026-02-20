//! Go orchestrator — delegates to DescriptorOrchestrator.

use super::descriptor::DescriptorOrchestrator;
use crate::languages::go::GoAnalyzer;

/// Create a Go orchestrator using the descriptor + parser pattern.
#[must_use]
pub fn orchestrator() -> DescriptorOrchestrator {
    DescriptorOrchestrator::new(GoAnalyzer::descriptor(), GoAnalyzer::parser())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::LanguageOrchestrator;

    #[test]
    fn go_orchestrator_language_id() {
        let orch = orchestrator();
        assert_eq!(orch.language_id(), "go");
    }
}
