//! Java orchestrator — delegates to DescriptorOrchestrator.

use super::descriptor::DescriptorOrchestrator;
use crate::languages::java::JavaAnalyzer;

/// Create a Java orchestrator using the descriptor + parser pattern.
#[must_use]
pub fn orchestrator() -> DescriptorOrchestrator {
    DescriptorOrchestrator::new(JavaAnalyzer::descriptor(), JavaAnalyzer::parser())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::LanguageOrchestrator;

    #[test]
    fn java_orchestrator_language_id() {
        let orch = orchestrator();
        assert_eq!(orch.language_id(), "java");
    }
}
