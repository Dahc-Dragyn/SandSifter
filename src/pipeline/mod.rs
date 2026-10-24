pub mod hook_machine;
pub mod script_generator;

pub use hook_machine::{
    CustomRubric, GeneratedHook, GrammaticalTopic, HookMachineEngine, HookMachineReport,
    NicheLexicon, NicheToneProfile, StairStepTier, StairSteppingProgression, TripleVectorScore,
};
pub use script_generator::{
    AcceptanceCriterion, BeatType, ConceptArchetype, CreatorIdentityContext, FourPRole,
    FourPsFramework, PredictiveScript, ScriptBeat, ScriptGenerator, ScriptRequest,
    ThumbnailConceptBrief, TitleAcceptanceCriteria,
};

