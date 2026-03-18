use sysgen_macros::implements;

const MASS_LIMIT: f64 = 2000.0;

pub struct Vehicle {
    mass: f64,
}

#[implements("VehicleRequirements::MassRequirement")]
pub fn check_mass(vehicle: &Vehicle) -> bool {
    vehicle.mass <= MASS_LIMIT
}

#[implements("R1")]
pub struct MassChecker;

fn main() {}
