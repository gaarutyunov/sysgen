/// Integration tests verifying that syster-base correctly parses SysML v2 constructs:
/// `requirement def`, `satisfy`, and `verify`.
///
/// Uses the real syster-base git dependency (jade-codes/syster-base) via AnalysisHost API.
/// Note: the package is named `syster-base` but the Rust crate name is `syster`.
use syster::ide::AnalysisHost;

const FIXTURE: &str = include_str!("fixtures/vehicle.sysml");

/// Verifies that `requirement def` constructs are parsed and appear in the symbol table.
#[test]
fn syster_parses_requirement_def() {
    let mut host = AnalysisHost::new();
    let errors = host.set_file_content("vehicle.sysml", FIXTURE);
    assert!(
        errors.is_empty(),
        "vehicle.sysml has parse errors: {errors:#?}"
    );

    let analysis = host.analysis();
    let index = analysis.symbol_index();

    assert!(
        index
            .lookup_qualified("VehicleRequirements::MassRequirement")
            .is_some(),
        "MassRequirement not found in symbol table"
    );
    assert!(
        index
            .lookup_qualified("VehicleRequirements::PerformanceRequirement")
            .is_some(),
        "PerformanceRequirement not found in symbol table"
    );
    assert!(
        index
            .lookup_qualified("VehicleRequirements::SafetyRequirement")
            .is_some(),
        "SafetyRequirement not found in symbol table"
    );
}

/// Verifies that `verification def` is parsed and appears in the symbol table
/// with the correct `VerificationCaseDefinition` kind (fixed in fork).
#[test]
fn syster_parses_verification_def() {
    use syster::hir::SymbolKind;

    let mut host = AnalysisHost::new();
    let errors = host.set_file_content("vehicle.sysml", FIXTURE);
    assert!(
        errors.is_empty(),
        "vehicle.sysml has parse errors: {errors:#?}"
    );

    let analysis = host.analysis();
    let index = analysis.symbol_index();

    let sym = index
        .lookup_qualified("VehicleRequirements::MassVerification")
        .expect("MassVerification (verification def) not found in symbol table");
    assert_eq!(
        sym.kind,
        SymbolKind::VerificationCaseDefinition,
        "MassVerification should be VerificationCaseDefinition, not {:?}",
        sym.kind
    );
}

/// Verifies that short names from angle-bracket IDs (e.g. `<'R1'>`) are extracted.
///
/// After parsing, `requirement def <'R1'> MassRequirement` should expose
/// `short_name = Some("R1")` on the corresponding HirSymbol.
#[test]
fn syster_exposes_short_names() {
    let mut host = AnalysisHost::new();
    let errors = host.set_file_content("vehicle.sysml", FIXTURE);
    assert!(
        errors.is_empty(),
        "vehicle.sysml has parse errors: {errors:#?}"
    );

    let analysis = host.analysis();
    let index = analysis.symbol_index();

    let r1 = index
        .lookup_qualified("VehicleRequirements::MassRequirement")
        .expect("MassRequirement not found");
    assert_eq!(
        r1.short_name.as_deref(),
        Some("R1"),
        "MassRequirement should have short_name R1"
    );

    let r2 = index
        .lookup_qualified("VehicleRequirements::PerformanceRequirement")
        .expect("PerformanceRequirement not found");
    assert_eq!(
        r2.short_name.as_deref(),
        Some("R2"),
        "PerformanceRequirement should have short_name R2"
    );

    let r3 = index
        .lookup_qualified("VehicleRequirements::SafetyRequirement")
        .expect("SafetyRequirement not found");
    assert_eq!(
        r3.short_name.as_deref(),
        Some("R3"),
        "SafetyRequirement should have short_name R3"
    );

    let v1 = index
        .lookup_qualified("VehicleRequirements::MassVerification")
        .expect("MassVerification not found");
    assert_eq!(
        v1.short_name.as_deref(),
        Some("V1"),
        "MassVerification should have short_name V1"
    );
}

/// Verifies that the `satisfy requirement` inside `part def Vehicle` is tracked in the
/// semantic graph as a child PartUsage symbol scoped under Vehicle.
///
/// syster-base represents `satisfy requirement : SomeReq` as an anonymous PartUsage child
/// symbol with a qualified name of the form `Vehicle::<:SomeReq#N@LM>`. The relationship
/// to the requirement type is captured in the symbol's `type_refs`.
#[test]
fn syster_tracks_satisfy_relationship() {
    let mut host = AnalysisHost::new();
    let errors = host.set_file_content("vehicle.sysml", FIXTURE);
    assert!(
        errors.is_empty(),
        "vehicle.sysml has parse errors: {errors:#?}"
    );

    let analysis = host.analysis();

    // Locate the anonymous child usage that represents `satisfy requirement : MassRequirement`
    // inside Vehicle. syster-base exposes it as a PartUsage with a generated qualified name
    // prefixed by "VehicleRequirements::Vehicle::" that embeds the requirement type name.
    let satisfy_symbol = analysis.workspace_symbols(None).into_iter().find(|s| {
        s.qualified_name
            .starts_with("VehicleRequirements::Vehicle::")
            && s.qualified_name.contains("MassRequirement")
    });

    assert!(
        satisfy_symbol.is_some(),
        "Expected a child symbol under VehicleRequirements::Vehicle referencing MassRequirement \
         (the satisfy requirement usage) but none found in workspace symbols"
    );
}

/// Verifies that the `verify requirement` inside `verification def MassVerification` is
/// tracked as a child symbol scoped under MassVerification.
#[test]
fn syster_tracks_verify_relationship() {
    let mut host = AnalysisHost::new();
    let errors = host.set_file_content("vehicle.sysml", FIXTURE);
    assert!(
        errors.is_empty(),
        "vehicle.sysml has parse errors: {errors:#?}"
    );

    let analysis = host.analysis();

    let verify_symbol = analysis.workspace_symbols(None).into_iter().find(|s| {
        s.qualified_name
            .starts_with("VehicleRequirements::MassVerification::")
            && s.qualified_name.contains("MassRequirement")
    });

    assert!(
        verify_symbol.is_some(),
        "Expected a child symbol under VehicleRequirements::MassVerification referencing \
         MassRequirement (the verify requirement usage) but none found in workspace symbols"
    );
}
