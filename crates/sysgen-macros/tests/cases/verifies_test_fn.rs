use sysgen_macros::verifies;

#[verifies("VehicleRequirements::MassRequirement")]
#[test]
fn test_mass_under_limit() {
    assert!(1500.0_f64 <= 2000.0_f64);
}

fn main() {}
