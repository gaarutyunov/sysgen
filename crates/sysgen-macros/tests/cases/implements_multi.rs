use sysgen_macros::implements;

#[implements("VehicleRequirements::MassRequirement", "VehicleRequirements::SafetyRequirement")]
pub struct VehicleSafetySystem;

fn main() {}
